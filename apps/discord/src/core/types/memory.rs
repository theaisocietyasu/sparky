//! What the engine remembers about a user, and forgetting it.

use serde::{Deserialize, Serialize};

/// Body of POST /memories/list.
#[derive(Debug, Serialize)]
pub struct MemoryRequest {
    /// Discord user id.
    pub user_id: String,
    /// Guild id.
    pub tenant_id: String,
}

/// Every memory the engine keeps about one user, numbered from 1.
#[derive(Debug, Default, Deserialize)]
pub struct MemoryList {
    /// In recall order: tasks due soon first, then newest first.
    #[serde(default)]
    pub memories: Vec<ListedMemory>,
}

/// One memory.
#[derive(Debug, Clone, Deserialize)]
pub struct ListedMemory {
    /// Position in the list; /forget takes it.
    pub number: usize,
    /// fact, preference or task.
    pub kind: String,
    /// The remembered line.
    pub text: String,
    /// Day it was written, YYYY-MM-DD.
    pub created_at: String,
    /// Day a task is due, YYYY-MM-DD.
    #[serde(default)]
    pub due: Option<String>,
}

/// Body of POST /memories/forget. No number removes everything.
#[derive(Debug, Serialize)]
pub struct ForgetRequest {
    /// Discord user id.
    pub user_id: String,
    /// Guild id.
    pub tenant_id: String,
    /// Number from /memories.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<usize>,
}

/// Reply to POST /memories/forget.
#[derive(Debug, Deserialize)]
pub struct ForgetResponse {
    /// Memories removed.
    pub removed: u64,
}
