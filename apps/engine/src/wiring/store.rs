//! Local store composition: conversations, memories and pending actions in the SQLite file.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::Stores;
use crate::core::config::{Config, StoreBackend};
use crate::stores::sqlite::{BackupPlan, Seal, SqliteStore, StoreOptions, run_backups};

/// Opens the SQLite file named by the store section and runs its migrations.
pub(super) async fn open(cfg: &Config) -> anyhow::Result<SqliteStore> {
    let options = StoreOptions {
        path: cfg.store.path.clone(),
        max_connections: cfg.store.max_connections,
        busy_timeout: Duration::from_millis(cfg.store.busy_timeout_ms),
    };
    SqliteStore::open(&options)
        .await
        .map_err(|e| anyhow::anyhow!("store {}: {e}", cfg.store.path))
}

/// The seal over the memory key. Config::validate guarantees the key.
pub(super) fn seal(cfg: &Config) -> anyhow::Result<Seal> {
    let key = cfg.memory.bytes()?;
    Ok(Seal::new(&key))
}

/// With store.backend sqlite, moves conversations, memories and pending actions to the file.
pub(super) async fn localize(cfg: &Config, stores: &mut Stores) -> anyhow::Result<()> {
    if cfg.store.backend == StoreBackend::Platform {
        tracing::warn!(
            "store.backend is platform; conversations and memories stay on the platform"
        );
        return Ok(());
    }
    let store = open(cfg).await?;
    stores.conversations = Arc::new(store.conversations());
    stores.memory = Arc::new(store.memory(seal(cfg)?));
    stores.confirmations = Arc::new(store.confirmations());
    stores.probes.push(Arc::new(store.probe()));
    let dir = cfg.store.backup_dir.trim();
    if !dir.is_empty() {
        let plan = BackupPlan {
            dir: PathBuf::from(dir),
            every: Duration::from_secs(cfg.store.backup_every_hours * 3600),
            kept: cfg.store.backups_kept,
        };
        tokio::spawn(run_backups(store.pool().clone(), plan));
    }
    tracing::info!(path = %cfg.store.path, "conversations and memories are in the local store");
    Ok(())
}
