//! Pending actions in the SQLite file: held once, claimed once.

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{database, stamp};
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::safety::policy::PendingAction;
use crate::core::types::store::StoreError;

/// Actions waiting on approval, held in the SQLite file.
pub struct SqliteConfirmations {
    pool: SqlitePool,
}

impl SqliteConfirmations {
    /// Wraps a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConfirmationStore for SqliteConfirmations {
    async fn hold(
        &self,
        ctx: &RequestContext,
        token: Uuid,
        pending: &PendingAction,
        payload_hash: &str,
        ttl: Duration,
    ) -> Result<(), StoreError> {
        let now = Utc::now();
        let ttl =
            chrono::Duration::from_std(ttl).map_err(|e| StoreError::Database(e.to_string()))?;
        let action =
            serde_json::to_string(pending).map_err(|e| StoreError::Database(e.to_string()))?;
        sqlx::query(
            "INSERT INTO pending_actions \
             (token, tenant_id, user_id, action, payload_hash, status, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)",
        )
        .bind(token.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(action)
        .bind(payload_hash)
        .bind(stamp(now))
        .bind(stamp(now + ttl))
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(())
    }

    async fn claim(
        &self,
        ctx: &RequestContext,
        token: Uuid,
        approved: bool,
    ) -> Result<Option<PendingAction>, StoreError> {
        let now = stamp(Utc::now());
        let status = if approved { "confirmed" } else { "denied" };
        // One statement, so a token is answered once.
        let row = sqlx::query(
            "UPDATE pending_actions SET status = ?, resolved_at = ? \
             WHERE token = ? AND tenant_id = ? AND user_id = ? AND status = 'pending' \
             AND expires_at > ? RETURNING action",
        )
        .bind(status)
        .bind(&now)
        .bind(token.to_string())
        .bind(&ctx.tenant_id)
        .bind(&ctx.user_id)
        .bind(&now)
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let action: String = row.try_get("action").map_err(database)?;
        serde_json::from_str(&action)
            .map(Some)
            .map_err(|e| StoreError::Database(format!("pending action: {e}")))
    }
}
