//! Admin changes and the held actions a moderator sees, in the SQLite file.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use super::{database, stamp};
use crate::core::traits::admin::AdminStore;
use crate::core::types::admin::{AdminChange, ChangeKind, HeldAction, NewChange};
use crate::core::types::store::StoreError;

/// Admin changes held in the SQLite file.
pub struct SqliteAdmin {
    pool: SqlitePool,
}

impl SqliteAdmin {
    /// Wraps a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

/// A stored change.
fn change(row: &SqliteRow) -> Result<AdminChange, StoreError> {
    let kind: String = row.try_get("kind").map_err(database)?;
    let value: String = row.try_get("value").map_err(database)?;
    Ok(AdminChange {
        id: row.try_get("id").map_err(database)?,
        kind: ChangeKind::parse(&kind)
            .ok_or_else(|| StoreError::Database(format!("admin change kind {kind}")))?,
        key: row.try_get("key").map_err(database)?,
        value: serde_json::from_str(&value)
            .map_err(|e| StoreError::Database(format!("admin change value: {e}")))?,
        actor: row.try_get("actor_user_id").map_err(database)?,
        created_at: row.try_get("created_at").map_err(database)?,
    })
}

const COLUMNS: &str = "id, kind, key, value, actor_user_id, created_at";

#[async_trait]
impl AdminStore for SqliteAdmin {
    async fn record(&self, new: NewChange) -> Result<AdminChange, StoreError> {
        let value =
            serde_json::to_string(&new.value).map_err(|e| StoreError::Database(e.to_string()))?;
        let row = sqlx::query(&format!(
            "INSERT INTO admin_changes (tenant_id, kind, key, value, actor_user_id, created_at) \
             VALUES (?, ?, ?, ?, ?, ?) RETURNING {COLUMNS}"
        ))
        .bind(&new.tenant_id)
        .bind(new.kind.as_str())
        .bind(&new.key)
        .bind(value)
        .bind(&new.actor)
        .bind(stamp(Utc::now()))
        .fetch_one(&self.pool)
        .await
        .map_err(database)?;
        change(&row)
    }

    async fn current(&self, tenant: &str) -> Result<Vec<AdminChange>, StoreError> {
        let rows = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM admin_changes WHERE id IN \
             (SELECT MAX(id) FROM admin_changes WHERE tenant_id = ? GROUP BY kind, key) \
             ORDER BY kind, key"
        ))
        .bind(tenant)
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        rows.iter().map(change).collect()
    }

    async fn history(&self, tenant: &str, limit: usize) -> Result<Vec<AdminChange>, StoreError> {
        let rows = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM admin_changes WHERE tenant_id = ? ORDER BY id DESC LIMIT ?"
        ))
        .bind(tenant)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        rows.iter().map(change).collect()
    }

    async fn change(&self, tenant: &str, id: i64) -> Result<Option<AdminChange>, StoreError> {
        let row = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM admin_changes WHERE tenant_id = ? AND id = ?"
        ))
        .bind(tenant)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        row.as_ref().map(change).transpose()
    }

    async fn held(&self, tenant: &str) -> Result<Vec<HeldAction>, StoreError> {
        let rows = sqlx::query(
            "SELECT token, user_id, json_extract(action, '$.action.tool') AS tool, created_at, expires_at \
             FROM pending_actions WHERE tenant_id = ? AND status = 'pending' AND expires_at > ? \
             ORDER BY created_at",
        )
        .bind(tenant)
        .bind(stamp(Utc::now()))
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        rows.iter()
            .map(|row| {
                Ok(HeldAction {
                    token: row.try_get("token").map_err(database)?,
                    user_id: row.try_get("user_id").map_err(database)?,
                    tool: row
                        .try_get::<Option<String>, _>("tool")
                        .map_err(database)?
                        .unwrap_or_default(),
                    created_at: row.try_get("created_at").map_err(database)?,
                    expires_at: row.try_get("expires_at").map_err(database)?,
                })
            })
            .collect()
    }

    async fn deny(&self, tenant: &str, token: &str) -> Result<bool, StoreError> {
        let now = stamp(Utc::now());
        let done = sqlx::query(
            "UPDATE pending_actions SET status = 'denied', resolved_at = ? \
             WHERE token = ? AND tenant_id = ? AND status = 'pending' AND expires_at > ?",
        )
        .bind(&now)
        .bind(token)
        .bind(tenant)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(done.rows_affected() == 1)
    }
}
