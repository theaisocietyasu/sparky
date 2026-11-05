//! Which tools are registered, how the search tools are worded, and the MCP servers.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::core::types::tools::RiskClass;

/// Default description of search_knowledge.
pub const KNOWLEDGE_DESCRIPTION: &str = "Search the stored knowledge base of ASU pages: programs, policies, admissions, advising, \
     buildings, services and offices, anything that changes rarely. Nothing is looked up \
     before you are called, so search it for any such question. Send several searches with \
     different wording in one step.";

/// Default description of search_live.
pub const LIVE_DESCRIPTION: &str = "Fetch an ASU source or the open web right now. Use it when the answer has to be current, \
     such as hours today, open seats, shuttle times, events, news or scores, or when the \
     knowledge base holds nothing.";

/// Default description of the query parameter of both search tools.
pub const QUERY_DESCRIPTION: &str = "What to search for, in keywords. This string is the whole request: the tool reads nothing \
     else from the conversation, so name the subject in full and write no pronoun and no bare \
     word. Bad: hours. Good: Hayden Library hours Sunday. Bad: does it have space left. Good: \
     CSE 310 Fall 2026 open seats.";

/// Default lead of the source parameter of both search tools.
pub const SOURCE_DESCRIPTION: &str = "Narrows the search to one source. Leave it out unless the question is plainly about that \
     one source: a wrong source returns nothing even when the answer is stored.";

/// Default answer of search_knowledge when the index holds nothing for the query.
pub const NOTHING_STORED: &str = "The knowledge base holds nothing for that query. If you named a source, search again \
     without it. Otherwise search again with the subject named differently, read the page \
     with read_page when you know its url, or call search_live. Say you do not have it only \
     once all of that has been tried this turn.";

/// Default note on a stored passage whose page is old.
pub const STALE_NOTE: &str = "This copy is old. If the answer depends on details that change, read the url with \
     read_page for the current page.";

/// Which tools are registered and how the two search tools are worded.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Tools {
    /// Tool names never registered, whatever their source.
    pub disabled: Vec<String>,
    /// Register search_knowledge and search_live over the sources the platform has published.
    pub search: bool,
    /// Source search_live fetches when a call names none.
    pub live_default_source: String,
    /// What search_knowledge does, for the model.
    pub knowledge_description: String,
    /// What search_live does, for the model.
    pub live_description: String,
    /// What the query parameter of both search tools must carry.
    pub query_description: String,
    /// Lead of the source parameter, before the key and hint of each source.
    pub source_description: String,
    /// What search_knowledge answers when the index holds nothing.
    pub nothing_stored: String,
    /// Note on a stored passage older than retrieval.stale_days.
    pub stale_note: String,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            disabled: Vec::new(),
            search: true,
            live_default_source: "web".into(),
            knowledge_description: KNOWLEDGE_DESCRIPTION.into(),
            live_description: LIVE_DESCRIPTION.into(),
            query_description: QUERY_DESCRIPTION.into(),
            source_description: SOURCE_DESCRIPTION.into(),
            nothing_stored: NOTHING_STORED.into(),
            stale_note: STALE_NOTE.into(),
        }
    }
}

/// MCP servers exposed as tools.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Mcp {
    /// Servers to connect to at boot. Expressed in the TOML layer.
    pub servers: Vec<McpServer>,
    /// Default for a server that does not set required_props_only.
    pub required_props_only: bool,
    /// Longest tool result handed back to the model.
    pub max_output_chars: usize,
    /// Longest per-property description kept in a tool schema.
    pub max_schema_description_chars: usize,
    /// Longest tool description kept.
    pub max_tool_description_chars: usize,
}

/// One MCP server.
#[derive(Debug, Clone, Deserialize)]
pub struct McpServer {
    /// Name used in logs and errors.
    pub name: String,
    /// Streamable-HTTP endpoint, e.g. http://localhost:8931/mcp.
    pub url: String,
    /// Remote tool names to expose; empty exposes every tool the server lists.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Overrides mcp.required_props_only for this server.
    #[serde(default)]
    pub required_props_only: Option<bool>,
    /// Overrides agent.tool_timeout_secs for the tools of this server.
    #[serde(default)]
    pub tool_timeout_secs: Option<u64>,
    /// Risk class per remote tool name, in place of the class derived from the name.
    #[serde(default)]
    pub risks: BTreeMap<String, RiskClass>,
}

impl Mcp {
    /// Configured servers, with empty URLs dropped.
    pub fn resolved_servers(&self) -> Vec<McpServer> {
        self.servers
            .iter()
            .filter(|s| !s.url.trim().is_empty())
            .cloned()
            .collect()
    }
}

impl Default for Mcp {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            required_props_only: true,
            max_output_chars: 6_000,
            max_schema_description_chars: 80,
            max_tool_description_chars: 160,
        }
    }
}
