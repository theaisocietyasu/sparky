//! ToolDefinition, RiskClass, ToolOutput, ToolError.

pub mod canvas;
pub mod gcal;
pub mod oauth;
pub mod outlook;
pub mod sandbox;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::types::knowledge::evidence::Citation;

/// Name of the tool that searches the stored index.
pub const SEARCH_KNOWLEDGE: &str = "search_knowledge";

/// Name of the tool that fetches a source now.
pub const SEARCH_LIVE: &str = "search_live";

/// Name of the tool that writes a memory.
pub const REMEMBER: &str = "remember";

/// Name of the tool that removes a memory.
pub const FORGET_MEMORY: &str = "forget_memory";

/// Name of the tool that runs a command in the isolated environment.
pub const SANDBOX: &str = "run_sandbox";

/// What a tool can do to the world. Drives Policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    /// Reads public data. Runs freely.
    ReadPublic,
    /// Reads inside the authenticated session of the user.
    ReadAuthenticated,
    /// Drafts or fills without submitting.
    PrepareWrite,
    /// Posts, creates, books, submits. Confirmed immediately before.
    ExternalWrite,
    /// Deletes or cancels. Confirmed immediately before.
    Destructive,
    /// Never allowed.
    Forbidden,
}

/// What the model sees, and what Policy classifies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique name the model calls.
    pub name: String,
    /// What it does, for the model.
    pub description: String,
    /// JSON Schema for arguments.
    pub parameters: Value,
    /// Risk classification.
    pub risk: RiskClass,
    /// Reads or writes what is personal to the user; offered and allowed only in a private turn.
    #[serde(default)]
    pub private: bool,
    /// Holds state between calls; its calls never run in parallel with other calls in the step.
    #[serde(default)]
    pub sequential: bool,
    /// Overrides agent.tool_timeout_secs for this tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// What a tool returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Text handed back to the model.
    pub content: String,
    /// Structured payload for the trace and the client, when the tool has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// Pages this call read. Cited alongside the evidence retrieval supplied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Citation>,
}

/// Tool failures. Reported to the model as text.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// Arguments did not match the schema.
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    /// The failure of the tool itself.
    #[error("{0}")]
    Failed(String),
    /// Ran past its timeout.
    #[error("tool timed out")]
    Timeout,
    /// The request was cancelled.
    #[error("tool cancelled")]
    Cancelled,
}

/// One tool the agent actually ran, in call order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRun {
    /// Tool name as the model called it.
    pub tool: String,
    /// Whether the call returned a result.
    pub ok: bool,
}
