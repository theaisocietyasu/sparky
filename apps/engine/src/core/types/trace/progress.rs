//! Progress: the live progress event on the wire, how much detail it carries, and its rendering.

use serde::Serialize;
use serde_json::Value;

use crate::core::types::safety::policy::Decision;
use crate::core::types::tools::{SEARCH_KNOWLEDGE, SEARCH_LIVE};
use crate::core::types::trace::TraceEvent;

/// How much of a tool call, a result, or a thought one progress line shows.
#[derive(Debug, Clone, Copy)]
pub struct ProgressStyle {
    /// Characters kept from each piece of detail on a line.
    pub detail_chars: usize,
    /// Characters of reasoning a thinking line keeps.
    pub thought_chars: usize,
}

/// Characters of detail a progress line carries when nothing is configured.
pub const DETAIL_CHARS: usize = 160;

/// Characters of reasoning a thinking line carries when nothing is configured.
pub const THOUGHT_CHARS: usize = 600;

impl Default for ProgressStyle {
    fn default() -> Self {
        Self {
            detail_chars: DETAIL_CHARS,
            thought_chars: THOUGHT_CHARS,
        }
    }
}

/// One line of progress for a run's watcher. The engine renders text; a client just displays it.
#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    /// Snake-case name of the trace event this came from.
    pub event: &'static str,
    /// Ready-to-display sentence.
    pub text: String,
    /// The line this replaces, when it replaces one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    /// Removes the line of slot instead of writing text there.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
    /// The text is the answer so far, not a step.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub draft: bool,
    /// The event itself, for clients that want more than text.
    pub detail: TraceEvent,
}

impl Progress {
    /// The progress line for an event, or None when the event is bookkeeping.
    pub fn of(event: &TraceEvent, style: ProgressStyle) -> Option<Self> {
        let slot = event.slot();
        let clear = event.clears_slot() && slot.is_some();
        let text = event.progress(style);
        if text.is_none() && !clear {
            return None;
        }
        Some(Self {
            event: event.kind(),
            text: text.unwrap_or_default(),
            slot,
            clear,
            draft: event.is_draft(),
            detail: event.clone(),
        })
    }
}

impl TraceEvent {
    /// What to show someone waiting on this run, or None when the event is bookkeeping.
    pub fn progress(&self, style: ProgressStyle) -> Option<String> {
        let detail = style.detail_chars;
        match self {
            Self::ModelStarted { .. } => Some("\u{1f914} thinking".to_owned()),
            Self::ModelReasoning { text, .. } => {
                let so_far = tail(&one_line(text), style.thought_chars);
                (!so_far.is_empty()).then(|| format!("\u{1f914} {so_far}"))
            }
            Self::ModelThought { text, .. } => {
                let thought = clip(&one_line(text), style.thought_chars);
                (!thought.is_empty()).then(|| format!("\u{1f4ad} {thought}"))
            }
            Self::AnswerDraft { text, .. } => {
                let draft = text.trim();
                (!draft.is_empty()).then(|| draft.to_owned())
            }
            Self::ToolStarted {
                tool, arguments, ..
            } => Some(format!(
                "\u{1f527} `{}`{} \u{2014} {}",
                tool,
                call_arguments(arguments, detail),
                running(tool)
            )),
            Self::ToolCall {
                tool,
                arguments,
                result,
                ..
            } => {
                let head = format!("`{}`{}", tool, call_arguments(arguments, detail));
                Some(match result {
                    Ok(output) => {
                        let shown = clip(&one_line(output), detail);
                        if shown.is_empty() {
                            format!("\u{2705} {head} \u{2014} nothing came back")
                        } else {
                            format!("\u{2705} {head} \u{2192} {shown}")
                        }
                    }
                    Err(error) => format!(
                        "\u{274c} {head} \u{2014} {}",
                        clip(&one_line(error), detail)
                    ),
                })
            }
            Self::MemoryRecalled { count: 0 } => None,
            Self::MemoryRecalled { count } => Some(format!(
                "\u{1f9e0} remembering {count} {} about you",
                plural(*count, "thing", "things")
            )),
            Self::MemoryWritten { .. } => Some("\u{1f4dd} remembering that".to_owned()),
            Self::MemoryForgotten => Some("\u{1f5d1} forgetting that".to_owned()),
            Self::GuardrailBlocked { stage, .. } => {
                Some(format!("\u{1f6d1} the {} was not allowed", stage.as_str()))
            }
            Self::Compaction { turns } => Some(format!(
                "\u{1f5dc}\u{fe0f} summarising {turns} earlier messages"
            )),
            Self::FileAttached {
                name, error: None, ..
            } => Some(format!("\u{1f4ce} opened `{name}`")),
            Self::FileAttached {
                name,
                error: Some(_),
                ..
            } => Some(format!("\u{1f4ce} could not open `{name}`")),
            Self::PolicyDecision { tool, decision, .. } => match decision {
                Decision::Deny { .. } => Some(format!("\u{1f6ab} `{tool}` was not allowed")),
                Decision::Confirm(_) => Some(format!("\u{270b} `{tool}` needs your approval")),
                Decision::Allow => None,
            },
            Self::ModelError { retried: true, .. } => {
                Some("\u{1f504} the model stumbled, retrying".to_owned())
            }
            Self::GuardrailRedacted { .. }
            | Self::ToolResultStored { .. }
            | Self::RequestStarted { .. }
            | Self::ContextAssembled { .. }
            | Self::ModelCall { .. }
            | Self::ModelAnswered { .. }
            | Self::AnswerDraftCleared { .. }
            | Self::ModelError { .. }
            | Self::Completed { .. } => None,
        }
    }
}

/// A compact rendering of the arguments a call was made with. Empty when there are none.
fn call_arguments(arguments: &Value, limit: usize) -> String {
    let Some(fields) = arguments.as_object() else {
        return String::new();
    };
    let shown: Vec<String> = fields
        .iter()
        .map(|(key, value)| match value {
            Value::String(text) => format!("{key}: {text}"),
            other => format!("{key}: {other}"),
        })
        .collect();
    if shown.is_empty() {
        return String::new();
    }
    format!(" ({})", clip(&one_line(&shown.join(", ")), limit))
}

/// The last limit characters of text, with an ellipsis in front when it was cut.
fn tail(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().skip(count - limit.saturating_sub(1)).collect();
    format!("\u{2026}{}", kept.trim_start())
}

/// Text as one line, with runs of whitespace collapsed.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text held to limit characters, with an ellipsis when it was cut.
fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{}\u{2026}", kept.trim_end())
}

/// What a tool is shown as while it runs. A search tool names what it searches.
fn running(tool: &str) -> String {
    match tool {
        SEARCH_KNOWLEDGE => "searching the knowledge base".to_owned(),
        SEARCH_LIVE => "searching live".to_owned(),
        _ => "running".to_owned(),
    }
}

/// The singular form for a count of one, the plural otherwise.
fn plural(count: usize, one: &'static str, many: &'static str) -> &'static str {
    if count == 1 { one } else { many }
}
