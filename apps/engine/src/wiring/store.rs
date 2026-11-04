//! Local store composition: conversations, memories, pending actions and Canvas feed links in the SQLite file.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::Stores;
use crate::core::config::{Config, StoreBackend};
use crate::stores::sqlite::{
    BackupPlan, MemoryLimits, PrunePlan, Seal, SqliteStore, StoreOptions, run_backups, run_prunes,
};

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

/// With store.backend sqlite, moves conversations, memories, pending actions and feed links to the file.
pub(super) async fn localize(cfg: &Config, stores: &mut Stores) -> anyhow::Result<()> {
    if cfg.store.backend == StoreBackend::Platform {
        tracing::warn!(
            "store.backend is platform; conversations and memories stay on the platform and Canvas is off"
        );
        return Ok(());
    }
    let store = open(cfg).await?;
    stores.conversations = Arc::new(store.conversations());
    let limits = MemoryLimits {
        max_per_user: cfg.memory.max_per_user,
        task_soon_days: cfg.memory.task_soon_days,
        utc_offset_hours: cfg.prompt.utc_offset_hours,
    };
    stores.memory = Arc::new(store.memory(seal(cfg)?, limits));
    if cfg.canvas.enabled {
        stores.canvas = Some(Arc::new(store.canvas_feeds(seal(cfg)?)));
    }
    stores.confirmations = Arc::new(store.confirmations());
    stores.admin = Some(Arc::new(store.admin()));
    stores.probes.push(Arc::new(store.probe()));
    let prune = PrunePlan {
        every: Duration::from_secs(cfg.store.prune_every_hours * 3600),
        idle_days: (cfg.store.conversation_idle_days > 0)
            .then_some(cfg.store.conversation_idle_days),
    };
    tokio::spawn(run_prunes(store.pool().clone(), prune));
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
