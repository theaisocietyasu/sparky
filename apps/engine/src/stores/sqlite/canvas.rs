//! Canvas calendar feed links in the SQLite file, sealed at rest.

use async_trait::async_trait;
use chrono::Utc;
use secrecy::SecretString;
use sqlx::{Row, SqlitePool};

use super::{Seal, database, stamp};
use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::core::types::store::StoreError;

/// Feed links held in the SQLite file, one per student.
pub struct SqliteCanvasFeeds {
    pool: SqlitePool,
    seal: Seal,
}

impl SqliteCanvasFeeds {
    /// Wraps a pool and the seal the links are kept under.
    pub fn new(pool: SqlitePool, seal: Seal) -> Self {
        Self { pool, seal }
    }
}

#[async_trait]
impl CanvasFeeds for SqliteCanvasFeeds {
    async fn save(&self, user: &str, link: &str) -> Result<(), StoreError> {
        let Some((nonce, sealed)) = self.seal.seal(link) else {
            return Err(StoreError::Database(
                "the feed link could not be sealed".into(),
            ));
        };
        sqlx::query(
            "INSERT INTO canvas_feeds (user_id, nonce, sealed, created_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT (user_id) DO UPDATE SET nonce = excluded.nonce, \
             sealed = excluded.sealed, created_at = excluded.created_at",
        )
        .bind(user)
        .bind(nonce)
        .bind(sealed)
        .bind(stamp(Utc::now()))
        .execute(&self.pool)
        .await
        .map_err(database)?;
        Ok(())
    }

    async fn load(&self, user: &str) -> Result<Option<SecretString>, StoreError> {
        let row = sqlx::query("SELECT nonce, sealed FROM canvas_feeds WHERE user_id = ?")
            .bind(user)
            .fetch_optional(&self.pool)
            .await
            .map_err(database)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let nonce: Vec<u8> = row.try_get("nonce").map_err(database)?;
        let sealed: Vec<u8> = row.try_get("sealed").map_err(database)?;
        let Some(link) = self.seal.open(&nonce, &sealed) else {
            tracing::warn!("a sealed Canvas feed link does not open; treated as not linked");
            return Ok(None);
        };
        Ok(Some(SecretString::from(link)))
    }

    async fn delete(&self, user: &str) -> Result<bool, StoreError> {
        let done = sqlx::query("DELETE FROM canvas_feeds WHERE user_id = ?")
            .bind(user)
            .execute(&self.pool)
            .await
            .map_err(database)?;
        Ok(done.rows_affected() > 0)
    }
}
