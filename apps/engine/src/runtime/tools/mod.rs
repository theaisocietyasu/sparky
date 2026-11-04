//! Built-in tools and MCP-backed tools. Each declares a RiskClass.

pub mod account;
pub mod files;
pub mod http;
pub mod knowledge;
pub mod mcp;
pub mod memory;
pub mod papers;
pub mod sandbox;
pub mod upcoming;
pub mod wiki;

/// The structured payload a tool hands back beside its text; unserializable values log, yield None.
pub fn structured<T: serde::Serialize>(value: &T) -> Option<serde_json::Value> {
    match serde_json::to_value(value) {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::error!(error = %error, "tool payload would not serialize");
            None
        }
    }
}

/// What an integration adds: tools for the model, and sources for the upcoming tool.
#[derive(Default)]
pub struct Built {
    /// Tools to register.
    pub tools: Vec<std::sync::Arc<dyn crate::core::traits::tools::Tool>>,
    /// Sources the upcoming tool reads.
    pub upcoming: Vec<std::sync::Arc<dyn crate::core::traits::tools::upcoming::UpcomingSource>>,
}

impl From<Vec<std::sync::Arc<dyn crate::core::traits::tools::Tool>>> for Built {
    fn from(tools: Vec<std::sync::Arc<dyn crate::core::traits::tools::Tool>>) -> Self {
        Self {
            tools,
            upcoming: Vec::new(),
        }
    }
}
