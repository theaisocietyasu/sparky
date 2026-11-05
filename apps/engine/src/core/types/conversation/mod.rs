//! Visibility, Stored, ResetRequest, ResetResponse: who reads history, what it holds, how it ends.

pub mod file;
pub mod image;
pub mod message;

use serde::{Deserialize, Serialize};

use crate::core::types::conversation::message::Message;

/// A message loaded from history and its position. A summary sits at the last message it replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// Position in the conversation, increasing in the order messages were written.
    pub position: i64,
    /// The message.
    pub message: Message,
}

/// Who can read the exchange a request belongs to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Others can read it, as in a guild thread.
    #[default]
    Public,
    /// Only the caller reads it, as in an ephemeral exchange or a one-to-one client.
    Private,
}

impl Visibility {
    /// Database column value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

/// POST /conversation/reset: ends every open conversation of the caller in one channel.
#[derive(Debug, Deserialize)]
pub struct ResetRequest {
    /// Caller id as the edge knows it.
    #[serde(rename = "user_id")]
    pub user: String,
    /// Tenant scope; defaults to the configured guild.
    #[serde(default, rename = "tenant_id")]
    pub tenant: Option<String>,
    /// Channel whose conversations end.
    #[serde(rename = "channel_id")]
    pub channel: String,
}

/// How many conversations ended.
#[derive(Debug, Serialize, Deserialize)]
pub struct ResetResponse {
    /// Conversations ended.
    pub ended: u64,
}

/// One open conversation as the web sidebar lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConversationSummary {
    /// Conversation id.
    pub id: String,
    /// The start of its first question.
    pub title: String,
    /// When it last changed, RFC 3339 UTC.
    pub updated_at: String,
}

/// One question or answer of a conversation as the web shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Turn {
    /// user or assistant.
    pub role: String,
    /// The text.
    pub text: String,
}
