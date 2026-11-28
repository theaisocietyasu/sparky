//! Bodies of the Canvas feed link routes.

use serde::{Deserialize, Serialize};

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

/// Body of POST /canvas/feed/remove.
#[derive(Debug, Serialize)]
pub struct RemoveFeedRequest {
    /// Discord user id.
    pub user: String,
}

/// Reply to POST /canvas/feed/remove.
#[derive(Debug, Deserialize)]
pub struct RemoveFeedResponse {
    /// Whether a link was removed.
    pub removed: bool,
}
