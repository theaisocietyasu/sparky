//! A command in an isolated container: read-only root, capped resources, non-root user, restricted network.

pub(crate) mod container;
mod egress;
pub(crate) mod output;
mod session;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::core::traits::tools::Tool;
use crate::core::traits::tools::sandbox::Sandbox;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::sandbox::{SandboxError, SandboxRequest};
use crate::core::types::tools::{RiskClass, SANDBOX, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::structured;

pub use self::container::ContainerSandbox;
pub use self::session::reap_sessions;

/// What the tool tells the model about itself.
#[derive(Debug, Clone)]
pub struct Wording {
    /// What the tool is for.
    pub description: String,
    /// What the command argument is.
    pub command: String,
    /// What the session argument is.
    pub session: String,
}

impl From<&crate::core::config::SandboxSettings> for Wording {
    fn from(cfg: &crate::core::config::SandboxSettings) -> Self {
        Self {
            description: cfg.description.clone(),
            command: cfg.command_description.clone(),
            session: cfg.session_description.clone(),
        }
    }
}

/// Offers the sandbox to the model.
pub struct SandboxTool {
    sandbox: Arc<dyn Sandbox>,
    risk: RiskClass,
    wording: Wording,
}

impl SandboxTool {
    /// Builds the tool. The risk class it declares is what Policy gates it by.
    pub fn new(sandbox: Arc<dyn Sandbox>, risk: RiskClass, wording: Wording) -> Self {
        Self {
            sandbox,
            risk,
            wording,
        }
    }
}

#[async_trait]
impl Tool for SandboxTool {
    fn available(&self) -> bool {
        self.sandbox.enabled()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: SANDBOX.to_owned(),
            description: self.wording.description.clone(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": self.wording.command },
                    "session": { "type": "string", "description": self.wording.session }
                },
                "required": ["command"]
            }),
            risk: self.risk,
            // Sandbox calls within one step run one at a time.
            private: false,
            sequential: true,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let request: SandboxRequest =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let out = self.sandbox.run(ctx, &request).await.map_err(|e| match e {
            SandboxError::Refused(reason) => ToolError::InvalidArguments(reason),
            SandboxError::Timeout => ToolError::Timeout,
            run @ SandboxError::Runtime(_) => ToolError::Failed(run.to_string()),
        })?;
        let mut text = format!("exit {}", out.exit_code);
        if !out.stdout.trim().is_empty() {
            text.push_str("\nstdout:\n");
            text.push_str(&out.stdout);
        }
        if !out.stderr.trim().is_empty() {
            text.push_str("\nstderr:\n");
            text.push_str(&out.stderr);
        }
        Ok(ToolOutput {
            content: text,
            data: structured(&out),
            sources: Vec::new(),
        })
    }
}
