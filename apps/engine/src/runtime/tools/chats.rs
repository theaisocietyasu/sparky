//! search_chats: what the student and Sparky said in earlier private chats.

use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};

/// Name of the chat search tool.
pub const SEARCH_CHATS: &str = "search_chats";

/// Searches the caller's earlier private chats.
pub struct ChatSearchTool {
    store: Arc<dyn ConversationStore>,
    max_hits: usize,
}

impl ChatSearchTool {
    /// A search over store returning at most max_hits messages.
    pub fn new(store: Arc<dyn ConversationStore>, max_hits: usize) -> Self {
        Self { store, max_hits }
    }
}

#[async_trait]
impl Tool for ChatSearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: SEARCH_CHATS.to_owned(),
            description: "Search what this student and you said in earlier chats. Use it when they \
                          refer to an earlier conversation (last time, that class I mentioned, what \
                          you told me about) or when an earlier chat would help and your memories \
                          do not hold it. Send keywords, not a sentence."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Keywords to find, such as internship resume Intel."}
                },
                "required": ["query"]
            }),
            risk: RiskClass::ReadAuthenticated,
            private: true,
            sequential: false,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .ok_or_else(|| ToolError::InvalidArguments("query is required".into()))?;
        let hits = self
            .store
            .search(ctx, query, self.max_hits)
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        if hits.is_empty() {
            return Ok(ToolOutput {
                content: format!("No earlier chat mentions {query}."),
                data: None,
                sources: Vec::new(),
            });
        }
        let mut out = String::new();
        for hit in &hits {
            let who = if hit.role == "user" {
                "They said"
            } else {
                "You said"
            };
            let day = hit.at.get(..10).unwrap_or(&hit.at);
            let _ = writeln!(out, "{day}, {who}: {}", hit.snippet.trim());
        }
        Ok(ToolOutput {
            content: out.trim_end().to_owned(),
            data: None,
            sources: Vec::new(),
        })
    }
}

/// The chat search tool over store.
pub fn tools(store: Arc<dyn ConversationStore>, max_hits: usize) -> Vec<Arc<dyn Tool>> {
    let tool: Arc<dyn Tool> = Arc::new(ChatSearchTool::new(store, max_hits));
    vec![tool]
}
