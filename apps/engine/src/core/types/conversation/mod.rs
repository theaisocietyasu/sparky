//! Visibility, Stored, ResetRequest, ResetResponse: who reads history, what it holds, how it ends.

pub mod file;
pub mod image;
pub mod message;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::types::conversation::message::{Message, Notice};

/// Channel id every web turn of a student shares.
pub const WEB_CHANNEL: &str = "web";

/// Namespace of home conversation ids.
const HOME: Uuid = Uuid::from_u128(0x5a1c_e7d0_4f1b_4e2a_9c3d_8b7e_6f50_a1b2);

/// The id of the one home conversation of a student. Sparky's own messages land there.
pub fn home_id(tenant: &str, user: &str) -> Uuid {
    Uuid::new_v5(&HOME, format!("{tenant}\n{user}").as_bytes())
}

/// The message a thread starts from: a conversation and a position in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parent {
    /// Conversation the message is in.
    pub conversation_id: Uuid,
    /// Position of the message in it.
    pub position: i64,
}

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
    /// The message it is a thread of, when it is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Parent>,
}

/// One past message that matched a chat search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatHit {
    /// Conversation it is in.
    pub conversation_id: String,
    /// Channel of that conversation; web for the web chat.
    pub channel_id: String,
    /// user or assistant.
    pub role: String,
    /// The part of the message around the match.
    pub snippet: String,
    /// When it was said, RFC 3339 UTC.
    pub at: String,
}

/// One question or answer of a conversation as the web shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Turn {
    /// Position in the conversation, the one a thread names.
    pub position: i64,
    /// user or assistant.
    pub role: String,
    /// The text.
    pub text: String,
    /// The notification a message Sparky sent on its own stands for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<Notice>,
}

/// GET /web/home: the id of the home conversation of the student.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Home {
    /// Conversation id.
    pub id: Uuid,
}
