//! Adapters over one SQLite file: conversations, memories, pending actions, Canvas feed links, reminders, nudges and admin changes.
//!
//! Every query binds tenant_id and user_id from the request; only the background workers read across users.

mod admin;
mod backup;
mod canvas;
mod confirmation;
mod conversation;
mod health;
mod home;
mod import;
mod memory;
mod nudge;
mod prune;
mod reminder;
mod seal;

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

use crate::core::types::store::StoreError;

pub use admin::SqliteAdmin;
#[cfg(test)]
pub use backup::backup_once;
pub use backup::{BackupPlan, run_backups};
pub use canvas::SqliteCanvasFeeds;
pub use confirmation::SqliteConfirmations;
pub use conversation::SqliteConversations;
pub use health::SqliteProbe;
#[cfg(test)]
pub use import::ImportError;
pub use import::{ImportCounts, Importer};
pub use memory::{MemoryLimits, SqliteMemory};
pub use nudge::SqliteNudges;
pub use prune::{PrunePlan, run_prunes};
#[cfg(test)]
pub use prune::{Pruned, prune_once};
pub use reminder::{ReminderLimits, SqliteReminders, run_reminders};
pub use seal::Seal;

/// How to open the file.
#[derive(Debug, Clone)]
pub struct StoreOptions {
    /// Path of the file, or sqlite::memory: for a private in-memory database.
    pub path: String,
    /// Pool size.
    pub max_connections: u32,
    /// How long a write waits on a lock.
    pub busy_timeout: Duration,
}

/// One open SQLite file with its migrations applied.
#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Opens or creates the file, creates its parent folder, and runs the embedded migrations.
    pub async fn open(options: &StoreOptions) -> Result<Self, StoreError> {
        let memory = options.path == "sqlite::memory:";
        let connect = if memory {
            SqliteConnectOptions::from_str("sqlite::memory:").map_err(database)?
        } else {
            if let Some(parent) = Path::new(&options.path).parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)
                    .map_err(|e| StoreError::Database(format!("{}: {e}", parent.display())))?;
            }
            SqliteConnectOptions::new()
                .filename(&options.path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
        };
        let connect = connect
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(options.busy_timeout);
        // An in-memory database lives in one connection.
        let max = if memory { 1 } else { options.max_connections };
        let pool = SqlitePoolOptions::new()
            .max_connections(max)
            .connect_with(connect)
            .await
            .map_err(database)?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(Self { pool })
    }

    /// The pool, for the adapters.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Conversations in this file.
    pub fn conversations(&self) -> SqliteConversations {
        SqliteConversations::new(self.pool.clone())
    }

    /// Memories in this file, sealed and opened with seal, kept within limits.
    pub fn memory(&self, seal: Seal, limits: MemoryLimits) -> SqliteMemory {
        SqliteMemory::new(self.pool.clone(), seal, limits)
    }

    /// Pending actions in this file.
    pub fn confirmations(&self) -> SqliteConfirmations {
        SqliteConfirmations::new(self.pool.clone())
    }

    /// Canvas calendar feed links in this file, sealed and opened with seal.
    pub fn canvas_feeds(&self, seal: Seal) -> SqliteCanvasFeeds {
        SqliteCanvasFeeds::new(self.pool.clone(), seal)
    }

    /// Reminders and notifications in this file, sealed and opened with seal.
    pub fn reminders(&self, seal: Seal, limits: ReminderLimits) -> SqliteReminders {
        SqliteReminders::new(self.pool.clone(), seal, limits)
    }

    /// Nudges in this file, their text sealed with seal.
    pub fn nudges(&self, seal: Seal) -> SqliteNudges {
        SqliteNudges::new(self.pool.clone(), seal)
    }

    /// Admin changes and held actions in this file.
    pub fn admin(&self) -> SqliteAdmin {
        SqliteAdmin::new(self.pool.clone())
    }

    /// Readiness of this file.
    pub fn probe(&self) -> SqliteProbe {
        SqliteProbe::new(self.pool.clone())
    }
}

/// Maps a driver error to a StoreError.
#[allow(clippy::needless_pass_by_value)]
fn database(e: sqlx::Error) -> StoreError {
    StoreError::Database(e.to_string())
}

/// A time as stored: RFC 3339 UTC with microseconds, so text order is time order.
fn stamp(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Micros, true)
}

/// A stored time.
fn parse_stamp(text: &str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(text)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| StoreError::Database(format!("time {text}: {e}")))
}
