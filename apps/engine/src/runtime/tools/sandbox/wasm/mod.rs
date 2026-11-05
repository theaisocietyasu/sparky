//! WasmSandbox: runs commands as WebAssembly in a child of the engine, one kept folder per student.

pub mod runner;

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use tokio::process::Command;

use crate::core::config::SandboxSettings;
use crate::core::traits::tools::sandbox::Sandbox;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::safety::redact::redact_text;
use crate::core::types::tools::sandbox::{
    SandboxCommand, SandboxError, SandboxOutput, SandboxRequest, SandboxSession, session_name,
    workspace_path,
};
use crate::runtime::tools::sandbox::log::{CommandLog, held};
use crate::runtime::tools::sandbox::output::{Bounded, bounded_input, clip};
use crate::runtime::tools::sandbox::session::Live;

use self::runner::{GUEST_WORK, SETUP_FAILED, SUBCOMMAND};

/// Where the packages, compiled modules and student folders live.
#[derive(Debug, Clone)]
pub struct Folders {
    /// The packages, each file named namespace__name@version.webc.
    pub packages: PathBuf,
    /// Compiled modules, kept across restarts.
    pub cache: PathBuf,
    /// One workspace folder per student.
    pub work: PathBuf,
}

/// The limits a wasm run is held to.
#[derive(Debug, Clone)]
pub struct WasmLimits {
    /// Where things live.
    pub folders: Folders,
    /// Wall-clock budget of one command.
    pub timeout: Duration,
    /// Longest stdout or stderr handed back.
    pub max_output_chars: usize,
    /// Commands running at once across every student.
    pub max_running: usize,
    /// Size a student folder may grow to, in mebibytes.
    pub workspace_mb: u32,
    /// Commands kept for the operator view.
    pub recent_commands: usize,
}

impl From<&SandboxSettings> for WasmLimits {
    fn from(cfg: &SandboxSettings) -> Self {
        Self {
            folders: Folders {
                packages: PathBuf::from(&cfg.wasm_packages),
                cache: PathBuf::from(&cfg.wasm_cache),
                work: PathBuf::from(&cfg.wasm_work),
            },
            timeout: Duration::from_secs(cfg.timeout_secs.max(1)),
            max_output_chars: cfg.max_output_chars,
            max_running: cfg.max_running,
            workspace_mb: cfg.workspace_mb,
            recent_commands: cfg.recent_commands,
        }
    }
}

/// Runs bash and the tools beside it as WebAssembly. Clones share the log and the folder list.
#[derive(Debug, Clone)]
pub struct WasmSandbox {
    limits: WasmLimits,
    /// The program the child runs as, which is this engine.
    program: PathBuf,
    /// Student folder name to what is known about it.
    folders: Arc<Mutex<HashMap<String, Live>>>,
    log: CommandLog,
    enabled: Arc<AtomicBool>,
    slots: Arc<tokio::sync::Semaphore>,
}

impl WasmSandbox {
    /// Builds the sandbox; program is the engine binary the child runs as.
    pub fn new(limits: WasmLimits, program: PathBuf) -> Self {
        Self {
            slots: Arc::new(tokio::sync::Semaphore::new(limits.max_running.max(1))),
            log: CommandLog::new(limits.recent_commands),
            limits,
            program,
            folders: Arc::default(),
            enabled: Arc::new(AtomicBool::new(true)),
        }
    }

    /// The folder name of one student, scoped to the tenant and user.
    pub fn folder_name(ctx: &RequestContext) -> String {
        let mut hasher = DefaultHasher::new();
        (&ctx.tenant_id, &ctx.user_id).hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// The folder of one student, made when missing.
    async fn folder(&self, ctx: &RequestContext) -> Result<(String, PathBuf), SandboxError> {
        let name = Self::folder_name(ctx);
        let path = self.limits.folders.work.join(&name);
        tokio::fs::create_dir_all(&path)
            .await
            .map_err(|e| SandboxError::Runtime(format!("could not make the workspace: {e}")))?;
        Ok((name, path))
    }

    /// Refuses a folder past the size cap.
    async fn within_cap(&self, path: &Path) -> Result<(), SandboxError> {
        let path = path.to_path_buf();
        let used = tokio::task::spawn_blocking(move || size_of(&path))
            .await
            .unwrap_or(0);
        let cap = u64::from(self.limits.workspace_mb) * 1024 * 1024;
        if used > cap {
            return Err(SandboxError::Refused(format!(
                "the workspace holds {} MB, past its {} MB; delete files in {GUEST_WORK} first",
                used / (1024 * 1024),
                self.limits.workspace_mb
            )));
        }
        Ok(())
    }

    /// Whether the packages are in place. Then compiles them in the background for the first run.
    pub async fn probe(&self) -> Result<(), SandboxError> {
        let folders = &self.limits.folders;
        let mut entries = tokio::fs::read_dir(&folders.packages)
            .await
            .map_err(|e| SandboxError::Runtime(format!("{}: {e}", folders.packages.display())))?;
        let mut shell = false;
        while let Ok(Some(entry)) = entries.next_entry().await {
            shell |= runner::package_id(&entry.path()).is_some_and(|(n, _)| n == "wasmer/bash");
        }
        if !shell {
            return Err(SandboxError::Runtime(format!(
                "no wasmer/bash package in {}",
                folders.packages.display()
            )));
        }
        for dir in [&folders.cache, &folders.work] {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| SandboxError::Runtime(format!("{}: {e}", dir.display())))?;
        }
        let warm = self.clone();
        tokio::spawn(async move { warm.warm().await });
        Ok(())
    }

    /// Runs python once in a scratch folder so the first student does not wait for compilation.
    async fn warm(&self) {
        let scratch = self.limits.folders.work.join(".warm");
        if tokio::fs::create_dir_all(&scratch).await.is_err() {
            return;
        }
        let started = Instant::now();
        let ran = self
            .child(
                &scratch,
                "python3 -c 0 && grep -q . /dev/null; true",
                Duration::from_secs(600),
            )
            .await;
        let _ = tokio::fs::remove_dir_all(&scratch).await;
        match ran {
            Ok(_) => tracing::info!(secs = started.elapsed().as_secs(), "wasm sandbox compiled"),
            Err(error) => tracing::warn!(%error, "wasm sandbox did not compile"),
        }
    }

    /// Runs command in the child over folder, killed when budget runs out.
    async fn child(
        &self,
        folder: &Path,
        command: &str,
        budget: Duration,
    ) -> Result<Bounded, SandboxError> {
        let cap = self.limits.max_output_chars.saturating_mul(4).max(1024);
        let folders = &self.limits.folders;
        let mut child = Command::new(&self.program);
        child
            .arg(SUBCOMMAND)
            .arg(&folders.packages)
            .arg(&folders.cache)
            .arg(folder)
            .arg(cap.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let input = Some(command.as_bytes().to_vec());
        let ran = tokio::time::timeout(budget, bounded_input(child, cap, input))
            .await
            .map_err(|_| SandboxError::Timeout)?
            .map_err(|e| SandboxError::Runtime(format!("the sandbox did not start: {e}")))?;
        if ran.status.code() == Some(SETUP_FAILED) {
            return Err(SandboxError::Runtime(clip(
                String::from_utf8_lossy(&ran.stderr).trim(),
                400,
            )));
        }
        Ok(ran)
    }
}

/// Bytes held under path, counting files only.
fn size_of(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => size_of(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

#[async_trait]
impl Sandbox for WasmSandbox {
    async fn run(
        &self,
        ctx: &RequestContext,
        request: &SandboxRequest,
    ) -> Result<SandboxOutput, SandboxError> {
        if request.command.trim().is_empty() {
            return Err(SandboxError::Refused("the command is empty".into()));
        }
        let named = request.session.as_deref().map(session_name).transpose()?;
        let (name, folder) = self.folder(ctx).await?;
        self.within_cap(&folder).await?;
        let budget = self.limits.timeout.min(ctx.remaining());
        let _slot = tokio::time::timeout(budget, self.slots.clone().acquire_owned())
            .await
            .map_err(|_| SandboxError::Timeout)?
            .map_err(|_| SandboxError::Runtime("the sandbox is shutting down".into()))?;
        {
            let mut folders = held(&self.folders);
            let live = folders
                .entry(name.clone())
                .or_insert_with(|| Live::new(GUEST_WORK));
            live.used = Instant::now();
            live.runs += 1;
        }
        let started = Instant::now();
        let running = self.log.started(SandboxCommand {
            id: 0,
            at: Utc::now(),
            container: Some(name),
            session: named,
            command: redact_text(&request.command),
            exit_code: None,
            duration_ms: None,
        });
        let ran = self.child(&folder, &request.command, budget).await;
        let took = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let output = match ran {
            Ok(output) => output,
            Err(error) => {
                running.ended(None, took);
                return Err(error);
            }
        };
        let exit_code = output.status.code().unwrap_or(-1);
        running.ended(Some(exit_code), took);
        let max = self.limits.max_output_chars;
        Ok(SandboxOutput {
            exit_code,
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

    fn converts(&self) -> bool {
        false
    }

    fn sessions(&self) -> Vec<SandboxSession> {
        let mut out: Vec<SandboxSession> = held(&self.folders)
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
        if held(&self.folders).remove(name).is_none() {
            return Err(SandboxError::Refused(format!("no workspace named {name}")));
        }
        let path = self.limits.folders.work.join(name);
        tokio::fs::remove_dir_all(&path)
            .await
            .or_else(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(SandboxError::Runtime(format!(
                    "could not delete the workspace: {e}"
                ))),
            })
    }

    async fn put(
        &self,
        ctx: &RequestContext,
        session: &str,
        name: &str,
        content: &[u8],
    ) -> Result<String, SandboxError> {
        session_name(session)?;
        let file = workspace_path(name)?;
        let (_, folder) = self.folder(ctx).await?;
        self.within_cap(&folder).await?;
        tokio::fs::write(folder.join(&file), content)
            .await
            .map_err(|e| SandboxError::Runtime(format!("could not write {file}: {e}")))?;
        Ok(format!("{GUEST_WORK}/{file}"))
    }
}
