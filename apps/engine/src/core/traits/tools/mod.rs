//! Tool trait.

pub mod canvas;
pub mod files;
pub mod gcal;
pub mod outlook;
pub mod sandbox;

use async_trait::async_trait;
use serde_json::Value;

use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::{ToolDefinition, ToolError, ToolOutput};

/// A callable capability.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Name, description, schema, and risk.
    fn definition(&self) -> ToolDefinition;
    /// Whether it is offered to the model on this step. A tool switched off is not listed.
    fn available(&self) -> bool {
        true
    }
    /// Executes with validated JSON arguments.
    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError>;
}
