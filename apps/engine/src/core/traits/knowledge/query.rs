//! SourceQueries trait: the sources Platform publishes and its search tool.

use async_trait::async_trait;
use serde_json::{Map, Value};

use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::query::{QueryError, QuerySourceInfo, SearchAnswer, SearchSpec};

/// Platform search: the live sources it can name, its tool description, and one search.
#[async_trait]
pub trait SourceQueries: Send + Sync {
    /// Live sources Platform publishes, for the admin pages.
    async fn sources(&self) -> Result<Vec<QuerySourceInfo>, QueryError>;

    /// The search tool as Platform describes it. Read once at boot to build the tool.
    async fn spec(&self) -> Result<SearchSpec, QueryError>;

    /// Runs one search with the arguments the model sent, inside the request budget.
    async fn search(
        &self,
        ctx: &RequestContext,
        args: &Map<String, Value>,
    ) -> Result<SearchAnswer, QueryError>;
}
