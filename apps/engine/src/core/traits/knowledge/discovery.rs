//! Discovery trait: hands the platform a public page the agent read, for it to save and refresh.

use async_trait::async_trait;

use crate::core::types::knowledge::retrieval::RetrievalError;

/// Sends the URL of a page the agent read; the platform decides whether to save it.
#[async_trait]
pub trait Discovery: Send + Sync {
    /// Suggests url to the platform. Sends the URL only.
    async fn suggest(&self, url: &str) -> Result<(), RetrievalError>;
}
