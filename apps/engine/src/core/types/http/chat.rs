//! ChatRequest, ChatResponse, and ErrorBody are the /chat wire contract with clients.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::types::conversation::file::FileAttachment;
use crate::core::types::conversation::image::Attachment;
use crate::core::types::conversation::{Parent, Visibility};
use crate::core::types::knowledge::evidence::Citation;
use crate::core::types::safety::policy::ConfirmationRequest;
use crate::core::types::tools::ToolRun;
use crate::core::types::trace::RunStatus;

/// Request body.
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// Caller id as the edge knows it.
    pub user_id: String,
    /// Tenant scope; defaults to the configured guild.
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Channel the message came from.
    #[serde(default = "default_channel")]
    pub channel_id: String,
    /// Roles asserted by the edge.
    #[serde(default)]
    pub roles: Vec<String>,
    /// Continue this conversation; omit to start one.
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
    /// Who can read the exchange. Public withholds personal memory.
    #[serde(default)]
    pub visibility: Visibility,
    /// Without conversation_id, continues the caller's newest open conversation at this visibility.
    #[serde(default)]
    pub continue_channel: bool,
    /// The message.
    pub message: String,
    /// The message of ours this one replies to, when it replies to one.
    #[serde(default)]
    pub reply_to: Option<String>,
    /// The message a new conversation is a thread of.
    #[serde(default)]
    pub parent: Option<Parent>,
    /// Images attached to the message.
    #[serde(default)]
    pub images: Vec<Attachment>,
    /// Other files attached to the message, opened in the sandbox.
    #[serde(default)]
    pub files: Vec<FileAttachment>,
}

fn default_channel() -> String {
    "http".into()
}

/// Response body.
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    /// Trace id.
    pub request_id: Uuid,
    /// Conversation to continue with.
    pub conversation_id: Uuid,
    /// The answer.
    pub text: String,
    /// Sources the answer rests on, best first.
    pub citations: Vec<Citation>,
    /// Set when the agent stopped to ask.
    pub confirmation: Option<ConfirmationRequest>,
    /// How the run ended.
    pub status: RunStatus,
    /// Model calls made.
    pub steps: u32,
    /// Tools that ran, in order.
    pub tools: Vec<ToolRun>,
    /// Content of each memory the prompt carried. Empty for a public request.
    pub memories: Vec<String>,
    /// Total tokens.
    pub tokens: u32,
    /// Estimated cost in USD.
    pub cost_usd: f64,
}

/// Error body.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    /// Trace id.
    pub request_id: Uuid,
    /// What went wrong.
    pub error: String,
    /// HTTP status this would have carried. Present on the stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
}

/// POST /confirm: answer an action the engine is holding.
#[derive(Debug, Deserialize)]
pub struct ConfirmRequest {
    /// The token from the ConfirmationRequest.
    pub token: Uuid,
    /// Whether to run it.
    pub approve: bool,
    /// Who is answering. Only the caller who was asked may answer.
    pub user_id: String,
    /// Guild or other scope; the engine default when absent.
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// The conversation the held action belongs to.
    pub conversation_id: Uuid,
    /// Who can read the exchange the answer lands in.
    #[serde(default)]
    pub visibility: Visibility,
}
