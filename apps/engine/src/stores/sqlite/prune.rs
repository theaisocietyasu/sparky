//! Deletes what the store no longer keeps: expired memories, settled confirmations, idle conversations.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use super::{database, stamp};
use crate::core::types::store::StoreError;

/// How often the prune runs and how long an idle conversation is kept.
#[derive(Debug, Clone, Copy)]
pub struct PrunePlan {
    /// Wait between runs.
    pub every: Duration,
    /// Days a conversation may sit idle; None keeps every conversation.
    pub idle_days: Option<u32>,
}

/// Rows one prune deleted.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pruned {
    /// Conversations idle past the limit, with their messages.
    pub conversations: u64,
    /// Memories past expires_at.
    pub memories: u64,
    /// Pending actions answered or past expires_at.
    pub pending_actions: u64,
}

/// One prune as of now.
pub async fn prune_once(
    pool: &SqlitePool,
    now: DateTime<Utc>,
    idle_days: Option<u32>,
) -> Result<Pruned, StoreError> {
    let at = stamp(now);
    let memories =
        sqlx::query("DELETE FROM memories WHERE expires_at IS NOT NULL AND expires_at <= ?")
            .bind(&at)
            .execute(pool)
            .await
            .map_err(database)?
            .rows_affected();
    let pending_actions =
        sqlx::query("DELETE FROM pending_actions WHERE status != 'pending' OR expires_at <= ?")
            .bind(&at)
            .execute(pool)
            .await
            .map_err(database)?
            .rows_affected();
    let conversations = match idle_days {
        Some(days) => {
            let cutoff = stamp(now - chrono::Duration::days(i64::from(days)));
            sqlx::query("DELETE FROM conversations WHERE updated_at < ?")
                .bind(cutoff)
                .execute(pool)
                .await
                .map_err(database)?
                .rows_affected()
        }
        None => 0,
    };
    Ok(Pruned {
        conversations,
        memories,
        pending_actions,
    })
}

/// Prunes now and then once per plan.every, logging what each run deleted.
pub async fn run_prunes(pool: SqlitePool, plan: PrunePlan) {
    let mut tick = tokio::time::interval(plan.every);
    loop {
        tick.tick().await;
        match prune_once(&pool, Utc::now(), plan.idle_days).await {
            Ok(done) => tracing::info!(
                conversations = done.conversations,
                memories = done.memories,
                pending_actions = done.pending_actions,
                "store pruned"
            ),
            Err(error) => tracing::warn!(%error, "store prune failed"),
        }
    }
}
