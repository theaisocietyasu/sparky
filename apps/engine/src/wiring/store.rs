//! Store composition: conversations, memories, pending actions, Canvas feed links, reminders and nudges in the SQLite file, beside the platform adapters.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::Stores;
use super::platform::Platform;
use crate::core::config::Config;
use crate::stores::sqlite::{
    BackupPlan, MemoryLimits, PrunePlan, ReminderLimits, Seal, SqliteStore, StoreOptions,
    run_backups, run_prunes, run_reminders,
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

/// Opens the SQLite file, starts its background loops, and gathers every store with the platform's.
pub(super) async fn stores(cfg: &Config, platform: Platform) -> anyhow::Result<Stores> {
    let store = open(cfg).await?;
    let limits = MemoryLimits {
        max_per_user: cfg.memory.max_per_user,
        task_soon_days: cfg.memory.task_soon_days,
        utc_offset_hours: cfg.prompt.utc_offset_hours,
    };
    let mut stores = Stores {
        queries: platform.queries,
        conversations: Arc::new(store.conversations()),
        memory: Arc::new(store.memory(seal(cfg)?, limits)),
        confirmations: Arc::new(store.confirmations()),
        canvas: None,
        reminders: None,
        nudges: None,
        admin: Some(Arc::new(store.admin())),
        discovery: platform.discovery,
        probes: vec![platform.probe, Arc::new(store.probe())],
    };
    if cfg.canvas.enabled {
        stores.canvas = Some(Arc::new(store.canvas_feeds(seal(cfg)?)));
    }
    if cfg.reminders.enabled {
        let limits = ReminderLimits {
            max_per_user: cfg.reminders.max_per_user,
            keep_days: cfg.reminders.keep_days,
        };
        let reminders = Arc::new(store.reminders(seal(cfg)?, limits));
        tokio::spawn(run_reminders(
            Arc::clone(&reminders),
            Duration::from_secs(cfg.reminders.tick_secs),
        ));
        stores.reminders = Some(reminders);
        if cfg.nudges.enabled {
            stores.nudges = Some(Arc::new(store.nudges(seal(cfg)?)));
        }
    }
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
    Ok(stores)
}
