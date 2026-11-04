//! One-time import of the platform agents export into the SQLite file.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::memory::{MemoryRow, insert};
use super::{Seal, database, stamp};
use crate::core::types::export::{ConversationPage, ExportConversation, MemoryPage};
use crate::core::types::memory::MemoryKind;
use crate::core::types::store::StoreError;

/// Why an import stopped.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The source was imported before and again was not asked for.
    #[error("{0} was imported on {1}; pass --again to import it once more")]
    Done(String, String),
    /// A row in the export is not one the store can hold.
    #[error("export row {0}: {1}")]
    Row(String, String),
    /// The store refused the write.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// What an import wrote.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportCounts {
    /// Conversations written.
    pub conversations: u64,
    /// Messages written.
    pub messages: u64,
    /// Memories written.
    pub memories: u64,
    /// Conversations left out: older than the cutoff or already present.
    pub skipped: u64,
}

/// Writes export pages for one tenant, one transaction per page.
pub struct Importer {
    pool: SqlitePool,
    seal: Seal,
    tenant_id: String,
    cutoff: DateTime<Utc>,
    counts: ImportCounts,
}

/// A platform time: RFC 3339, or ISO 8601 with no offset, which the platform writes in UTC.
fn platform_time(text: &str) -> Option<DateTime<Utc>> {
    if let Ok(t) = DateTime::parse_from_rfc3339(text) {
        return Some(t.with_timezone(&Utc));
    }
    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .map(|t| t.and_utc())
}

/// The time of field on row, or a Row error naming it.
fn time(row: &str, field: &str, text: &str) -> Result<DateTime<Utc>, ImportError> {
    platform_time(text).ok_or_else(|| ImportError::Row(row.into(), format!("{field} {text}")))
}

impl Importer {
    /// An importer into pool for tenant_id. Conversations last updated before cutoff are left out.
    pub fn new(pool: SqlitePool, seal: Seal, tenant_id: &str, cutoff: DateTime<Utc>) -> Self {
        Self {
            pool,
            seal,
            tenant_id: tenant_id.to_owned(),
            cutoff,
            counts: ImportCounts::default(),
        }
    }

    /// Refuses a source imported before unless again is true.
    pub async fn begin(&self, source: &str, again: bool) -> Result<(), ImportError> {
        let done: Option<(String,)> =
            sqlx::query_as("SELECT finished_at FROM imports WHERE source = ?")
                .bind(source)
                .fetch_optional(&self.pool)
                .await
                .map_err(database)?;
        match done {
            Some((at,)) if !again => Err(ImportError::Done(source.to_owned(), at)),
            _ => Ok(()),
        }
    }

    /// Writes one page of conversations with their messages.
    pub async fn conversations(&mut self, page: &ConversationPage) -> Result<(), ImportError> {
        let mut tx = self.pool.begin().await.map_err(database)?;
        for conversation in &page.conversations {
            let updated = time(&conversation.id, "updated_at", &conversation.updated_at)?;
            if updated < self.cutoff || !self.conversation(&mut tx, conversation).await? {
                self.counts.skipped += 1;
                continue;
            }
            self.counts.conversations += 1;
            self.messages(&mut tx, conversation).await?;
        }
        tx.commit().await.map_err(database)?;
        Ok(())
    }

    /// Inserts the conversation row; false if its id is already in the file.
    async fn conversation(
        &self,
        conn: &mut SqliteConnection,
        c: &ExportConversation,
    ) -> Result<bool, ImportError> {
        let id =
            Uuid::parse_str(&c.id).map_err(|e| ImportError::Row(c.id.clone(), e.to_string()))?;
        if !matches!(c.visibility.as_str(), "public" | "private") {
            return Err(ImportError::Row(
                c.id.clone(),
                format!("visibility {}", c.visibility),
            ));
        }
        let ended = c
            .ended_at
            .as_deref()
            .map(|t| time(&c.id, "ended_at", t))
            .transpose()?;
        let done = sqlx::query(
            "INSERT INTO conversations \
             (id, tenant_id, user_id, channel_id, visibility, created_at, updated_at, ended_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (id) DO NOTHING",
        )
        .bind(id.to_string())
        .bind(&self.tenant_id)
        .bind(&c.discord_id)
        .bind(&c.channel_id)
        .bind(&c.visibility)
        .bind(stamp(time(&c.id, "created_at", &c.created_at)?))
        .bind(stamp(time(&c.id, "updated_at", &c.updated_at)?))
        .bind(ended.map(stamp))
        .execute(&mut *conn)
        .await
        .map_err(database)?;
        Ok(done.rows_affected() == 1)
    }

    /// Inserts the messages of c in platform seq order, rewriting each summary covers.
    async fn messages(
        &mut self,
        conn: &mut SqliteConnection,
        c: &ExportConversation,
    ) -> Result<(), ImportError> {
        let mut messages: Vec<_> = c.messages.iter().collect();
        messages.sort_by_key(|m| m.seq);
        // Platform seq to the seq this file gave the same message.
        let mut seqs = BTreeMap::new();
        for m in messages {
            let covers = match m.covers_seq {
                // A summary covers the newest imported message at or before its platform seq.
                Some(old) => seqs.range(..=old).next_back().map(|(_, new)| *new),
                None => None,
            };
            let content = serde_json::to_string(&m.content)
                .map_err(|e| ImportError::Row(c.id.clone(), e.to_string()))?;
            let at = time(&c.id, "message created_at", &m.created_at)?;
            let (seq,): (i64,) = sqlx::query_as(
                "INSERT INTO messages (conversation_id, role, content, covers, created_at) \
                 VALUES (?, ?, ?, ?, ?) RETURNING seq",
            )
            .bind(&c.id)
            .bind(&m.role)
            .bind(content)
            .bind(covers)
            .bind(stamp(at))
            .fetch_one(&mut *conn)
            .await
            .map_err(database)?;
            seqs.insert(m.seq, seq);
            self.counts.messages += 1;
        }
        Ok(())
    }

    /// Writes one page of memories, sealing the ones the platform marks sensitive.
    pub async fn memories(&mut self, page: &MemoryPage) -> Result<(), ImportError> {
        let mut tx = self.pool.begin().await.map_err(database)?;
        for m in &page.memories {
            let row = |e: String| ImportError::Row(m.id.clone(), e);
            let kind = MemoryKind::from_platform(&m.kind)
                .ok_or_else(|| row(format!("kind {}", m.kind)))?;
            let memory = MemoryRow {
                id: Uuid::parse_str(&m.id).map_err(|e| row(e.to_string()))?,
                kind,
                content: m.content.clone(),
                sensitive: m.sensitivity == "sensitive",
                source_conversation: None,
                source_seq: None,
                created_at: time(&m.id, "created_at", &m.created_at)?,
                expires_at: m
                    .expires_at
                    .as_deref()
                    .map(|t| time(&m.id, "expires_at", t))
                    .transpose()?,
            };
            if insert(&mut tx, &self.seal, &self.tenant_id, &m.discord_id, &memory).await? {
                self.counts.memories += 1;
            }
        }
        tx.commit().await.map_err(database)?;
        Ok(())
    }

    /// Records the import of source and returns what it wrote.
    pub async fn finish(self, source: &str) -> Result<ImportCounts, ImportError> {
        let to_i64 = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
        sqlx::query(
            "INSERT INTO imports (source, finished_at, conversations, messages, memories) \
             VALUES (?, ?, ?, ?, ?) ON CONFLICT (source) DO UPDATE SET \
             finished_at = excluded.finished_at, conversations = excluded.conversations, \
             messages = excluded.messages, memories = excluded.memories",
        )
        .bind(source)
        .bind(stamp(Utc::now()))
        .bind(to_i64(self.counts.conversations))
        .bind(to_i64(self.counts.messages))
        .bind(to_i64(self.counts.memories))
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(self.counts)
    }
}
