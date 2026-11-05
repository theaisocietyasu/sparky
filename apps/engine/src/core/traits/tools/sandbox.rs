//! Sandbox trait.

use async_trait::async_trait;

use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::sandbox::{
    SandboxCommand, SandboxError, SandboxOutput, SandboxRequest, SandboxSession,
};

/// Runs a command in an isolated environment.
#[async_trait]
pub trait Sandbox: Send + Sync {
    /// Runs one command and returns what it produced.
    async fn run(
        &self,
        ctx: &RequestContext,
        request: &SandboxRequest,
    ) -> Result<SandboxOutput, SandboxError>;

    /// Whether the tool is offered to the model.
    fn enabled(&self) -> bool;

    /// Offers the tool, or stops offering it. Containers already up are left alone.
    fn set_enabled(&self, on: bool);

    /// Whether it holds the PDF and scan converters an attached file is opened with.
    fn converts(&self) -> bool {
        true
    }

    /// The session containers running now, newest use first.
    fn sessions(&self) -> Vec<SandboxSession>;

    /// The commands it ran, newest first, up to what it keeps.
    fn commands(&self) -> Vec<SandboxCommand>;

    /// Removes one session container by name. Removing one that is gone is not a failure.
    async fn kill(&self, name: &str) -> Result<(), SandboxError>;

    /// Writes content into a session workspace and returns the path it landed at.
    async fn put(
        &self,
        ctx: &RequestContext,
        session: &str,
        name: &str,
        content: &[u8],
    ) -> Result<String, SandboxError>;
}
