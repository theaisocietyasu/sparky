//! Readiness of the SQLite file.

use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::core::traits::health::Probe;

/// Answers readiness with SELECT 1.
pub struct SqliteProbe {
    pool: SqlitePool,
}

impl SqliteProbe {
    /// Wraps a pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Probe for SqliteProbe {
    fn name(&self) -> &'static str {
        "store"
    }

    async fn ready(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
}
