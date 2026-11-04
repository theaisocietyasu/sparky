//! Consistent copies of the SQLite file into a folder, newest kept.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use sqlx::SqlitePool;

use super::database;
use crate::core::types::store::StoreError;

/// File name prefix of a backup copy.
const PREFIX: &str = "sparky-";

/// Where and how often to copy.
#[derive(Debug, Clone)]
pub struct BackupPlan {
    /// Folder the copies go into.
    pub dir: PathBuf,
    /// Time between copies.
    pub every: Duration,
    /// Newest copies kept.
    pub kept: usize,
}

/// Writes one copy with VACUUM INTO, then removes all but the newest plan.kept. Returns the copy.
pub async fn backup_once(pool: &SqlitePool, plan: &BackupPlan) -> Result<PathBuf, StoreError> {
    std::fs::create_dir_all(&plan.dir)
        .map_err(|e| StoreError::Database(format!("{}: {e}", plan.dir.display())))?;
    let name = format!("{PREFIX}{}.db", Utc::now().format("%Y%m%dT%H%M%SZ"));
    let target = plan.dir.join(name);
    // VACUUM INTO refuses an existing file.
    if target.exists() {
        std::fs::remove_file(&target)
            .map_err(|e| StoreError::Database(format!("{}: {e}", target.display())))?;
    }
    sqlx::query("VACUUM INTO ?")
        .bind(target.to_string_lossy().into_owned())
        .execute(pool)
        .await
        .map_err(database)?;
    prune(&plan.dir, plan.kept)?;
    Ok(target)
}

/// Removes every backup copy in dir but the newest kept.
fn prune(dir: &Path, kept: usize) -> Result<(), StoreError> {
    let read = std::fs::read_dir(dir)
        .map_err(|e| StoreError::Database(format!("{}: {e}", dir.display())))?;
    let mut copies: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let named = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(PREFIX));
            named && path.extension().is_some_and(|e| e == "db")
        })
        .collect();
    // The names sort by time.
    copies.sort();
    let extra = copies.len().saturating_sub(kept);
    for old in copies.into_iter().take(extra) {
        std::fs::remove_file(&old)
            .map_err(|e| StoreError::Database(format!("{}: {e}", old.display())))?;
    }
    Ok(())
}

/// Copies the file every plan.every until the process ends. A failure is logged and retried.
pub async fn run_backups(pool: SqlitePool, plan: BackupPlan) {
    let mut ticks = tokio::time::interval(plan.every);
    loop {
        ticks.tick().await;
        match backup_once(&pool, &plan).await {
            Ok(copy) => tracing::info!(path = %copy.display(), "store backup written"),
            Err(error) => tracing::error!(%error, "store backup failed"),
        }
    }
}
