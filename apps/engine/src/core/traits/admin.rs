//! AdminStore trait.

use async_trait::async_trait;

use crate::core::types::admin::{AdminChange, HeldAction, NewChange};
use crate::core::types::store::StoreError;

/// Where admin changes are recorded and held actions are read.
#[async_trait]
pub trait AdminStore: Send + Sync {
    /// Records a change and returns it with its id and time.
    async fn record(&self, change: NewChange) -> Result<AdminChange, StoreError>;

    /// The newest change per kind and key in tenant: the values in force.
    async fn current(&self, tenant: &str) -> Result<Vec<AdminChange>, StoreError>;

    /// Changes in tenant, newest first, at most limit.
    async fn history(&self, tenant: &str, limit: usize) -> Result<Vec<AdminChange>, StoreError>;

    /// One change in tenant by id.
    async fn change(&self, tenant: &str, id: i64) -> Result<Option<AdminChange>, StoreError>;

    /// Actions in tenant still waiting on approval, oldest first.
    async fn held(&self, tenant: &str) -> Result<Vec<HeldAction>, StoreError>;

    /// Denies a held action; false when it is not waiting.
    async fn deny(&self, tenant: &str, token: &str) -> Result<bool, StoreError>;
}
