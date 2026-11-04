//! The wire shapes of POST /memories/list and POST /memories/forget.

use serde::{Deserialize, Serialize};

/// Asks for every memory of one user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryListRequest {
    /// The user whose memories these are.
    pub user_id: String,
    /// Guild the request belongs to.
    #[serde(default)]
    pub tenant_id: Option<String>,
}

/// One memory as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedMemory {
    /// Position in the list, from 1. /memories/forget takes it.
    pub number: usize,
    /// Short id, the first 8 hex digits.
    pub id: String,
    /// fact, preference or task.
    pub kind: String,
    /// The remembered line.
    pub text: String,
    /// Day it was written, YYYY-MM-DD.
    pub created_at: String,
    /// Day a task is due, YYYY-MM-DD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

/// Every unexpired memory of the user, in recall order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryList {
    /// Numbered from 1.
    pub memories: Vec<ListedMemory>,
}

/// Removes one memory by its number in the list, or every memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryForgetRequest {
    /// The user whose memories these are.
    pub user_id: String,
    /// Guild the request belongs to.
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// Number from the list. Absent removes every memory.
    #[serde(default)]
    pub number: Option<usize>,
}

/// How many memories went.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryForgetResponse {
    /// Memories removed.
    pub removed: u64,
}
