//! Memories on the platform agents routes, kept for the store.backend = "platform" rollback.

use async_trait::async_trait;
use reqwest::Method;
use serde_json::Value;
use uuid::Uuid;

use super::client::{Call, PlatformClient, PlatformError, field, optional_timestamp, timestamp};
use crate::core::traits::memory::MemoryStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::{Memory, MemoryKind, NewMemory};
use crate::core::types::store::StoreError;

/// Most memories the platform returns in one recall.
const RECALL_LIMIT: usize = 500;

/// Maps a platform error to a StoreError.
#[allow(clippy::needless_pass_by_value)]
fn store(e: PlatformError) -> StoreError {
    StoreError::Database(e.to_string())
}

/// The error of a write the platform backend does not take.
fn read_only() -> StoreError {
    StoreError::Database("memory writes need store.backend = \"sqlite\"".into())
}

/// Memories held by the platform agents module. Read only.
pub struct PlatformMemory {
    client: PlatformClient,
}

impl PlatformMemory {
    /// Wraps a client.
    pub fn new(client: PlatformClient) -> Self {
        Self { client }
    }
}

/// One memory from its platform row.
fn memory(row: &Value) -> Result<Memory, PlatformError> {
    let kind: String = field(row, "kind")?;
    let kind = MemoryKind::from_platform(&kind)
        .ok_or_else(|| PlatformError::Body(format!("memory kind {kind} is unknown")))?;
    let sensitivity: String = field(row, "sensitivity").unwrap_or_default();
    Ok(Memory {
        id: field(row, "id")?,
        kind,
        content: field(row, "content")?,
        sensitive: sensitivity == "sensitive",
        created_at: timestamp(row, "created_at")?,
        expires_at: optional_timestamp(row, "expires_at")?,
    })
}

#[async_trait]
impl MemoryStore for PlatformMemory {
    async fn recall(&self, ctx: &RequestContext) -> Result<Vec<Memory>, StoreError> {
        let query = vec![("limit", RECALL_LIMIT.to_string())];
        let path = PlatformClient::member(&ctx.user_id, "/memories");
        let value = self
            .client
            .call(&Call::new(Method::GET, &path).query(&query))
            .await
            .map_err(store)?;
        let rows: Vec<Value> = field(&value, "memories").map_err(store)?;
        rows.iter().map(|r| memory(r).map_err(store)).collect()
    }

    async fn remember(
        &self,
        _ctx: &RequestContext,
        _memory: &NewMemory,
    ) -> Result<Memory, StoreError> {
        Err(read_only())
    }

    async fn forget(&self, _ctx: &RequestContext, _id: Uuid) -> Result<bool, StoreError> {
        Err(read_only())
    }

    async fn forget_all(&self, _ctx: &RequestContext) -> Result<u64, StoreError> {
        Err(read_only())
    }
}
