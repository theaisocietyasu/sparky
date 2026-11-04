//! MemoryStore trait. Every call is scoped to the tenant and user of the request.

use async_trait::async_trait;
use uuid::Uuid;

use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::{Memory, NewMemory};
use crate::core::types::store::StoreError;

/// Cross-conversation memory for one user in one tenant.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Unexpired memories: tasks due soon first, soonest first, then the rest newest first.
    async fn recall(&self, ctx: &RequestContext) -> Result<Vec<Memory>, StoreError>;
    /// Writes one memory, dropping the oldest when the caller is at the cap.
    async fn remember(
        &self,
        ctx: &RequestContext,
        memory: &NewMemory,
    ) -> Result<Memory, StoreError>;
    /// Removes one memory of the caller; false if it was not there.
    async fn forget(&self, ctx: &RequestContext, id: Uuid) -> Result<bool, StoreError>;
    /// Removes every memory of the caller; returns the count.
    async fn forget_all(&self, ctx: &RequestContext) -> Result<u64, StoreError>;
}
