//! Pages the agent read, sent to the platform knowledge module to save and refresh.

use async_trait::async_trait;
use reqwest::Method;
use serde_json::json;

use super::client::{Call, PlatformClient};
use crate::core::traits::knowledge::discovery::Discovery;
use crate::core::types::knowledge::query::QueryError;

/// Suggests pages over POST /api/knowledge/discovered.
pub struct PlatformDiscovery {
    client: PlatformClient,
}

impl PlatformDiscovery {
    /// Wraps a platform client whose token holds knowledge:discover.
    pub fn new(client: PlatformClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Discovery for PlatformDiscovery {
    async fn suggest(&self, url: &str) -> Result<(), QueryError> {
        let body = json!({ "url": url });
        self.client
            .call(&Call::new(Method::POST, "/api/knowledge/discovered").body(&body))
            .await
            .map(|_| ())
            .map_err(|e| QueryError::Store(e.to_string()))
    }
}
