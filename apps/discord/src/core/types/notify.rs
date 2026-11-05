//! Notifications the engine holds for the bot to send as direct messages.

use serde::{Deserialize, Serialize};

/// Reply to GET /notifications/outgoing.
#[derive(Debug, Default, Deserialize)]
pub struct OutgoingList {
    /// Oldest first.
    #[serde(default)]
    pub notifications: Vec<Outgoing>,
}

/// One notification to send.
#[derive(Debug, Clone, Deserialize)]
pub struct Outgoing {
    /// Sent back once delivered.
    pub id: String,
    /// The Discord user to message.
    pub user_id: String,
    /// What it says.
    pub text: String,
}

/// Body of POST /notifications/sent.
#[derive(Debug, Serialize)]
pub struct SentRequest {
    /// Ids that were delivered or can never be.
    pub ids: Vec<String>,
}
