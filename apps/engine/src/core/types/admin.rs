//! Admin surface data: recorded changes, held actions, and the wire shapes of the admin routes.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What an admin change sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// A tool switched on or off; the value is a boolean.
    Tool,
    /// The system instructions; the value is a string, or null for the configured default.
    Instructions,
}

impl ChangeKind {
    /// The stored name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Instructions => "instructions",
        }
    }

    /// The kind a stored name names.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "tool" => Some(Self::Tool),
            "instructions" => Some(Self::Instructions),
            _ => None,
        }
    }
}

/// The key the instructions are recorded under.
pub const INSTRUCTIONS_KEY: &str = "system";

/// A change before it is recorded.
#[derive(Debug, Clone, PartialEq)]
pub struct NewChange {
    /// Scope of the change.
    pub tenant_id: String,
    /// What it sets.
    pub kind: ChangeKind,
    /// The tool name, or INSTRUCTIONS_KEY.
    pub key: String,
    /// The value set.
    pub value: Value,
    /// Who made it.
    pub actor: String,
}

/// A recorded change.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AdminChange {
    /// Row id; a rollback names it.
    pub id: i64,
    /// What it set.
    pub kind: ChangeKind,
    /// The tool name, or INSTRUCTIONS_KEY.
    pub key: String,
    /// The value set.
    pub value: Value,
    /// Who made it.
    pub actor: String,
    /// When, RFC 3339 UTC.
    pub created_at: String,
}

/// An action waiting on a student's approval, without its arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeldAction {
    /// The confirmation token.
    pub token: String,
    /// Who was asked.
    pub user_id: String,
    /// The tool that would run.
    pub tool: String,
    /// When it was held, RFC 3339 UTC.
    pub created_at: String,
    /// When it lapses, RFC 3339 UTC.
    pub expires_at: String,
}

/// One tool as GET /admin/tools lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolState {
    /// Tool name.
    pub name: String,
    /// What the model reads about it.
    pub description: String,
    /// Offered to the model.
    pub on: bool,
    /// Offered only in private turns.
    pub private: bool,
}

/// POST /admin/tools/{name} body.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ToolSwitch {
    /// Offer the tool, or stop offering it.
    pub on: bool,
}

/// GET /admin/instructions answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstructionsState {
    /// The instructions in force.
    pub text: String,
    /// The configured instructions, used when no change is in force.
    pub default_text: String,
    /// Whether an admin change is in force.
    pub changed: bool,
    /// The longest text PUT accepts.
    pub max_chars: usize,
}

/// PUT /admin/instructions body.
#[derive(Debug, Clone, Deserialize)]
pub struct InstructionsUpdate {
    /// New instructions, or null to return to the configured default.
    pub text: Option<String>,
}

/// GET /admin/limits answer: the settings that bound load, read only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminLimits {
    /// Turns that run at once.
    pub max_turns: usize,
    /// Model requests that run at once; 0 is unlimited.
    pub model_slots: usize,
    /// Requests per user per minute; 0 is unlimited.
    pub rate_limit_per_min: u32,
    /// Memories kept per user.
    pub memory_max_per_user: usize,
    /// Days an idle conversation is kept; 0 keeps it.
    pub conversation_idle_days: u32,
}

/// One request as GET /admin/traces lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceSummary {
    /// Request id; GET /admin/traces/{id} reads the whole trace.
    pub request_id: String,
    /// Conversation it belongs to.
    pub conversation_id: String,
    /// When it started, RFC 3339 UTC.
    pub at: String,
    /// Tenant scope.
    pub tenant_id: String,
    /// Caller.
    pub user_id: String,
    /// The start of the user input.
    pub input: String,
    /// How it ended; None while running or when the trace was cut short.
    pub status: Option<String>,
    /// Model steps.
    pub steps: u32,
    /// Total tokens.
    pub tokens: u32,
    /// Wall time.
    pub duration_ms: u64,
}
