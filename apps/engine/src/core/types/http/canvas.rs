//! Wire shapes of the Canvas feed link routes.

use serde::{Deserialize, Serialize};

/// A caller and the Canvas calendar feed link they shared, sent by the bot.
#[derive(Deserialize)]
pub struct SaveFeedRequest {
    /// The caller the link belongs to. A web session uses its own user instead.
    #[serde(default)]
    pub user: String,
    /// The Calendar Feed link from Canvas.
    pub link: String,
}

/// The caller whose link to remove, sent by the bot.
#[derive(Deserialize)]
pub struct RemoveFeedRequest {
    /// The caller whose link goes. A web session uses its own user instead.
    #[serde(default)]
    pub user: String,
}

/// Whether the caller has a Canvas link, for the web Connections page.
#[derive(Serialize)]
pub struct FeedStatus {
    /// True when a link is kept.
    pub linked: bool,
}

/// The answer to a save that kept the link.
#[derive(Serialize)]
pub struct SaveFeedResponse {
    /// Always true; a refused link answers with an error status.
    pub saved: bool,
}

/// Whether a remove found a link.
#[derive(Serialize)]
pub struct RemoveFeedResponse {
    /// True when a link was there and is now gone.
    pub removed: bool,
}
