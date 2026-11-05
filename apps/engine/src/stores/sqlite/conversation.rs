//! Conversations and their messages in the SQLite file.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{database, stamp};
use crate::core::traits::conversation::ConversationStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::conversation::{ChatHit, ConversationSummary, Parent, Stored, home_id};
use crate::core::types::store::StoreError;

/// The row role of a summary.
const SUMMARY: &str = "summary";

/// The role column of a message. The whole message, its own role included, is the content.
fn row_role(message: &Message) -> &'static str {
    match message.role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
        Role::Summary => SUMMARY,
    }
}

/// One message as the content column.
fn content(message: &Message) -> Result<String, StoreError> {
    serde_json::to_string(message).map_err(|e| StoreError::Database(e.to_string()))
}

/// One message from the content column.
fn message(text: &str) -> Result<Message, StoreError> {
    serde_json::from_str(text).map_err(|e| StoreError::Database(format!("message: {e}")))
}

/// An FTS5 match over the words of query, each quoted and prefix matched, any of them.
fn fts_query(query: &str) -> Option<String> {
    let words: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 1)
        .take(12)
        .map(|w| format!("\"{}\"*", w.to_lowercase()))
        .collect();
    (!words.is_empty()).then(|| words.join(" OR "))
}

/// Conversations held in the SQLite file.
pub struct SqliteConversations {
    pool: SqlitePool,
}

impl SqliteConversations {
    /// Wraps a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Writes rows to the request conversation in one transaction, or NotOwned.
    async fn write(
        &self,
        ctx: &RequestContext,
        rows: &[(&str, String, Option<i64>)],
    ) -> Result<(), StoreError> {
        let now = stamp(Utc::now());
        let mut tx = self.pool.begin().await.map_err(database)?;
        let touched = sqlx::query(
            "UPDATE conversations SET updated_at = ? WHERE id = ? AND tenant_id = ? AND user_id = ?",
        )
        .bind(&now)
        .bind(ctx.conversation_id.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .execute(&mut *tx)
        .await
        .map_err(database)?;
        if touched.rows_affected() == 0 {
            return Err(StoreError::NotOwned);
        }
        for (role, body, covers) in rows {
            sqlx::query(
                "INSERT INTO messages (conversation_id, role, content, covers, created_at) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(ctx.conversation_id.to_string())
            .bind(role)
            .bind(body)
            .bind(covers)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(database)?;
        }
        tx.commit().await.map_err(database)
    }

    /// Whether the request conversation belongs to the caller, at any visibility.
    async fn owned(&self, ctx: &RequestContext) -> Result<bool, StoreError> {
        let found = sqlx::query(
            "SELECT 1 FROM conversations WHERE id = ? AND tenant_id = ? AND user_id = ?",
        )
        .bind(ctx.conversation_id.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        Ok(found.is_some())
    }
}

#[async_trait]
impl ConversationStore for SqliteConversations {
    async fn ensure(&self, ctx: &RequestContext, channel_id: &str) -> Result<(), StoreError> {
        let now = stamp(Utc::now());
        sqlx::query(
            "INSERT INTO conversations \
             (id, tenant_id, user_id, channel_id, visibility, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT (id) DO NOTHING",
        )
        .bind(ctx.conversation_id.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(channel_id)
        .bind(ctx.visibility.as_str())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(database)?;
        let same = sqlx::query(
            "SELECT 1 FROM conversations WHERE id = ? AND tenant_id = ? AND user_id = ? \
             AND channel_id = ? AND visibility = ?",
        )
        .bind(ctx.conversation_id.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(channel_id)
        .bind(ctx.visibility.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        match same {
            Some(_) => Ok(()),
            None => Err(StoreError::NotOwned),
        }
    }

    async fn owns(&self, ctx: &RequestContext) -> Result<bool, StoreError> {
        let found = sqlx::query(
            "SELECT 1 FROM conversations WHERE id = ? AND tenant_id = ? AND user_id = ? \
             AND visibility = ?",
        )
        .bind(ctx.conversation_id.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(ctx.visibility.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        Ok(found.is_some())
    }

    async fn load(&self, ctx: &RequestContext, limit: usize) -> Result<Vec<Stored>, StoreError> {
        if !self.owned(ctx).await? {
            return Ok(Vec::new());
        }
        let id = ctx.conversation_id.to_string();
        let summary = sqlx::query(
            "SELECT seq, content, covers FROM messages WHERE conversation_id = ? AND role = ? \
             ORDER BY seq DESC LIMIT 1",
        )
        .bind(&id)
        .bind(SUMMARY)
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        let mut out = Vec::new();
        let mut start = 0_i64;
        if let Some(row) = summary {
            let seq: i64 = row.try_get("seq").map_err(database)?;
            let covers: Option<i64> = row.try_get("covers").map_err(database)?;
            start = covers.unwrap_or(seq);
            let text: String = row.try_get("content").map_err(database)?;
            out.push(Stored {
                position: start,
                message: message(&text)?,
            });
        }
        let limit = i64::try_from(limit.max(1)).unwrap_or(i64::MAX);
        let recent = sqlx::query(
            "SELECT seq, content FROM messages WHERE conversation_id = ? AND role != ? AND seq > ? \
             ORDER BY seq DESC LIMIT ?",
        )
        .bind(&id)
        .bind(SUMMARY)
        .bind(start)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        for row in recent.iter().rev() {
            let text: String = row.try_get("content").map_err(database)?;
            out.push(Stored {
                position: row.try_get("seq").map_err(database)?,
                message: message(&text)?,
            });
        }
        Ok(out)
    }

    async fn append(&self, ctx: &RequestContext, turns: &[Message]) -> Result<(), StoreError> {
        if turns.is_empty() {
            return Ok(());
        }
        let rows = turns
            .iter()
            .map(|m| Ok((row_role(m), content(m)?, None)))
            .collect::<Result<Vec<_>, StoreError>>()?;
        self.write(ctx, &rows).await
    }

    async fn append_summary(
        &self,
        ctx: &RequestContext,
        summary: &Message,
        covers: i64,
    ) -> Result<(), StoreError> {
        if covers < 1 {
            return Err(StoreError::Database(
                "covers must be a message position".into(),
            ));
        }
        self.write(ctx, &[(SUMMARY, content(summary)?, Some(covers))])
            .await
    }

    async fn latest(
        &self,
        ctx: &RequestContext,
        channel_id: &str,
    ) -> Result<Option<Uuid>, StoreError> {
        let row = sqlx::query(
            "SELECT id FROM conversations WHERE tenant_id = ? AND user_id = ? AND channel_id = ? \
             AND visibility = ? AND ended_at IS NULL ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(channel_id)
        .bind(ctx.visibility.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let id: String = row.try_get("id").map_err(database)?;
        Uuid::parse_str(&id)
            .map(Some)
            .map_err(|e| StoreError::Database(format!("conversation id {id}: {e}")))
    }

    async fn end(&self, ctx: &RequestContext, channel_id: &str) -> Result<u64, StoreError> {
        let ended = sqlx::query(
            "UPDATE conversations SET ended_at = ? WHERE tenant_id = ? AND user_id = ? \
             AND channel_id = ? AND ended_at IS NULL AND id != ?",
        )
        .bind(stamp(Utc::now()))
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(channel_id)
        .bind(home_id(&ctx.tenant_id, &ctx.user_id).to_string())
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(ended.rows_affected())
    }

    async fn list(
        &self,
        ctx: &RequestContext,
        channel_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSummary>, StoreError> {
        let limit = i64::try_from(limit.max(1)).unwrap_or(i64::MAX);
        let rows = sqlx::query(
            "SELECT c.id, c.updated_at, c.parent_id, c.parent_seq, (SELECT m.content FROM messages m \
             WHERE m.conversation_id = c.id AND m.role = 'user' ORDER BY m.seq LIMIT 1) AS first \
             FROM conversations c WHERE c.tenant_id = ? AND c.user_id = ? AND c.channel_id = ? \
             AND c.ended_at IS NULL AND c.id != ? ORDER BY c.updated_at DESC LIMIT ?",
        )
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(channel_id)
        .bind(home_id(&ctx.tenant_id, &ctx.user_id).to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        let mut out = Vec::new();
        for row in rows {
            let first: Option<String> = row.try_get("first").map_err(database)?;
            let Some(first) = first else {
                continue;
            };
            let parent_id: Option<String> = row.try_get("parent_id").map_err(database)?;
            let parent_seq: Option<i64> = row.try_get("parent_seq").map_err(database)?;
            let parent = parent_id.zip(parent_seq).and_then(|(id, position)| {
                Uuid::parse_str(&id).ok().map(|conversation_id| Parent {
                    conversation_id,
                    position,
                })
            });
            out.push(ConversationSummary {
                id: row.try_get("id").map_err(database)?,
                title: message(&first)?.content,
                updated_at: row.try_get("updated_at").map_err(database)?,
                parent,
            });
        }
        Ok(out)
    }

    async fn search(
        &self,
        ctx: &RequestContext,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ChatHit>, StoreError> {
        let Some(matched) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let rows = sqlx::query(
            "SELECT c.id, c.channel_id, m.role, m.created_at, \
             snippet(messages_fts, 0, '', '', ' ... ', 24) AS snippet \
             FROM messages_fts JOIN messages m ON m.seq = messages_fts.rowid \
             JOIN conversations c ON c.id = m.conversation_id \
             WHERE messages_fts MATCH ? AND c.tenant_id = ? AND c.user_id = ? \
             AND c.visibility = 'private' AND c.id != ? \
             ORDER BY bm25(messages_fts) LIMIT ?",
        )
        .bind(matched)
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(ctx.conversation_id.to_string())
        .bind(i64::try_from(limit.max(1)).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        rows.iter()
            .map(|row| {
                Ok(ChatHit {
                    conversation_id: row.try_get("id").map_err(database)?,
                    channel_id: row.try_get("channel_id").map_err(database)?,
                    role: row.try_get("role").map_err(database)?,
                    snippet: row.try_get("snippet").map_err(database)?,
                    at: row.try_get("created_at").map_err(database)?,
                })
            })
            .collect()
    }

    async fn link(&self, ctx: &RequestContext, parent: &Parent) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE conversations SET parent_id = ?, parent_seq = ? \
             WHERE id = ? AND tenant_id = ? AND user_id = ? AND parent_id IS NULL AND id != ? \
             AND EXISTS (SELECT 1 FROM messages m JOIN conversations p ON p.id = m.conversation_id \
             WHERE p.id = ? AND p.tenant_id = ? AND p.user_id = ? AND m.seq = ?)",
        )
        .bind(parent.conversation_id.to_string())
        .bind(parent.position)
        .bind(ctx.conversation_id.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(parent.conversation_id.to_string())
        .bind(parent.conversation_id.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(parent.position)
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(())
    }

    async fn delete(&self, ctx: &RequestContext) -> Result<bool, StoreError> {
        if !self.owned(ctx).await? {
            return Ok(false);
        }
        let id = ctx.conversation_id.to_string();
        let mut tx = self.pool.begin().await.map_err(database)?;
        sqlx::query("DELETE FROM messages WHERE conversation_id = ?")
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(database)?;
        let gone =
            sqlx::query("DELETE FROM conversations WHERE id = ? AND tenant_id = ? AND user_id = ?")
                .bind(&id)
                .bind(&ctx.tenant_id)
                .bind(&ctx.user_id)
                .execute(&mut *tx)
                .await
                .map_err(database)?;
        tx.commit().await.map_err(database)?;
        Ok(gone.rows_affected() > 0)
    }
}
