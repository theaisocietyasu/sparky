//! Bodies of the per-user OAuth routes and the Canvas feed link routes.

use serde::{Deserialize, Serialize};

/// Body of POST /oauth/{provider}/authorize and /oauth/{provider}/logout.
#[derive(Debug, Serialize)]
pub struct AuthorizeRequest {
    /// Discord user id.
    pub user: String,
}

/// Reply to POST /oauth/{provider}/authorize.
#[derive(Debug, Deserialize)]
pub struct AuthorizeResponse {
    /// The consent URL the user opens to connect the provider.
    pub url: String,
}

/// Body of POST /canvas/feed.
#[derive(Serialize)]
pub struct SaveFeedRequest {
    /// Discord user id.
    pub user: String,
    /// The Calendar Feed link the user pasted. Never logged.
    pub link: String,
}

/// Reply to POST /canvas/feed.
#[derive(Debug, Deserialize)]
pub struct SaveFeedResponse {
    /// Whether the link was kept.
    pub saved: bool,
}

/// Reply to POST /oauth/{provider}/logout.
#[derive(Debug, Deserialize)]
pub struct DisconnectResponse {
    /// Whether a connection was removed.
    pub removed: bool,
}
