//! ContainerSandbox: runs commands in a sealed container through the container runtime.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::core::traits::tools::sandbox::Sandbox;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::safety::redact::redact_text;
use crate::core::types::tools::sandbox::{
    Limits, SandboxCommand, SandboxError, SandboxOutput, SandboxRequest, SandboxSession,
    session_name, workspace_path,
};
use crate::runtime::tools::sandbox::log::CommandLog;
pub(super) use crate::runtime::tools::sandbox::log::held;
use crate::runtime::tools::sandbox::output::{bounded_output, clip};
use crate::runtime::tools::sandbox::session::Live;

/// Directory the workspace is mounted at inside the container.
pub const WORKSPACE: &str = "/tmp";

/// Label every container the engine starts carries, so an operator can find them all.
pub const LABEL: &str = "sparky.sandbox";

/// Label naming which engine instance started a container.
pub const INSTANCE_LABEL: &str = "sparky.sandbox.instance";

/// Prefix of every session container name.
pub(super) const SESSION_PREFIX: &str = "sparky-sb-";

/// Runs commands in a container. Clones share the session registry and the command log.
#[derive(Debug, Clone)]
pub struct ContainerSandbox {
    pub(super) limits: Limits,
    /// Container name to what is known about it, for the idle sweep and the per-caller cap.
    pub(super) sessions: Arc<Mutex<HashMap<String, Live>>>,
    /// The commands running and the ones that ran.
    pub(super) log: CommandLog,
    /// Whether the tool is offered. An operator turns it off without restarting the engine.
    pub(super) enabled: Arc<AtomicBool>,
    /// One permit per command allowed to run at once, across every caller.
    pub(super) slots: Arc<tokio::sync::Semaphore>,
}

impl Default for ContainerSandbox {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}

impl ContainerSandbox {
    /// Builds the sandbox over its limits.
    pub fn new(limits: Limits) -> Self {
        Self {
            slots: Arc::new(tokio::sync::Semaphore::new(limits.max_running.max(1))),
            log: CommandLog::new(limits.recent_commands),
            limits,
            sessions: Arc::default(),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    /// The prefix every session container of one caller shares.
    pub(super) fn owner_prefix(ctx: &RequestContext) -> String {
        let mut hasher = DefaultHasher::new();
        (&ctx.tenant_id, &ctx.user_id).hash(&mut hasher);
        format!("{SESSION_PREFIX}{:016x}-", hasher.finish())
    }

    /// The container name a session runs under, scoped to the tenant and user.
    pub fn container_name(ctx: &RequestContext, session: &str) -> String {
        format!("{}{session}", Self::owner_prefix(ctx))
    }

    /// Whether the runtime answers, with egress made ready. Called once at boot.
    pub async fn probe(&self) -> Result<(), SandboxError> {
        let out = Command::new(&self.limits.runtime)
            .arg("version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|e| {
                SandboxError::Runtime(format!("{} did not start: {e}", self.limits.runtime))
            })?;
        if out.status.success() {
            return self.prepare_egress().await;
        }
        Err(SandboxError::Runtime(format!(
            "{} answered: {}",
            self.limits.runtime,
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }

    /// Runs one runtime subcommand and reports whether it succeeded.
    pub(super) async fn runtime_ok(&self, args: &[&str]) -> bool {
        Command::new(&self.limits.runtime)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .status()
            .await
            .is_ok_and(|status| status.success())
    }

    /// Whether a container of this name exists and is running.
    pub(super) async fn running(&self, name: &str) -> bool {
        let out = Command::new(&self.limits.runtime)
            .args(["inspect", "--format", "{{.State.Running}}", name])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await;
        out.is_ok_and(|out| {
            out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true"
        })
    }

    /// Removes a container and forgets it.
    pub(super) async fn remove(&self, name: &str) {
        self.runtime_ok(&["rm", "--force", name]).await;
        held(&self.sessions).remove(name);
    }

    /// The arguments that seal a container.
    pub fn seal(&self) -> Vec<String> {
        let l = &self.limits;
        let network = l.egress.as_ref().map_or("none", |e| e.network.as_str());
        let mut args: Vec<String> = [
            "--label",
            LABEL,
            "--network",
            network,
            "--read-only",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--user",
            "65534:65534",
            "--memory",
            &l.memory,
            "--cpus",
            &l.cpus,
            "--pids-limit",
            &l.pids.to_string(),
            "--tmpfs",
            &format!("{WORKSPACE}:rw,noexec,nosuid,size={}m", l.workspace_mb),
            "--workdir",
            WORKSPACE,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        args.push("--label".to_owned());
        args.push(format!("{INSTANCE_LABEL}={}", l.instance));
        if let Some(egress) = &l.egress {
            let url = egress.proxy_url();
            for var in ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"] {
                args.push("--env".to_owned());
                args.push(format!("{var}={url}"));
            }
        }
        args
    }

    /// Runs one runtime subcommand, failing with what it printed.
    pub(super) async fn runtime(&self, args: &[&str]) -> Result<(), SandboxError> {
        let out = Command::new(&self.limits.runtime)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|e| {
                SandboxError::Runtime(format!("{} did not start: {e}", self.limits.runtime))
            })?;
        if out.status.success() {
            return Ok(());
        }
        Err(SandboxError::Runtime(format!(
            "{} {} failed: {}",
            self.limits.runtime,
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }

    /// The trimmed output of a runtime subcommand, or None when it failed.
    pub(super) async fn inspect(&self, args: &[&str]) -> Option<String> {
        let out = Command::new(&self.limits.runtime)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }

    /// The arguments for a call that keeps no session.
    pub fn args(&self) -> Vec<String> {
        let mut args = vec!["run".to_owned(), "--rm".to_owned()];
        args.extend(self.seal());
        args.push(self.limits.image.clone());
        args
    }
}

/// The shell that writes standard input to path, with no interpolation of the content.
fn write_to(path: &str) -> String {
    format!("cat > {path}")
}

#[async_trait]
impl Sandbox for ContainerSandbox {
    async fn run(
        &self,
        ctx: &RequestContext,
        request: &SandboxRequest,
    ) -> Result<SandboxOutput, SandboxError> {
        if request.command.trim().is_empty() {
            return Err(SandboxError::Refused("the command is empty".into()));
        }
        let named = request.session.as_deref().map(session_name).transpose()?;
        let session = named.as_deref().map(|name| Self::container_name(ctx, name));
        let mut command = Command::new(&self.limits.runtime);
        match (&session, &named) {
            (Some(container), Some(name)) => {
                self.start_session(ctx, container, name).await?;
                self.note_used(container, name, true);
                command
                    .arg("exec")
                    .arg("--workdir")
                    .arg(WORKSPACE)
                    .arg(container);
            }
            _ => {
                command.args(self.args());
            }
        }
        let budget = self.limits.timeout.min(ctx.remaining());
        // timeout inside the container kills the command; a client timeout leaves the container.
        command
            .arg("timeout")
            .arg("-s")
            .arg("KILL")
            .arg(budget.as_secs().max(1).to_string())
            .arg("sh")
            .arg("-c")
            .arg(&request.command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let _slot = tokio::time::timeout(budget, self.slots.clone().acquire_owned())
            .await
            .map_err(|_| SandboxError::Timeout)?
            .map_err(|_| SandboxError::Runtime("the sandbox is shutting down".into()))?;
        let started = Instant::now();
        let running = self.log.started(SandboxCommand {
            // started stamps the ticket over this.
            id: 0,
            at: Utc::now(),
            container: session.clone(),
            session: named.clone(),
            command: redact_text(&request.command),
            exit_code: None,
            duration_ms: None,
        });
        let cap = self.limits.max_output_chars.saturating_mul(4).max(1024);
        let ran = tokio::time::timeout(budget, bounded_output(command, cap)).await;
        let took = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let output = match ran {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                running.ended(None, took);
                return Err(SandboxError::Runtime(format!(
                    "{} did not start: {e}",
                    self.limits.runtime
                )));
            }
            Err(_) => {
                running.ended(None, took);
                return Err(SandboxError::Timeout);
            }
        };
        running.ended(Some(output.status.code().unwrap_or(-1)), took);

        let max = self.limits.max_output_chars;
        Ok(SandboxOutput {
            // A process ended by a signal has no exit code and reports -1.
            exit_code: output.status.code().unwrap_or(-1),
            stdout: clip(&String::from_utf8_lossy(&output.stdout), max),
            stderr: clip(&String::from_utf8_lossy(&output.stderr), max),
            session: request.session.clone(),
        })
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    fn sessions(&self) -> Vec<SandboxSession> {
        let mut out: Vec<SandboxSession> = held(&self.sessions)
            .iter()
            .map(|(name, live)| SandboxSession {
                name: name.clone(),
                session: live.session.clone(),
                age_secs: live.started.elapsed().as_secs(),
                idle_secs: live.used.elapsed().as_secs(),
                runs: live.runs,
            })
            .collect();
        out.sort_by_key(|s| s.idle_secs);
        out
    }

    fn commands(&self) -> Vec<SandboxCommand> {
        self.log.commands()
    }

    async fn kill(&self, name: &str) -> Result<(), SandboxError> {
        if !held(&self.sessions).contains_key(name) {
            return Err(SandboxError::Refused(format!(
                "no session container named {name}"
            )));
        }
        self.remove(name).await;
        Ok(())
    }

    async fn put(
        &self,
        ctx: &RequestContext,
        session: &str,
        name: &str,
        content: &[u8],
    ) -> Result<String, SandboxError> {
        let session = session_name(session)?;
        let file = workspace_path(name)?;
        let container = Self::container_name(ctx, &session);
        self.start_session(ctx, &container, &session).await?;
        let path = format!("{WORKSPACE}/{file}");

        let mut child = Command::new(&self.limits.runtime)
            .args(["exec", "--interactive", "--workdir", WORKSPACE, &container])
            .args(["sh", "-c", &write_to(&path)])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                SandboxError::Runtime(format!("{} did not start: {e}", self.limits.runtime))
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(content)
                .await
                .map_err(|e| SandboxError::Runtime(format!("could not write {path}: {e}")))?;
            stdin
                .shutdown()
                .await
                .map_err(|e| SandboxError::Runtime(format!("could not close {path}: {e}")))?;
        }
        let budget = self.limits.timeout.min(ctx.remaining());
        let out = tokio::time::timeout(budget, child.wait_with_output())
            .await
            .map_err(|_| SandboxError::Timeout)?
            .map_err(|e| SandboxError::Runtime(format!("could not write {path}: {e}")))?;
        if !out.status.success() {
            return Err(SandboxError::Runtime(format!(
                "could not write {path}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(path)
    }
}
