//! AgentConfig, Answer, AgentError.

pub mod assemble;
pub mod context;
pub mod thinking;

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::types::agent::assemble::Budget;
use crate::core::types::agent::thinking::ThinkingRules;
use crate::core::types::knowledge::evidence::Citation;
use crate::core::types::model::{ModelError, Usage};
use crate::core::types::safety::policy::ConfirmationRequest;
use crate::core::types::tools::ToolRun;
use crate::core::types::trace::RunStatus;

/// Knobs for the loop. Default is implemented in core::config.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// gen_ai.provider.name on model spans.
    pub provider_name: std::sync::Arc<str>,
    /// Model name as requested, for gen_ai.request.model.
    pub model_name: std::sync::Arc<str>,
    /// Maximum model calls per request.
    pub max_steps: u32,
    /// Retries on a retryable model error, per step.
    pub max_model_retries: u32,
    /// Per-tool-call timeout.
    pub tool_timeout: Duration,
    /// How long a held action waits for caller approval.
    pub confirmation_ttl: Duration,
    /// Completion budget of a model call that thinks.
    pub max_tokens: u32,
    /// Completion budget of a model call that does not think.
    pub max_tokens_without_thinking: u32,
    /// Sampling temperature.
    pub temperature: f32,
    /// Characters of an attached file's text put in the prompt.
    pub upload_preview_chars: usize,
    /// Characters of passages matching the question put in the prompt.
    pub upload_match_chars: usize,
    /// Pages of a scanned PDF read by OCR.
    pub upload_ocr_pages: u32,
    /// Prior turns to load.
    pub history_turns: usize,
    /// Tokens of recent turns a compaction keeps whole.
    pub history_keep: usize,
    /// Memories recalled per request.
    pub memory_recall_limit: usize,
    /// First retry wait, doubled per attempt.
    pub retry_base_ms: u64,
    /// Longest retry wait.
    pub retry_cap_ms: u64,
    /// Longest value recorded on a span; the JSONL trace keeps the rest.
    pub max_span_value_chars: usize,
    /// Tool result length past which the result is written to the sandbox workspace. 0 keeps all.
    pub tool_result_to_file_chars: usize,
    /// USD per million prompt tokens, for cost tracking. Zero for local models.
    pub usd_per_m_prompt: f64,
    /// USD per million completion tokens.
    pub usd_per_m_completion: f64,
    /// Prompt budgets.
    pub budget: Budget,
    /// When a model call thinks.
    pub thinking: ThinkingRules,
    /// Stream model calls to whoever is watching.
    pub stream: bool,
    /// Longest run of answer text held back while waiting for the end of a sentence or line.
    pub stream_block_chars: usize,
}

/// How a run ended and what it produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    /// Final text. Empty when awaiting confirmation.
    pub text: String,
    /// Pages the tools read while answering, in the order they read them.
    #[serde(default)]
    pub sources: Vec<Citation>,
    /// Set when the loop stopped to ask the user.
    pub confirmation: Option<ConfirmationRequest>,
    /// How it ended.
    pub status: RunStatus,
    /// Model calls made.
    pub steps: u32,
    /// Tools that ran, in order.
    pub tool_runs: Vec<ToolRun>,
    /// Content of each memory the prompt carried.
    #[serde(default)]
    pub memories: Vec<String>,
    /// Tokens across every call.
    pub usage: Usage,
    /// Estimated cost in USD.
    pub cost_usd: f64,
}

impl Answer {
    /// One citation per page the tools read while answering, each page once.
    pub fn citations(&self) -> Vec<Citation> {
        let mut out: Vec<Citation> = Vec::new();
        for source in &self.sources {
            if !out.iter().any(|c| c.url.is_some() && c.url == source.url) {
                out.push(source.clone());
            }
        }
        out
    }
}

/// Loop failures. Everything recoverable has already been fed back to the model.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The model failed after retries.
    #[error(transparent)]
    Model(#[from] ModelError),
    /// A store the request needs was unavailable.
    #[error("store: {0}")]
    Store(String),
}
