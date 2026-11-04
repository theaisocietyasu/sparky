//! The admin surface: how long the instructions may be and how much history a page reads.

use serde::Deserialize;

/// The admin routes. They are served while the web sign-in is on, to a session with the write role.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Admin {
    /// The longest system instructions a moderator may set, in characters.
    pub max_instructions_chars: usize,
    /// Changes GET /admin/history returns, newest first.
    pub history_limit: usize,
    /// Traces GET /admin/traces returns, newest first.
    pub traces_limit: usize,
}

impl Default for Admin {
    fn default() -> Self {
        Self {
            max_instructions_chars: 20_000,
            history_limit: 50,
            traces_limit: 100,
        }
    }
}
