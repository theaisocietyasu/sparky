//! Memories in the SQLite file. Sensitive text is sealed at rest.

use async_trait::async_trait;
use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveTime, TimeZone, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::{Seal, database, parse_stamp, stamp};
use crate::core::traits::memory::MemoryStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::{Memory, MemoryKind, NewMemory};
use crate::core::types::store::StoreError;

/// How many memories a student keeps, and what counts as due soon.
#[derive(Debug, Clone, Copy)]
pub struct MemoryLimits {
    /// Memories per student; at the cap the oldest fact goes first.
    pub max_per_user: usize,
    /// A task due within this many days is recalled first.
    pub task_soon_days: i64,
    /// Hours from UTC of the day a task is due on.
    pub utc_offset_hours: i32,
}

/// One row to insert, as the import and remember write it.
#[derive(Debug, Clone)]
pub(super) struct MemoryRow {
    /// Row id.
    pub id: Uuid,
    /// Kind.
    pub kind: MemoryKind,
    /// The text in clear.
    pub content: String,
    /// Sealed at rest.
    pub sensitive: bool,
    /// The conversation it came from.
    pub source_conversation: Option<Uuid>,
    /// The newest message of that conversation when it was written.
    pub source_seq: Option<i64>,
    /// When it was written.
    pub created_at: DateTime<Utc>,
    /// When it stops being recalled.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Memories held in the SQLite file.
pub struct SqliteMemory {
    pool: SqlitePool,
    seal: Seal,
    limits: MemoryLimits,
}

/// The end of day due at offset hours from UTC.
fn end_of(due: NaiveDate, offset_hours: i32) -> Option<DateTime<Utc>> {
    let offset = FixedOffset::east_opt(offset_hours * 3600)?;
    let end = due.and_time(NaiveTime::from_hms_opt(23, 59, 59)?);
    offset
        .from_local_datetime(&end)
        .single()
        .map(|t| t.with_timezone(&Utc))
}

impl SqliteMemory {
    /// Wraps a pool, the seal for sensitive rows, and the per-student limits.
    pub fn new(pool: SqlitePool, seal: Seal, limits: MemoryLimits) -> Self {
        Self { pool, seal, limits }
    }

    /// The memory from one row, or None when a sealed row does not open.
    fn row(&self, row: &SqliteRow) -> Result<Option<Memory>, StoreError> {
        let id: String = row.try_get("id").map_err(database)?;
        let kind: String = row.try_get("kind").map_err(database)?;
        let bytes: Vec<u8> = row.try_get("content").map_err(database)?;
        let sensitive: bool = row.try_get("sensitive").map_err(database)?;
        let content = if sensitive {
            let nonce: Option<Vec<u8>> = row.try_get("nonce").map_err(database)?;
            let opened = nonce.and_then(|n| self.seal.open(&n, &bytes));
            let Some(text) = opened else {
                tracing::warn!(memory_id = %id, "a sealed memory does not open; left out of recall");
                return Ok(None);
            };
            text
        } else {
            String::from_utf8(bytes).map_err(|e| StoreError::Database(e.to_string()))?
        };
        let created: String = row.try_get("created_at").map_err(database)?;
        let expires: Option<String> = row.try_get("expires_at").map_err(database)?;
        Ok(Some(Memory {
            id: Uuid::parse_str(&id).map_err(|e| StoreError::Database(e.to_string()))?,
            kind: MemoryKind::parse(&kind)
                .ok_or_else(|| StoreError::Database(format!("memory kind {kind} is unknown")))?,
            content,
            sensitive,
            created_at: parse_stamp(&created)?,
            expires_at: expires.as_deref().map(parse_stamp).transpose()?,
        }))
    }

    /// Deletes the oldest memories of the caller until one more fits under the cap.
    async fn make_room(
        &self,
        conn: &mut SqliteConnection,
        ctx: &RequestContext,
    ) -> Result<(), StoreError> {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM memories WHERE tenant_id = ? AND user_id = ?")
                .bind(&ctx.tenant_id)
                .bind(&ctx.user_id)
                .fetch_one(&mut *conn)
                .await
                .map_err(database)?;
        let cap = i64::try_from(self.limits.max_per_user).unwrap_or(i64::MAX);
        let over = count + 1 - cap;
        if over <= 0 {
            return Ok(());
        }
        // Facts go first, then preferences, then tasks; oldest first within each.
        sqlx::query(
            "DELETE FROM memories WHERE id IN (SELECT id FROM memories \
             WHERE tenant_id = ? AND user_id = ? ORDER BY \
             CASE kind WHEN 'fact' THEN 0 WHEN 'preference' THEN 1 ELSE 2 END, created_at \
             LIMIT ?)",
        )
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(over)
        .execute(&mut *conn)
        .await
        .map_err(database)?;
        Ok(())
    }
}

/// Inserts one memory for tenant and user, sealing it when it is sensitive. False if the id exists.
pub(super) async fn insert(
    conn: &mut SqliteConnection,
    seal: &Seal,
    tenant_id: &str,
    user_id: &str,
    memory: &MemoryRow,
) -> Result<bool, StoreError> {
    let (content, nonce) = if memory.sensitive {
        let Some((nonce, sealed)) = seal.seal(&memory.content) else {
            return Err(StoreError::Database(
                "a sensitive memory could not be sealed".into(),
            ));
        };
        (sealed, Some(nonce))
    } else {
        (memory.content.as_bytes().to_vec(), None)
    };
    let done = sqlx::query(
        "INSERT INTO memories (id, tenant_id, user_id, kind, content, sensitive, nonce, \
         source_conversation, source_seq, created_at, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (id) DO NOTHING",
    )
    .bind(memory.id.to_string())
    .bind(tenant_id)
    .bind(user_id)
    .bind(memory.kind.as_str())
    .bind(content)
    .bind(memory.sensitive)
    .bind(nonce)
    .bind(memory.source_conversation.map(|c| c.to_string()))
    .bind(memory.source_seq)
    .bind(stamp(memory.created_at))
    .bind(memory.expires_at.map(stamp))
    .execute(conn)
    .await
    .map_err(database)?;
    Ok(done.rows_affected() == 1)
}

#[async_trait]
impl MemoryStore for SqliteMemory {
    async fn recall(&self, ctx: &RequestContext) -> Result<Vec<Memory>, StoreError> {
        let now = Utc::now();
        let soon = now + Duration::days(self.limits.task_soon_days);
        let rows = sqlx::query(
            "SELECT id, kind, content, sensitive, nonce, created_at, expires_at \
             FROM memories WHERE tenant_id = ? AND user_id = ? \
             AND (expires_at IS NULL OR expires_at > ?) \
             ORDER BY CASE WHEN kind = 'task' AND expires_at <= ? THEN 0 ELSE 1 END, \
             CASE WHEN kind = 'task' AND expires_at <= ? THEN expires_at END, \
             created_at DESC",
        )
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(stamp(now))
        .bind(stamp(soon))
        .bind(stamp(soon))
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            if let Some(memory) = self.row(row)? {
                out.push(memory);
            }
        }
        Ok(out)
    }

    async fn remember(
        &self,
        ctx: &RequestContext,
        memory: &NewMemory,
    ) -> Result<Memory, StoreError> {
        let expires_at = match (memory.kind, memory.due) {
            (MemoryKind::Task, Some(due)) => Some(
                end_of(due, self.limits.utc_offset_hours)
                    .ok_or_else(|| StoreError::Database(format!("due date {due}")))?,
            ),
            (MemoryKind::Task, None) => {
                return Err(StoreError::Database("a task needs a due date".into()));
            }
            _ => None,
        };
        let mut tx = self.pool.begin().await.map_err(database)?;
        let source: Option<(i64,)> = sqlx::query_as(
            "SELECT MAX(seq) FROM messages WHERE conversation_id = ? HAVING MAX(seq) IS NOT NULL",
        )
        .bind(ctx.conversation_id.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(database)?;
        self.make_room(&mut tx, ctx).await?;
        let row = MemoryRow {
            id: Uuid::new_v4(),
            kind: memory.kind,
            content: memory.text.clone(),
            sensitive: memory.sensitive,
            source_conversation: Some(ctx.conversation_id),
            source_seq: source.map(|(seq,)| seq),
            created_at: Utc::now(),
            expires_at,
        };
        insert(&mut tx, &self.seal, &ctx.tenant_id, &ctx.user_id, &row).await?;
        tx.commit().await.map_err(database)?;
        Ok(Memory {
            id: row.id,
            kind: row.kind,
            content: row.content,
            sensitive: row.sensitive,
            created_at: row.created_at,
            expires_at: row.expires_at,
        })
    }

    async fn forget(&self, ctx: &RequestContext, id: Uuid) -> Result<bool, StoreError> {
        let done =
            sqlx::query("DELETE FROM memories WHERE id = ? AND tenant_id = ? AND user_id = ?")
                .bind(id.to_string())
                .bind(&ctx.tenant_id)
                .bind(&ctx.user_id)
                .execute(&self.pool)
                .await
                .map_err(database)?;
        Ok(done.rows_affected() == 1)
    }

    async fn forget_all(&self, ctx: &RequestContext) -> Result<u64, StoreError> {
        let done = sqlx::query("DELETE FROM memories WHERE tenant_id = ? AND user_id = ?")
            .bind(&ctx.tenant_id)
            .bind(&ctx.user_id)
            .execute(&self.pool)
            .await
            .map_err(database)?;
        Ok(done.rows_affected())
    }
}
