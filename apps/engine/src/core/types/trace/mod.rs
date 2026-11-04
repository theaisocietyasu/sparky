//! TraceEvent, RunStatus, TraceRecord.

pub mod progress;
pub mod status;

pub use self::status::RunStatus;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::core::types::memory::MemoryKind;
use crate::core::types::model::{FinishReason, Usage};
use crate::core::types::safety::guardrail::Stage;
use crate::core::types::safety::policy::Decision;

/// One thing that happened during a request. Never carries secrets or raw credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TraceEvent {
    /// The loop began.
    RequestStarted {
        /// User input, as received.
        input: String,
        /// Tenant scope.
        tenant_id: String,
        /// Caller.
        user_id: String,
    },
    /// Context assembled for one step.
    ContextAssembled {
        /// Loop step, from 1.
        step: u32,
        /// Messages sent.
        message_count: usize,
        /// Rough prompt token estimate.
        estimated_tokens: usize,
    },
    /// A model call is about to start. Once per step, whatever the retries.
    ModelStarted {
        /// Loop step.
        step: u32,
    },
    /// One completion returned.
    ModelCall {
        /// Loop step.
        step: u32,
        /// Model name as reported.
        model: String,
        /// Why it stopped.
        finish_reason: FinishReason,
        /// Tokens.
        usage: Usage,
        /// Wall time.
        duration_ms: u64,
        /// Retry index, 0 for the first attempt.
        attempt: u32,
    },
    /// The model has reasoned this far on a step. Sent while it streams; never recorded.
    ModelReasoning {
        /// Loop step.
        step: u32,
        /// The reasoning so far.
        text: String,
    },
    /// What the model reasoned on a step, or wrote on its way to a tool call.
    ModelThought {
        /// Loop step.
        step: u32,
        /// What it wrote, truncated.
        text: String,
    },
    /// The answer as written so far, released a block at a time while it streams; never recorded.
    AnswerDraft {
        /// Loop step.
        step: u32,
        /// The answer so far.
        text: String,
    },
    /// The draft so far withdraws: tools were called, the call failed, or the guardrail refused it.
    AnswerDraftCleared {
        /// Loop step.
        step: u32,
    },
    /// The model wrote the answer and the loop is done.
    ModelAnswered {
        /// Loop step.
        step: u32,
    },
    /// A model call failed.
    ModelError {
        /// Loop step.
        step: u32,
        /// Attempt that failed.
        attempt: u32,
        /// Error text.
        error: String,
        /// Whether the loop retried.
        retried: bool,
    },
    /// Policy ruled on a proposed tool call.
    PolicyDecision {
        /// Loop step.
        step: u32,
        /// Tool name.
        tool: String,
        /// Verdict.
        decision: Decision,
    },
    /// The guardrail refused a response.
    GuardrailBlocked {
        /// Loop step.
        step: u32,
        /// Which branch the response was on.
        stage: Stage,
        /// Why it was refused.
        reason: String,
    },
    /// The guardrail removed protected terms from a response before showing it.
    GuardrailRedacted {
        /// Loop step.
        step: u32,
        /// Which branch the response was on.
        stage: Stage,
        /// What was removed, for the trace.
        reason: String,
    },
    /// History that no longer fits is being replaced by one turn.
    Compaction {
        /// Stored messages being replaced, a previous summary included.
        turns: usize,
    },
    /// A tool is about to run.
    ToolStarted {
        /// Loop step.
        step: u32,
        /// Provider call id.
        call_id: String,
        /// Tool name.
        tool: String,
        /// Validated arguments.
        arguments: Value,
    },
    /// A tool ran.
    ToolCall {
        /// Loop step.
        step: u32,
        /// Provider call id.
        call_id: String,
        /// Tool name.
        tool: String,
        /// Validated arguments.
        arguments: Value,
        /// Ok text or Err message, truncated.
        result: Result<String, String>,
        /// Wall time.
        duration_ms: u64,
    },
    /// A tool result too long to carry was written to the sandbox workspace.
    ToolResultStored {
        /// Loop step.
        step: u32,
        /// Tool whose result it is.
        tool: String,
        /// Where it landed inside the workspace.
        path: String,
        /// Size of what was written.
        bytes: usize,
    },
    /// A file the caller attached was copied into the sandbox workspace, or could not be.
    FileAttached {
        /// File name as uploaded.
        name: String,
        /// Size written. Zero when it failed.
        bytes: usize,
        /// Where it landed inside the workspace.
        path: Option<String>,
        /// Why it could not be opened.
        error: Option<String>,
    },
    /// Memory and the profile graph were read for the prompt.
    MemoryRecalled {
        /// Memories recalled.
        count: usize,
    },
    /// A memory was written for the caller. The text stays in the redacted tool span.
    MemoryWritten {
        /// Kind of the memory.
        memory: MemoryKind,
    },
    /// A memory of the caller was removed.
    MemoryForgotten,
    /// The loop finished.
    Completed {
        /// How it ended.
        status: RunStatus,
        /// Total steps.
        steps: u32,
        /// Total tokens.
        usage: Usage,
        /// Estimated cost in USD.
        cost_usd: f64,
        /// Wall time.
        duration_ms: u64,
    },
}

impl TraceEvent {
    /// The snake case name this event serializes under.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::RequestStarted { .. } => "request_started",
            Self::ContextAssembled { .. } => "context_assembled",
            Self::ModelStarted { .. } => "model_started",
            Self::ModelCall { .. } => "model_call",
            Self::ModelReasoning { .. } => "model_reasoning",
            Self::ModelThought { .. } => "model_thought",
            Self::AnswerDraft { .. } => "answer_draft",
            Self::AnswerDraftCleared { .. } => "answer_draft_cleared",
            Self::ModelAnswered { .. } => "model_answered",
            Self::ModelError { .. } => "model_error",
            Self::PolicyDecision { .. } => "policy_decision",
            Self::GuardrailBlocked { .. } => "guardrail_blocked",
            Self::GuardrailRedacted { .. } => "guardrail_redacted",
            Self::Compaction { .. } => "compaction",
            Self::ToolStarted { .. } => "tool_started",
            Self::ToolCall { .. } => "tool_call",
            Self::ToolResultStored { .. } => "tool_result_stored",
            Self::FileAttached { .. } => "file_attached",
            Self::MemoryRecalled { .. } => "memory_recalled",
            Self::MemoryWritten { .. } => "memory_written",
            Self::MemoryForgotten => "memory_forgotten",
            Self::Completed { .. } => "completed",
        }
    }

    /// Whether this event removes the line of its slot instead of writing one.
    pub fn clears_slot(&self) -> bool {
        matches!(
            self,
            Self::ModelAnswered { .. } | Self::AnswerDraftCleared { .. }
        )
    }

    /// Whether this event carries the answer being written rather than a step.
    pub fn is_draft(&self) -> bool {
        matches!(
            self,
            Self::AnswerDraft { .. } | Self::AnswerDraftCleared { .. }
        )
    }

    /// Whether this event is sent only to watchers and left out of the recorded trace.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::ModelReasoning { .. }
                | Self::AnswerDraft { .. }
                | Self::AnswerDraftCleared { .. }
        )
    }

    /// Line an event overwrites, or None for a new one. Calls share start slot; events share step.
    pub fn slot(&self) -> Option<String> {
        match self {
            Self::ToolStarted { call_id, .. } | Self::ToolCall { call_id, .. } => {
                Some(format!("tool:{call_id}"))
            }
            Self::ModelStarted { step }
            | Self::ModelReasoning { step, .. }
            | Self::ModelThought { step, .. }
            | Self::ModelAnswered { step } => Some(format!("model:{step}")),
            Self::AnswerDraft { .. } | Self::AnswerDraftCleared { .. } => {
                Some(ANSWER_SLOT.to_owned())
            }
            _ => None,
        }
    }
}

/// The slot the answer draft is written to.
pub const ANSWER_SLOT: &str = "answer";

/// A trace event with its envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRecord {
    /// Request this belongs to.
    pub request_id: Uuid,
    /// Conversation this belongs to.
    pub conversation_id: Uuid,
    /// When it was emitted.
    pub at: DateTime<Utc>,
    /// The event.
    #[serde(flatten)]
    pub event: TraceEvent,
}
