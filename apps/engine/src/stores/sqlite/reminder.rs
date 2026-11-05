//! Reminders and notifications in the SQLite file, their text sealed at rest.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{Seal, database, parse_stamp, stamp};
use crate::core::traits::reminder::ReminderStore;
use crate::core::types::reminder::{NewReminder, Notification, Outgoing, Reminder, Repeat};
use crate::core::types::store::StoreError;

/// Most reminders one fire call moves on.
const FIRE_BATCH: i64 = 500;

/// How many reminders a student may keep and how long notifications stay.
#[derive(Debug, Clone, Copy)]
pub struct ReminderLimits {
    /// Most reminders one student keeps at once.
    pub max_per_user: usize,
    /// Days a notification stays after it is made.
    pub keep_days: u32,
}

/// Reminders and notifications held in the SQLite file.
pub struct SqliteReminders {
    pool: SqlitePool,
    seal: Seal,
    limits: ReminderLimits,
}

impl SqliteReminders {
    /// Wraps a pool, the seal the text is kept under, and the limits.
    pub fn new(pool: SqlitePool, seal: Seal, limits: ReminderLimits) -> Self {
        Self { pool, seal, limits }
    }

    /// The text of a row, or a placeholder when the seal does not open it.
    fn open(&self, row: &SqliteRow) -> Result<String, StoreError> {
        let nonce: Vec<u8> = row.try_get("nonce").map_err(database)?;
        let sealed: Vec<u8> = row.try_get("sealed").map_err(database)?;
        Ok(self.seal.open(&nonce, &sealed).unwrap_or_else(|| {
            tracing::warn!("a sealed reminder does not open");
            "(a reminder that could not be read)".to_owned()
        }))
    }
}

#[async_trait]
impl ReminderStore for SqliteReminders {
    async fn add(
        &self,
        tenant: &str,
        user: &str,
        reminder: &NewReminder,
    ) -> Result<Option<Reminder>, StoreError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM reminders WHERE tenant_id = ? AND user_id = ?",
        )
        .bind(tenant)
        .bind(user)
        .fetch_one(&self.pool)
        .await
        .map_err(database)?;
        if usize::try_from(count).unwrap_or(usize::MAX) >= self.limits.max_per_user {
            return Ok(None);
        }
        let Some((nonce, sealed)) = self.seal.seal(&reminder.text) else {
            return Err(StoreError::Database(
                "the reminder could not be sealed".into(),
            ));
        };
        let id = Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO reminders (id, tenant_id, user_id, nonce, sealed, due_at, repeat, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(tenant)
        .bind(user)
        .bind(nonce)
        .bind(sealed)
        .bind(stamp(reminder.due_at))
        .bind(reminder.repeat.as_str())
        .bind(stamp(Utc::now()))
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(Some(Reminder {
            id,
            text: reminder.text.clone(),
            due_at: reminder.due_at,
            repeat: reminder.repeat,
        }))
    }

    async fn list(&self, tenant: &str, user: &str) -> Result<Vec<Reminder>, StoreError> {
        let rows = sqlx::query(
            "SELECT id, nonce, sealed, due_at, repeat FROM reminders \
             WHERE tenant_id = ? AND user_id = ? ORDER BY due_at",
        )
        .bind(tenant)
        .bind(user)
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        rows.iter()
            .map(|row| {
                let due: String = row.try_get("due_at").map_err(database)?;
                let repeat: String = row.try_get("repeat").map_err(database)?;
                Ok(Reminder {
                    id: row.try_get("id").map_err(database)?,
                    text: self.open(row)?,
                    due_at: parse_stamp(&due)?,
                    repeat: Repeat::parse(&repeat).unwrap_or(Repeat::Once),
                })
            })
            .collect()
    }

    async fn cancel(&self, tenant: &str, user: &str, id: &str) -> Result<bool, StoreError> {
        let done =
            sqlx::query("DELETE FROM reminders WHERE tenant_id = ? AND user_id = ? AND id = ?")
                .bind(tenant)
                .bind(user)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(database)?;
        Ok(done.rows_affected() > 0)
    }

    async fn fire(&self, now: DateTime<Utc>) -> Result<usize, StoreError> {
        let mut tx = self.pool.begin().await.map_err(database)?;
        let due = sqlx::query(
            "SELECT id, tenant_id, user_id, nonce, sealed, due_at, repeat FROM reminders \
             WHERE due_at <= ? ORDER BY due_at LIMIT ?",
        )
        .bind(stamp(now))
        .bind(FIRE_BATCH)
        .fetch_all(&mut *tx)
        .await
        .map_err(database)?;
        for row in &due {
            let id: String = row.try_get("id").map_err(database)?;
            let nonce: Vec<u8> = row.try_get("nonce").map_err(database)?;
            let sealed: Vec<u8> = row.try_get("sealed").map_err(database)?;
            sqlx::query(
                "INSERT INTO notifications (id, tenant_id, user_id, nonce, sealed, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().simple().to_string())
            .bind(row.try_get::<String, _>("tenant_id").map_err(database)?)
            .bind(row.try_get::<String, _>("user_id").map_err(database)?)
            .bind(nonce)
            .bind(sealed)
            .bind(stamp(now))
            .execute(&mut *tx)
            .await
            .map_err(database)?;
            let at = parse_stamp(&row.try_get::<String, _>("due_at").map_err(database)?)?;
            let repeat = Repeat::parse(&row.try_get::<String, _>("repeat").map_err(database)?)
                .unwrap_or(Repeat::Once);
            match repeat.next_after(at, now) {
                Some(next) => sqlx::query("UPDATE reminders SET due_at = ? WHERE id = ?")
                    .bind(stamp(next))
                    .bind(&id),
                None => sqlx::query("DELETE FROM reminders WHERE id = ?").bind(&id),
            }
            .execute(&mut *tx)
            .await
            .map_err(database)?;
        }
        let cutoff = now - Duration::days(i64::from(self.limits.keep_days));
        sqlx::query("DELETE FROM notifications WHERE created_at < ?")
            .bind(stamp(cutoff))
            .execute(&mut *tx)
            .await
            .map_err(database)?;
        tx.commit().await.map_err(database)?;
        Ok(due.len())
    }

    async fn notifications(
        &self,
        tenant: &str,
        user: &str,
        limit: usize,
    ) -> Result<Vec<Notification>, StoreError> {
        let rows = sqlx::query(
            "SELECT id, nonce, sealed, created_at, read_at, kind, helpful FROM notifications \
             WHERE tenant_id = ? AND user_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(tenant)
        .bind(user)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        rows.iter()
            .map(|row| {
                let created: String = row.try_get("created_at").map_err(database)?;
                let read: Option<String> = row.try_get("read_at").map_err(database)?;
                let helpful: Option<i64> = row.try_get("helpful").map_err(database)?;
                Ok(Notification {
                    id: row.try_get("id").map_err(database)?,
                    text: self.open(row)?,
                    created_at: parse_stamp(&created)?,
                    read: read.is_some(),
                    kind: row.try_get("kind").map_err(database)?,
                    helpful: helpful.map(|h| h != 0),
                })
            })
            .collect()
    }

    async fn read_all(&self, tenant: &str, user: &str) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE notifications SET read_at = ? \
             WHERE tenant_id = ? AND user_id = ? AND read_at IS NULL",
        )
        .bind(stamp(Utc::now()))
        .bind(tenant)
        .bind(user)
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(())
    }

    async fn outgoing(&self, limit: usize) -> Result<Vec<Outgoing>, StoreError> {
        let rows = sqlx::query(
            "SELECT id, user_id, nonce, sealed FROM notifications \
             WHERE sent_at IS NULL ORDER BY created_at LIMIT ?",
        )
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        rows.iter()
            .map(|row| {
                Ok(Outgoing {
                    id: row.try_get("id").map_err(database)?,
                    user_id: row.try_get("user_id").map_err(database)?,
                    text: self.open(row)?,
                })
            })
            .collect()
    }

    async fn sent(&self, ids: &[String]) -> Result<(), StoreError> {
        let at = stamp(Utc::now());
        for id in ids {
            sqlx::query("UPDATE notifications SET sent_at = ? WHERE id = ? AND sent_at IS NULL")
                .bind(&at)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(database)?;
        }
        Ok(())
    }
}

/// Fires due reminders every tick until the process ends.
pub async fn run_reminders(store: std::sync::Arc<SqliteReminders>, every: std::time::Duration) {
    let mut tick = tokio::time::interval(every);
    loop {
        tick.tick().await;
        match store.fire(Utc::now()).await {
            Ok(0) => {}
            Ok(fired) => tracing::info!(fired, "reminders fired"),
            Err(error) => tracing::error!(%error, "could not fire reminders"),
        }
    }
}
