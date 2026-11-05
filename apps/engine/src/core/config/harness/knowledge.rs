//! Settings for retrieval and live source queries.

use serde::Deserialize;

/// Knowledge search on the platform: how much comes back, and whether the query is embedded.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Retrieval {
    /// Chunks search_knowledge returns per call.
    pub top_k: usize,
    /// Send a query vector, so the platform runs its dense leg beside text search.
    pub dense: bool,
    /// Rows either side of a hit read back with it. 0 hands back the hit alone.
    pub window: i32,
    /// Days after which a stored page is marked as possibly out of date.
    pub stale_days: u32,
}

impl Default for Retrieval {
    fn default() -> Self {
        Self {
            top_k: 6,
            dense: true,
            window: 2,
            stale_days: 14,
        }
    }
}

/// How a live source query runs. The tools section decides whether it is offered at all.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Query {
    /// Budget for one live query, end to end. Overrides agent.tool_timeout_secs for this tool.
    pub timeout_secs: u64,
}

impl Default for Query {
    fn default() -> Self {
        Self { timeout_secs: 90 }
    }
}
