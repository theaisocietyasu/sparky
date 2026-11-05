//! Nudges in the SQLite file: what was said, what each student set, and the notifications they become.

use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{Seal, database, stamp};
use crate::core::traits::nudge::NudgeStore;
use crate::core::types::nudge::{Nudge, NudgeKind, NudgePrefs, Person};
use crate::core::types::store::StoreError;

/// Nudges held in the SQLite file, their text sealed like reminders.
pub struct SqliteNudges {
    pool: SqlitePool,
    seal: Seal,
}

impl SqliteNudges {
    /// Wraps a pool and the seal notification text is kept under.
    pub fn new(pool: SqlitePool, seal: Seal) -> Self {
        Self { pool, seal }
    }
}

/// The off column: kind values joined by commas.
fn off_column(off: &[NudgeKind]) -> String {
    off.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(",")
}

/// Hour columns as stored, out of range values moved to 0.
fn hour(value: i64) -> u8 {
    u8::try_from(value).ok().filter(|h| *h < 24).unwrap_or(0)
}

#[async_trait]
impl NudgeStore for SqliteNudges {
    async fn people(&self, since: DateTime<Utc>) -> Result<Vec<Person>, StoreError> {
        let rows = sqlx::query(
            "SELECT tenant_id, user_id, max(updated_at) AS latest FROM conversations \
             WHERE visibility = 'private' AND updated_at >= ? \
             GROUP BY tenant_id, user_id ORDER BY latest DESC",
        )
        .bind(stamp(since))
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        // Newest first, so the first tenant seen for a user is their latest.
        let mut seen: HashSet<String> = HashSet::new();
        let mut out = Vec::new();
        for row in &rows {
            let user: String = row.try_get("user_id").map_err(database)?;
            if !seen.insert(user.clone()) {
                continue;
            }
            out.push(Person {
                tenant: row.try_get("tenant_id").map_err(database)?,
                user,
            });
        }
        Ok(out)
    }

    async fn prefs(&self, who: &Person) -> Result<Option<NudgePrefs>, StoreError> {
        let row = sqlx::query(
            "SELECT quiet_start, quiet_end, off FROM nudge_prefs WHERE tenant_id = ? AND user_id = ?",
        )
        .bind(&who.tenant)
        .bind(&who.user)
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        let Some(row) = row else { return Ok(None) };
        let off: String = row.try_get("off").map_err(database)?;
        Ok(Some(NudgePrefs {
            quiet_start: hour(row.try_get("quiet_start").map_err(database)?),
            quiet_end: hour(row.try_get("quiet_end").map_err(database)?),
            off: off.split(',').filter_map(NudgeKind::parse).collect(),
        }))
    }

    async fn set_prefs(&self, who: &Person, prefs: &NudgePrefs) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO nudge_prefs (tenant_id, user_id, quiet_start, quiet_end, off, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT (tenant_id, user_id) DO UPDATE SET quiet_start = excluded.quiet_start, \
             quiet_end = excluded.quiet_end, off = excluded.off, updated_at = excluded.updated_at",
        )
        .bind(&who.tenant)
        .bind(&who.user)
        .bind(i64::from(prefs.quiet_start))
        .bind(i64::from(prefs.quiet_end))
        .bind(off_column(&prefs.off))
        .bind(stamp(Utc::now()))
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(())
    }

    async fn sent_keys(&self, who: &Person, keys: &[String]) -> Result<Vec<String>, StoreError> {
        let mut found = Vec::new();
        for key in keys {
            let hit = sqlx::query(
                "SELECT 1 FROM nudges_sent WHERE tenant_id = ? AND user_id = ? AND key = ?",
            )
            .bind(&who.tenant)
            .bind(&who.user)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(database)?;
            if hit.is_some() {
                found.push(key.clone());
            }
        }
        Ok(found)
    }

    async fn mark(&self, who: &Person, kind: NudgeKind, keys: &[String]) -> Result<(), StoreError> {
        let now = stamp(Utc::now());
        let mut tx = self.pool.begin().await.map_err(database)?;
        for key in keys {
            sqlx::query(
                "INSERT OR IGNORE INTO nudges_sent (tenant_id, user_id, key, kind, created_at) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&who.tenant)
            .bind(&who.user)
            .bind(key)
            .bind(kind.as_str())
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(database)?;
        }
        tx.commit().await.map_err(database)
    }

    async fn count_since(&self, who: &Person, since: DateTime<Utc>) -> Result<usize, StoreError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notifications WHERE tenant_id = ? AND user_id = ? \
             AND kind != 'reminder' AND created_at >= ?",
        )
        .bind(&who.tenant)
        .bind(&who.user)
        .bind(stamp(since))
        .fetch_one(&self.pool)
        .await
        .map_err(database)?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    async fn recent(
        &self,
        who: &Person,
        kind: NudgeKind,
        limit: usize,
    ) -> Result<Vec<String>, StoreError> {
        let rows = sqlx::query(
            "SELECT nonce, sealed FROM notifications WHERE tenant_id = ? AND user_id = ? \
             AND kind = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(&who.tenant)
        .bind(&who.user)
        .bind(kind.as_str())
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await
        .map_err(database)?;
        Ok(rows
            .iter()
            .filter_map(|row| {
                let nonce: Vec<u8> = row.try_get("nonce").ok()?;
                let sealed: Vec<u8> = row.try_get("sealed").ok()?;
                self.seal.open(&nonce, &sealed)
            })
            .collect())
    }

    async fn send(&self, who: &Person, nudge: &Nudge) -> Result<(), StoreError> {
        let Some((nonce, sealed)) = self.seal.seal(&nudge.text) else {
            return Err(StoreError::Database("the nudge could not be sealed".into()));
        };
        let now = stamp(Utc::now());
        let mut tx = self.pool.begin().await.map_err(database)?;
        sqlx::query(
            "INSERT INTO notifications (id, tenant_id, user_id, nonce, sealed, created_at, kind) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().simple().to_string())
        .bind(&who.tenant)
        .bind(&who.user)
        .bind(nonce)
        .bind(sealed)
        .bind(&now)
        .bind(nudge.kind.as_str())
        .execute(&mut *tx)
        .await
        .map_err(database)?;
        for key in &nudge.keys {
            sqlx::query(
                "INSERT OR IGNORE INTO nudges_sent (tenant_id, user_id, key, kind, created_at) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&who.tenant)
            .bind(&who.user)
            .bind(key)
            .bind(nudge.kind.as_str())
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(database)?;
        }
        tx.commit().await.map_err(database)
    }

    async fn feedback(
        &self,
        who: &Person,
        id: &str,
        helpful: bool,
    ) -> Result<Option<NudgeKind>, StoreError> {
        let kind: Option<String> = sqlx::query_scalar(
            "UPDATE notifications SET helpful = ? WHERE id = ? AND tenant_id = ? AND user_id = ? \
             RETURNING kind",
        )
        .bind(i64::from(helpful))
        .bind(id)
        .bind(&who.tenant)
        .bind(&who.user)
        .fetch_optional(&self.pool)
        .await
        .map_err(database)?;
        Ok(kind.as_deref().and_then(NudgeKind::parse))
    }

    async fn prune(&self, before: DateTime<Utc>) -> Result<u64, StoreError> {
        let done = sqlx::query("DELETE FROM nudges_sent WHERE created_at < ?")
            .bind(stamp(before))
            .execute(&self.pool)
            .await
            .map_err(database)?;
        Ok(done.rows_affected())
    }
}
