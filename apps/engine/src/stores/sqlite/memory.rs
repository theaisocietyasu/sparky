//! Memories in the SQLite file. Sensitive text is sealed at rest.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::{Seal, database, parse_stamp, stamp};
use crate::core::traits::memory::MemoryStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::{Memory, MemoryKind, MemoryQuery};
use crate::core::types::store::StoreError;

/// One memory to write.
#[derive(Debug, Clone)]
pub struct NewMemory {
    /// Row id.
    pub id: Uuid,
    /// Kind.
    pub kind: MemoryKind,
    /// The remembered text.
    pub content: String,
    /// Sealed at rest when true.
    pub sensitive: bool,
    /// Confidence, 0 to 1.
    pub confidence: f32,
    /// The conversation it came from, when known.
    pub source_conversation: Option<Uuid>,
    /// The message it came from, when known.
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
}

impl SqliteMemory {
    /// Wraps a pool and the seal for sensitive rows.
    pub fn new(pool: SqlitePool, seal: Seal) -> Self {
        Self { pool, seal }
    }

    /// Writes one memory of the caller.
    #[cfg(test)]
    pub async fn write(&self, ctx: &RequestContext, memory: &NewMemory) -> Result<(), StoreError> {
        let mut conn = self.pool.acquire().await.map_err(database)?;
        insert(&mut conn, &self.seal, &ctx.tenant_id, &ctx.user_id, memory).await?;
        Ok(())
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
        let confidence: f64 = row.try_get("confidence").map_err(database)?;
        let created: String = row.try_get("created_at").map_err(database)?;
        let expires: Option<String> = row.try_get("expires_at").map_err(database)?;
        // Confidence is a fraction from 0 to 1.
        #[allow(clippy::cast_possible_truncation)]
        Ok(Some(Memory {
            id: Uuid::parse_str(&id).map_err(|e| StoreError::Database(e.to_string()))?,
            kind: MemoryKind::parse(&kind)
                .ok_or_else(|| StoreError::Database(format!("memory kind {kind} is unknown")))?,
            content,
            confidence: confidence as f32,
            created_at: parse_stamp(&created)?,
            expires_at: expires.as_deref().map(parse_stamp).transpose()?,
        }))
    }
}

/// Inserts one memory for tenant and user, sealing it when it is sensitive. False if the id exists.
pub(super) async fn insert(
    conn: &mut SqliteConnection,
    seal: &Seal,
    tenant_id: &str,
    user_id: &str,
    memory: &NewMemory,
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
        "INSERT INTO memories (id, tenant_id, user_id, kind, content, sensitive, nonce, confidence, \
         source_conversation, source_seq, created_at, expires_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (id) DO NOTHING",
    )
    .bind(memory.id.to_string())
    .bind(tenant_id)
    .bind(user_id)
    .bind(memory.kind.as_str())
    .bind(content)
    .bind(memory.sensitive)
    .bind(nonce)
    .bind(f64::from(memory.confidence))
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
    async fn recall(
        &self,
        ctx: &RequestContext,
        q: &MemoryQuery,
    ) -> Result<Vec<Memory>, StoreError> {
        let kinds: Vec<&str> = q.kinds.iter().map(|k| k.as_str()).collect();
        let placeholders = vec!["?"; kinds.len()].join(", ");
        let filter = if kinds.is_empty() {
            String::new()
        } else {
            format!(" AND kind IN ({placeholders})")
        };
        let sql = format!(
            "SELECT id, kind, content, sensitive, nonce, confidence, created_at, expires_at \
             FROM memories WHERE tenant_id = ? AND user_id = ? \
             AND (expires_at IS NULL OR expires_at > ?){filter} \
             ORDER BY confidence DESC, created_at DESC LIMIT ?"
        );
        let mut query = sqlx::query(&sql)
            .bind(&ctx.tenant_id)
            .bind(&ctx.user_id)
            .bind(stamp(Utc::now()));
        for kind in &kinds {
            query = query.bind(*kind);
        }
        let limit = i64::try_from(q.limit.max(1)).unwrap_or(i64::MAX);
        let rows = query
            .bind(limit)
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
}
