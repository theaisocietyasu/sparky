//! Platform search: the live sources it publishes, and the search tool run over /api/tools.

use async_trait::async_trait;
use reqwest::Method;
use serde_json::{Map, Value};

use super::client::{Call, PlatformClient, PlatformError, field};
use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::query::{QueryError, QuerySourceInfo, SearchAnswer, SearchSpec};

/// Name of the search tool on Platform.
const SEARCH_TOOL: &str = "search";

/// Search run by the platform.
pub struct PlatformQueries {
    client: PlatformClient,
}

impl PlatformQueries {
    /// Wraps a client.
    pub fn new(client: PlatformClient) -> Self {
        Self { client }
    }
}

/// Maps a platform error: a refusal of the arguments goes back to the model, the rest are failures.
fn query_error(e: PlatformError) -> QueryError {
    match e {
        PlatformError::Timeout(budget) => QueryError::Timeout(budget),
        PlatformError::Status {
            status: 400 | 404 | 422,
            message,
        } => QueryError::Rejected(message),
        PlatformError::Status {
            status: 502..=504,
            message,
        } => QueryError::Unavailable(message),
        PlatformError::Transport(_) => {
            QueryError::Unavailable("the connection to Platform broke".into())
        }
        other => QueryError::Store(other.to_string()),
    }
}

/// Time a request must still have left for a failed search to be sent again.
const RETRY_ROOM: std::time::Duration = std::time::Duration::from_secs(5);

/// Whether a failed call may pass when sent again: the connection broke, or a live source was down.
fn transient(error: &PlatformError) -> bool {
    matches!(error, PlatformError::Transport(_)) || matches!(error.status(), Some(502..=504))
}

/// The tool named name in a GET /api/tools answer.
fn spec_in(listing: &Value, name: &str) -> Result<SearchSpec, QueryError> {
    let tools: Vec<Value> =
        field(listing, "tools").map_err(|e| QueryError::Store(e.to_string()))?;
    let tool = tools
        .into_iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| {
            QueryError::Store(format!(
                "Platform offers no {name} tool to this token; it needs knowledge:read and the knowledge module on"
            ))
        })?;
    serde_json::from_value(tool).map_err(|e| QueryError::Store(format!("{name} tool: {e}")))
}

#[async_trait]
impl SourceQueries for PlatformQueries {
    async fn sources(&self) -> Result<Vec<QuerySourceInfo>, QueryError> {
        let value = self
            .client
            .call(&Call::new(Method::GET, "/api/asu/queries"))
            .await
            .map_err(query_error)?;
        field(&value, "queries").map_err(|e| QueryError::Store(e.to_string()))
    }

    async fn spec(&self) -> Result<SearchSpec, QueryError> {
        let listing = self
            .client
            .call(&Call::new(Method::GET, "/api/tools"))
            .await
            .map_err(query_error)?;
        spec_in(&listing, SEARCH_TOOL)
    }

    async fn search(
        &self,
        ctx: &RequestContext,
        args: &Map<String, Value>,
    ) -> Result<SearchAnswer, QueryError> {
        let remaining = ctx.remaining();
        if remaining.is_zero() {
            return Err(QueryError::Timeout(remaining));
        }
        let body = Value::Object(args.clone());
        let path = format!("/api/tools/{SEARCH_TOOL}");
        let mut retried = false;
        let value = loop {
            let remaining = ctx.remaining();
            let call = Call::new(Method::POST, &path)
                .body(&body)
                .timeout(remaining);
            let answer = tokio::select! {
                () = ctx.cancel.cancelled() => return Err(QueryError::Cancelled),
                answer = self.client.call(&call) => answer,
            };
            match answer {
                Ok(value) => break value,
                Err(error) if !retried && transient(&error) && ctx.remaining() > RETRY_ROOM => {
                    tracing::warn!(%error, "search failed for a moment; trying once more");
                    retried = true;
                }
                Err(error) => return Err(query_error(error)),
            }
        };
        field(&value, "result").map_err(|e| QueryError::Store(format!("search result: {e}")))
    }
}
