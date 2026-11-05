//! Plain progress: short everyday phrases for a student, with no tool names, arguments or results.

use serde_json::Value;

use crate::core::types::safety::policy::Decision;
use crate::core::types::tools::{SEARCH_KNOWLEDGE, SEARCH_LIVE};
use crate::core::types::trace::TraceEvent;

/// Characters of a search query a plain line quotes.
const QUERY_CHARS: usize = 48;

/// What a tool does, as a line while it runs and a line once it has run.
struct Action {
    running: String,
    done: String,
    place: String,
}

impl TraceEvent {
    /// A plain line for a student, or None when the event says nothing a student needs.
    pub fn plain(&self) -> Option<String> {
        match self {
            Self::ModelStarted { .. } => Some("\u{1f914} Thinking".to_owned()),
            Self::ToolStarted {
                tool, arguments, ..
            } => Some(format!("\u{1f527} {}", action(tool, arguments).running)),
            Self::ToolCall {
                tool,
                arguments,
                result,
                ..
            } => {
                let action = action(tool, arguments);
                Some(match result {
                    Ok(_) => format!("\u{2705} {}", action.done),
                    Err(_) => format!("\u{274c} Could not reach {}", action.place),
                })
            }
            Self::MemoryWritten { .. } => Some("\u{1f4dd} Noted for later".to_owned()),
            Self::MemoryForgotten => Some("\u{1f5d1} Forgot that".to_owned()),
            Self::GuardrailBlocked { .. } => Some("\u{1f6d1} That was not allowed".to_owned()),
            Self::FileAttached {
                name, error: None, ..
            } => Some(format!("\u{1f4ce} Read {name}")),
            Self::FileAttached {
                name,
                error: Some(_),
                ..
            } => Some(format!("\u{1f4ce} Could not open {name}")),
            Self::PolicyDecision { decision, .. } => match decision {
                Decision::Deny { .. } => Some("\u{1f6ab} That is not allowed here".to_owned()),
                Decision::Confirm(_) => Some("\u{270b} Waiting for your OK".to_owned()),
                Decision::Allow => None,
            },
            Self::ModelError { retried: true, .. } => Some("\u{1f504} Trying again".to_owned()),
            _ => None,
        }
    }
}

/// The words a tool is shown with.
fn action(tool: &str, arguments: &Value) -> Action {
    let query = text_arg(arguments, "query").map(|q| quoted(&q));
    let simple = |running: &str, done: &str, place: &str| Action {
        running: running.to_owned(),
        done: done.to_owned(),
        place: place.to_owned(),
    };
    match tool {
        SEARCH_KNOWLEDGE => Action {
            running: match &query {
                Some(q) => format!("Searching ASU pages for {q}"),
                None => "Searching ASU pages".to_owned(),
            },
            done: "Checked ASU pages".to_owned(),
            place: "ASU pages".to_owned(),
        },
        SEARCH_LIVE => {
            let source = text_arg(arguments, "source").unwrap_or_default();
            let place = live_place(&source);
            Action {
                running: match (&query, source.as_str()) {
                    (Some(q), "web") => format!("Searching the web for {q}"),
                    _ => format!("Checking {place}"),
                },
                done: format!("Checked {place}"),
                place,
            }
        }
        "platform_asu_clubs" => simple(
            "Looking through clubs on Sun Devil Central",
            "Looked through clubs",
            "Sun Devil Central",
        ),
        "platform_asu_events" => simple(
            "Looking for events on Sun Devil Central",
            "Looked for events",
            "Sun Devil Central",
        ),
        "canvas_deadlines" => simple("Checking your Canvas", "Checked your Canvas", "Canvas"),
        "upcoming" => simple(
            "Checking what is coming up",
            "Checked what is coming up",
            "your schedule",
        ),
        "search_papers" => simple(
            "Searching research papers",
            "Searched research papers",
            "the paper search",
        ),
        "search_chats" => simple(
            "Looking back at your chats",
            "Looked back at your chats",
            "your chats",
        ),
        "calculate" => simple("Working it out", "Worked it out", "the calculator"),
        "read_page" => simple("Reading the page", "Read the page", "the page"),
        "wikipedia_lookup" => simple(
            "Looking it up on Wikipedia",
            "Looked it up on Wikipedia",
            "Wikipedia",
        ),
        "remember" => simple("Saving that", "Saved that", "your memories"),
        "forget_memory" => simple("Forgetting that", "Forgot that", "your memories"),
        "remind" => simple("Setting a reminder", "Set a reminder", "reminders"),
        "cancel_reminder" => simple("Cancelling a reminder", "Cancelled a reminder", "reminders"),
        "run_sandbox" => simple("Working it out", "Worked it out", "the workspace"),
        other => {
            let words = other
                .trim_start_matches("platform_")
                .replace(['_', '.'], " ");
            Action {
                running: format!("Using {words}"),
                done: format!("Used {words}"),
                place: words,
            }
        }
    }
}

/// What a live source is called in a plain line.
fn live_place(source: &str) -> String {
    match source {
        "courses" => "the class search".to_owned(),
        "library" | "library_hours" => "library hours".to_owned(),
        "events" => "campus events".to_owned(),
        "clubs" => "clubs".to_owned(),
        "shuttles" => "shuttle times".to_owned(),
        "study_rooms" => "study rooms".to_owned(),
        "dining" => "dining".to_owned(),
        "web" => "the web".to_owned(),
        "" => "live ASU sources".to_owned(),
        other => other.replace('_', " "),
    }
}

/// A string argument of a call, trimmed, or None when it is missing or empty.
fn text_arg(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

/// A query in curly quotes, held to a short length.
fn quoted(query: &str) -> String {
    let one_line = query.split_whitespace().collect::<Vec<_>>().join(" ");
    let shown = if one_line.chars().count() <= QUERY_CHARS {
        one_line
    } else {
        let kept: String = one_line.chars().take(QUERY_CHARS - 1).collect();
        format!("{}\u{2026}", kept.trim_end())
    };
    format!("\u{201c}{shown}\u{201d}")
}
