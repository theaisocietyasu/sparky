//! Which tools are registered, and the MCP servers.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::core::types::tools::RiskClass;

/// Which tools are registered.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Tools {
    /// Tool names never registered, whatever their source.
    pub disabled: Vec<String>,
    /// Register the Platform search tool.
    pub search: bool,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            disabled: Vec::new(),
            search: true,
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
