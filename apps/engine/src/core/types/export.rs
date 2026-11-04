//! ExportConversation, ExportMessage, ExportMemory: the platform agents export, one page at a time.

use serde::Deserialize;
use serde_json::Value;

/// One message of an exported conversation.
#[derive(Debug, Clone, Deserialize)]
pub struct ExportMessage {
    /// Platform write order.
    pub seq: i64,
    /// system, user, assistant, tool or summary.
    pub role: String,
    /// The whole message as the engine stored it.
    pub content: Value,
    /// For a summary, the platform seq it stands in for.
    #[serde(default)]
    pub covers_seq: Option<i64>,
    /// When it was written.
    pub created_at: String,
}

/// One exported conversation with its messages in seq order.
#[derive(Debug, Clone, Deserialize)]
pub struct ExportConversation {
    /// Conversation id, kept on import.
    pub id: String,
    /// The member, which becomes user_id.
    pub discord_id: String,
    /// Channel the conversation is in.
    pub channel_id: String,
    /// public or private.
    pub visibility: String,
    /// When it started.
    pub created_at: String,
    /// When it last changed.
    pub updated_at: String,
    /// When it ended, if it did.
    #[serde(default)]
    pub ended_at: Option<String>,
    /// Its messages, oldest first.
    #[serde(default)]
    pub messages: Vec<ExportMessage>,
}

/// One exported memory, opened by the platform.
#[derive(Debug, Clone, Deserialize)]
pub struct ExportMemory {
    /// Memory id, kept on import.
    pub id: String,
    /// The member, which becomes user_id.
    pub discord_id: String,
    /// episodic, semantic, profile or task.
    pub kind: String,
    /// The remembered text in clear.
    pub content: String,
    /// normal or sensitive.
    pub sensitivity: String,
    /// Confidence, 0 to 1.
    pub confidence: f64,
    /// When it was written.
    pub created_at: String,
    /// When it stops being recalled.
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// GET /api/agents/export/conversations: one page and the cursor of the next.
#[derive(Debug, Clone, Deserialize)]
pub struct ConversationPage {
    /// Conversations in id order.
    pub conversations: Vec<ExportConversation>,
    /// Pass as after for the next page; None on the last page.
    #[serde(default)]
    pub next: Option<String>,
}

/// GET /api/agents/export/memories: one page and the cursor of the next.
#[derive(Debug, Clone, Deserialize)]
pub struct MemoryPage {
    /// Memories in id order.
    pub memories: Vec<ExportMemory>,
    /// Pass as after for the next page; None on the last page.
    #[serde(default)]
    pub next: Option<String>,
}
