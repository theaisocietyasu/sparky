//! Chunked replies, failure lines, and the text of the memory commands.

use std::fmt::Write;

use crate::core::types::{EngineError, MemoryList};

/// The Discord hard limit on message content. bot.max_message_chars may go below it, never above.
pub const MAX_MESSAGE: usize = 2_000;

/// Splits text into messages of at most limit bytes on line, then space, boundaries.
pub fn chunk(text: &str, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        if rest.len() <= limit {
            out.push(rest.to_owned());
            break;
        }
        let mut cut = limit;
        while cut > 0 && !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        if cut == 0 {
            cut = rest.chars().next().map_or(rest.len(), char::len_utf8);
        }
        let window = &rest[..cut];
        let split = window
            .rfind('\n')
            .or_else(|| window.rfind(' '))
            .filter(|&i| i > limit / 4)
            .unwrap_or(cut);
        out.push(rest[..split].trim_end().to_owned());
        rest = rest[split..].trim_start();
    }
    out
}

/// What to say when Sparky cannot be reached or the request could not be served.
pub const UNAVAILABLE: &str = "Sparky is unavailable right now. Please try again shortly.";

/// What to say when the engine call fails: busy on 503, closed approval on 409, lost chat on 404.
pub fn failure(e: &EngineError) -> String {
    match e {
        EngineError::Status { status: 503, .. } => {
            "Sparky is busy with other questions right now. Try again in a moment.".into()
        }
        EngineError::Status { status: 409, .. } => "That approval is no longer open.".into(),
        EngineError::Status { status: 404, .. } => {
            "I lost track of that conversation. Ask again to start a new one.".into()
        }
        EngineError::Status { .. } | EngineError::Transport(_) => UNAVAILABLE.into(),
    }
}

/// What /memory says when the engine remembers nothing.
pub const NOTHING_REMEMBERED: &str = "I do not remember anything about you yet.";

/// What a presser sees when the engine refused or failed their approval press.
pub fn confirm_failure(e: &EngineError) -> String {
    if confirm_refused(e) {
        NOT_YOURS.into()
    } else {
        failure(e)
    }
}

/// What a presser sees when the approval is not theirs or no longer open.
pub const NOT_YOURS: &str = "This approval is not yours, or it is no longer open.";

/// Whether the engine refused an approval press: not the asker, or no longer open.
pub fn confirm_refused(e: &EngineError) -> bool {
    matches!(
        e,
        EngineError::Status {
            status: 404 | 409,
            ..
        }
    )
}

/// The numbered memories, split into sendable messages.
pub fn render_memories(list: &MemoryList, limit: usize) -> Vec<String> {
    if list.memories.is_empty() {
        return vec![NOTHING_REMEMBERED.to_owned()];
    }
    let mut body = String::from("**What I remember**\n");
    for m in &list.memories {
        let when = m
            .due
            .as_ref()
            .map_or_else(|| m.created_at.clone(), |due| format!("due {due}"));
        let _ = writeln!(body, "{}. {}: {} ({when})", m.number, m.kind, m.text);
    }
    body.push_str("\n/forget with a number drops one. /forget all clears everything.");
    chunk(&body, limit.clamp(1, MAX_MESSAGE))
}

/// What /forget says after the engine removed some memories. number is set for a single one.
pub fn forgot(removed: u64, number: Option<usize>) -> String {
    match (removed, number) {
        (0, Some(n)) => format!("I have no memory {n}. /memories shows the numbers."),
        (0, None) => "I did not remember anything about you.".into(),
        (_, Some(n)) => format!("Forgot memory {n}."),
        (1, None) => "Forgot 1 thing.".into(),
        (n, None) => format!("Forgot {n} things."),
    }
}

/// What to say when forgetting memory number failed. 404 means there is no such number.
pub fn forget_failure(e: &EngineError, number: usize) -> String {
    match e {
        EngineError::Status { status: 404, .. } => forgot(0, Some(number)),
        other => memory_failure(other),
    }
}

/// What to say when a memory call failed. 503 means the store is unreachable.
pub fn memory_failure(e: &EngineError) -> String {
    match e {
        EngineError::Status { status: 503, .. } => {
            "I cannot reach my memory right now. Try again soon.".into()
        }
        EngineError::Status { status: 404, .. } => UNAVAILABLE.into(),
        other => failure(other),
    }
}
