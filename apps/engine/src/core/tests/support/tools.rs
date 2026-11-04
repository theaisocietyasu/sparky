//! Tool doubles: echo, slow, ordered, and failing tools.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};

/// Returns its arguments as text.
pub struct Echo(pub RiskClass);

#[async_trait]
impl Tool for Echo {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "echo".into(),
            description: "echoes".into(),
            parameters: json!({"type": "object"}),
            risk: self.0,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }
    async fn call(&self, _ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        Ok(output(args.to_string()))
    }
}

/// A tool answering to whatever name it is built with.
pub struct Named(pub &'static str);

#[async_trait]
impl Tool for Named {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.0.into(),
            description: "does whatever it is named for".into(),
            parameters: json!({"type": "object"}),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }
    async fn call(&self, _ctx: &RequestContext, _args: Value) -> Result<ToolOutput, ToolError> {
        Ok(output("what the page says"))
    }
}

/// Sleeps past any reasonable tool timeout.
pub struct Slow;

#[async_trait]
impl Tool for Slow {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "slow".into(),
            description: "sleeps".into(),
            parameters: json!({"type": "object"}),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }
    async fn call(&self, _ctx: &RequestContext, _args: Value) -> Result<ToolOutput, ToolError> {
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(output("late"))
    }
}

/// Sequential tool: records call order by sleeping longer for smaller inputs.
pub struct Ordered(pub RiskClass);

#[async_trait]
impl Tool for Ordered {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ordered".into(),
            description: "stateful".into(),
            parameters: json!({"type": "object"}),
            risk: self.0,
            private: false,
            sequential: true,
            timeout_secs: None,
        }
    }
    async fn call(&self, _ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let n = args["n"].as_u64().unwrap_or(0);
        tokio::time::sleep(Duration::from_millis(40 * (4 - n))).await;
        Ok(output(n.to_string()))
    }
}

/// Always fails.
pub struct Boom;

#[async_trait]
impl Tool for Boom {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "boom".into(),
            description: "always fails".into(),
            parameters: serde_json::json!({"type": "object"}),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }

    async fn call(
        &self,
        _ctx: &RequestContext,
        _args: serde_json::Value,
    ) -> Result<ToolOutput, ToolError> {
        Err(ToolError::Failed("nope".into()))
    }
}

/// Text-only tool output.
fn output(content: impl Into<String>) -> ToolOutput {
    ToolOutput {
        content: content.into(),
        data: None,
        sources: Vec::new(),
    }
}
