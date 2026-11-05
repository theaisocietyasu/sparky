//! Settings for a Platform search.

use serde::Deserialize;

/// How a Platform search runs. The tools section decides whether it is offered at all.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Query {
    /// Budget for one search, end to end. Overrides agent.tool_timeout_secs for this tool.
    pub timeout_secs: u64,
}

impl Default for Query {
    fn default() -> Self {
        Self { timeout_secs: 90 }
    }
}
