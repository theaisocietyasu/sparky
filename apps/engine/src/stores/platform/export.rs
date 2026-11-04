//! Pages of the platform agents export, read once to import into the SQLite file.

use reqwest::Method;

use super::client::{Call, PlatformClient, PlatformError};
use crate::core::types::export::{ConversationPage, MemoryPage};

/// Reads the platform agents export of the token organization.
pub struct PlatformExport {
    client: PlatformClient,
}

impl PlatformExport {
    /// Wraps a client.
    pub fn new(client: PlatformClient) -> Self {
        Self { client }
    }

    /// The query of one page: the cursor when there is one, and the page size.
    fn page_query(after: Option<&str>, limit: usize) -> Vec<(&'static str, String)> {
        let mut query = vec![("limit", limit.max(1).to_string())];
        if let Some(after) = after {
            query.push(("after", after.to_owned()));
        }
        query
    }

    /// One page of conversations with their messages, after the cursor.
    pub async fn conversations(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ConversationPage, PlatformError> {
        let query = Self::page_query(after, limit);
        let call = Call::new(Method::GET, "/api/agents/export/conversations").query(&query);
        let value = self.client.call(&call).await?;
        serde_json::from_value(value).map_err(|e| PlatformError::Body(e.to_string()))
    }

    /// One page of memories in clear, after the cursor.
    pub async fn memories(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<MemoryPage, PlatformError> {
        let query = Self::page_query(after, limit);
        let call = Call::new(Method::GET, "/api/agents/export/memories").query(&query);
        let value = self.client.call(&call).await?;
        serde_json::from_value(value).map_err(|e| PlatformError::Body(e.to_string()))
    }
}
