//! Settings that shape the agent loop and the prompt it assembles.

use serde::Deserialize;

use crate::core::config::ConfigError;
use crate::core::types::agent::assemble::{self, Budget};
use crate::core::types::agent::thinking::{ThinkingMode, ThinkingRules, words};
use crate::core::types::trace::progress::{self, ProgressStyle};

/// Agent loop limits and prompt budgets.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Agent {
    /// Model calls per request.
    pub max_steps: u32,
    /// Retries per model call on transport or 5xx errors.
    pub max_model_retries: u32,
    /// Wall-clock budget per request.
    pub request_timeout_secs: u64,
    /// Budget per tool call, unless the tool declares its own.
    pub tool_timeout_secs: u64,
    /// Seconds of the request kept for the final answer. Gathering stops when less is left.
    pub answer_reserve_secs: u64,
    /// Tool calls per request before the next step must answer. 0 removes the limit.
    pub max_tool_calls: usize,
    /// How long a held action waits for caller approval.
    pub confirmation_ttl_secs: u64,
    /// Model calls in flight at once. Match llama-server --parallel. 0 removes the limit.
    pub model_slots: usize,
    /// How long a request waits for a free model slot before reporting the model busy.
    pub model_queue_wait_secs: u64,
    /// Turns the engine runs at once.
    pub max_turns: usize,
    /// Longest a turn waits for a free turn slot before it is refused as busy.
    pub turn_queue_wait_secs: u64,
    /// Sampling temperature.
    pub temperature: f32,
    /// Prior turns loaded into the prompt.
    pub history_turns: usize,
    /// Whole-prompt token budget.
    pub prompt_budget_tokens: usize,
    /// Cap on prior turns.
    pub history_budget_tokens: usize,
    /// Cap on the memory section.
    pub memory_budget_tokens: usize,
    /// Cap on the capabilities section.
    pub capabilities_budget_tokens: usize,
    /// Cap on the quoted message a reply answers.
    pub reply_budget_tokens: usize,
    /// Images of one message sent to the model. 0 sends none.
    pub max_images: usize,
    /// Other files of one message copied into the sandbox. 0 opens none.
    pub max_files: usize,
    /// Largest attached file copied into the sandbox, in bytes.
    pub max_file_bytes: u64,
    /// Hosts attached files are downloaded from. Any other link is refused.
    pub file_hosts: Vec<String>,
    /// Characters of an attached file's text put in the prompt beside the question.
    pub upload_preview_chars: usize,
    /// Characters of the passages of an attached file that match the question, put in the prompt.
    pub upload_match_chars: usize,
    /// Pages of a scanned PDF read by OCR when it holds no text layer.
    pub upload_ocr_pages: u32,
    /// Characters per token the budget estimator assumes.
    pub chars_per_token: usize,
    /// First retry wait, doubled per attempt.
    pub retry_base_ms: u64,
    /// Longest retry wait.
    pub retry_cap_ms: u64,
    /// Longest value recorded on a span; the JSONL trace keeps the rest.
    pub max_span_value_chars: usize,
    /// Tool result length past which the result goes to the sandbox workspace. 0 carries it whole.
    pub tool_result_to_file_chars: usize,
    /// Characters of a tool argument list or a tool result one live progress line carries.
    pub progress_detail_chars: usize,
    /// Characters of reasoning a thinking line carries.
    pub progress_thought_chars: usize,
    /// Live progress in short everyday phrases, with no tool names, arguments or results.
    pub progress_plain: bool,
    /// Stream the reasoning and the answer of model calls to the watcher as they are written.
    pub stream: bool,
    /// Longest run of answer text held back while waiting for the end of a sentence or line.
    pub stream_block_chars: usize,
    /// When a model call thinks.
    pub thinking: ThinkingRules,
    /// Fraction added to the estimated prompt when checked against the chat server slot context.
    pub prompt_estimate_headroom: f64,
}

impl Default for Agent {
    fn default() -> Self {
        Self {
            max_steps: 8,
            max_model_retries: 2,
            request_timeout_secs: 90,
            tool_timeout_secs: 20,
            answer_reserve_secs: 25,
            max_tool_calls: 10,
            confirmation_ttl_secs: 600,
            model_slots: 2,
            model_queue_wait_secs: 30,
            max_turns: 16,
            turn_queue_wait_secs: 20,
            temperature: 0.6,
            history_turns: 20,
            prompt_budget_tokens: 4_000,
            history_budget_tokens: 1_000,
            memory_budget_tokens: 300,
            capabilities_budget_tokens: 600,
            reply_budget_tokens: 200,
            max_images: 4,
            max_files: 4,
            max_file_bytes: 2_000_000,
            file_hosts: vec![
                "cdn.discordapp.com".to_owned(),
                "media.discordapp.net".to_owned(),
            ],
            upload_preview_chars: 800,
            upload_match_chars: 2_400,
            upload_ocr_pages: 3,
            chars_per_token: 4,
            retry_base_ms: 250,
            retry_cap_ms: 8_000,
            max_span_value_chars: 32_000,
            tool_result_to_file_chars: 0,
            progress_detail_chars: progress::DETAIL_CHARS,
            progress_thought_chars: progress::THOUGHT_CHARS,
            progress_plain: true,
            stream: true,
            stream_block_chars: 160,
            prompt_estimate_headroom: 0.3,
            thinking: ThinkingRules::default(),
        }
    }
}

impl Agent {
    /// How much detail a live progress line carries.
    pub fn progress_style(&self) -> ProgressStyle {
        ProgressStyle {
            detail_chars: self.progress_detail_chars,
            thought_chars: self.progress_thought_chars,
            plain: self.progress_plain,
        }
    }

    /// The prompt budgets these settings describe.
    pub fn budget(&self) -> Budget {
        Budget {
            total: self.prompt_budget_tokens,
            history: self.history_budget_tokens,
            memory: self.memory_budget_tokens,
            capabilities: self.capabilities_budget_tokens,
            reply: self.reply_budget_tokens,
            chars_per_token: self.chars_per_token,
        }
    }
}

impl Default for ThinkingRules {
    fn default() -> Self {
        Self {
            mode: ThinkingMode::Auto,
            after_tools: false,
            max_quick_chars: 160,
            plan_searches: false,
            recover: true,
            cues: [
                "why",
                "compare",
                "difference",
                "explain",
                "should i",
                "recommend",
                "plan",
                "pros and cons",
            ]
            .map(str::to_owned)
            .to_vec(),
            retry_without: true,
        }
    }
}

/// Rejects loop limits and budgets that leave the loop unable to run.
pub fn validate_agent(agent: &Agent) -> Result<(), ConfigError> {
    let invalid = |m: &str| Err(ConfigError::Invalid(m.into()));
    if agent.chars_per_token == 0 {
        return invalid("agent.chars_per_token must be at least 1");
    }
    if agent.prompt_budget_tokens == 0 {
        return invalid("agent.prompt_budget_tokens must be at least 1");
    }
    if agent.max_steps == 0 {
        return invalid("agent.max_steps must be at least 1");
    }
    if agent.answer_reserve_secs >= agent.request_timeout_secs {
        return invalid("agent.answer_reserve_secs must be below agent.request_timeout_secs");
    }
    if !(0.0..=2.0).contains(&agent.prompt_estimate_headroom) {
        return invalid("agent.prompt_estimate_headroom must be between 0 and 2");
    }
    Ok(())
}

/// Rejects a thinking cue that holds no word to match.
pub fn validate_thinking(rules: &ThinkingRules) -> Result<(), ConfigError> {
    match rules.cues.iter().find(|cue| words(cue).is_empty()) {
        Some(cue) => Err(ConfigError::Invalid(format!(
            "agent.thinking.cues holds {cue:?}, which has no word to match"
        ))),
        None => Ok(()),
    }
}

/// The text the harness writes around every prompt. Changing any of it changes the prompt hash.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Prompt {
    /// System instructions. Overrides the built-in default; overridden by system_file.
    pub system: Option<String>,
    /// Path to a file holding the system instructions. Read once at boot.
    pub system_file: Option<String>,
    /// Line naming the user, with {user} and {roles}.
    pub role_line: String,
    /// Line naming a user who holds no roles, with {user}.
    pub role_line_no_roles: String,
    /// Heading above recalled memories.
    pub memory_header: String,
    /// What to remember and what never to store. Added to the system prompt in a private turn.
    pub memory_rules: String,
    /// How to find events and clubs from memories. Added to the system prompt in a private turn.
    pub discovery_rules: String,
    /// How to look ahead for the student. Added to the system prompt in a private turn.
    pub proactive_rules: String,
    /// How to get to know a new student. Added to a private turn while few memories describe them.
    pub onboarding_rules: String,
    /// Fact and preference memories at which a student counts as known.
    pub onboarded_at: usize,
    /// How to offer tap-to-answer choices. Added to a private turn from the web chat.
    pub widget_rules: String,
    /// Heading above what the model may do.
    pub capabilities_header: String,
    /// Heading above the message of ours a reply answers.
    pub reply_header: String,
    /// Line closing a tool result cut to fit the prompt, with {chars}.
    pub result_cut_line: String,
    /// Line naming the current date, with {date}.
    pub date_line: String,
    /// Line added to a call that is offered no tools.
    pub answer_only_line: String,
    /// Line sent back when a tool failed and the sandbox was not tried. Empty turns it off.
    pub sandbox_retry_line: String,
    /// Line naming a file the user attached and its text.
    pub upload_line: String,
    /// Line naming a file no text could be pulled from.
    pub upload_raw_line: String,
    /// Line naming a file read in the engine, for a host with no sandbox.
    pub upload_inline_line: String,
    /// Line naming a file the user attached that could not be opened, with {reason}.
    pub upload_failed_line: String,
    /// The question sent in place of an empty message that carries attachments.
    pub attachments_only_input: String,
    /// Hours from UTC the date is rendered in.
    pub utc_offset_hours: i32,
}

impl Default for Prompt {
    fn default() -> Self {
        Self {
            system: None,
            system_file: None,
            role_line: assemble::ROLE_LINE.into(),
            role_line_no_roles: assemble::ROLE_LINE_NO_ROLES.into(),
            memory_header: assemble::MEMORY_HEADER.into(),
            memory_rules: assemble::MEMORY_RULES.into(),
            discovery_rules: assemble::DISCOVERY_RULES.into(),
            proactive_rules: assemble::PROACTIVE_RULES.into(),
            onboarding_rules: assemble::ONBOARDING_RULES.into(),
            onboarded_at: assemble::ONBOARDED_AT,
            widget_rules: assemble::WIDGET_RULES.into(),
            capabilities_header: assemble::CAPABILITIES_HEADER.into(),
            reply_header: assemble::REPLY_HEADER.into(),
            result_cut_line: assemble::RESULT_CUT_LINE.into(),
            date_line: assemble::DATE_LINE.into(),
            answer_only_line: assemble::ANSWER_ONLY_LINE.into(),
            sandbox_retry_line: assemble::SANDBOX_RETRY_LINE.into(),
            upload_line: assemble::UPLOAD_LINE.into(),
            upload_raw_line: assemble::UPLOAD_RAW_LINE.into(),
            upload_inline_line: assemble::UPLOAD_INLINE_LINE.into(),
            upload_failed_line: assemble::UPLOAD_FAILED_LINE.into(),
            attachments_only_input: assemble::ATTACHMENTS_ONLY_INPUT.into(),
            utc_offset_hours: -7,
        }
    }
}
