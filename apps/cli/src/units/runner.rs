//! Starts, stops, streams unit output. Hosts run via setsid; compose runs via docker compose.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::UnboundedSender;

use crate::core::types::{Event, Kind, LogLine, RunnerError, ServiceState, Stream, Unit};
use crate::units::output::{parse_ps, sanitize_line};

const COMPOSE_FILE: &str = "deploy/compose.yml";

/// Owns the children the console started.
pub struct Runner {
    root: PathBuf,
    tx: UnboundedSender<Event>,
    /// Process-group ids of host processes and tasks, by unit id.
    groups: HashMap<String, u32>,
    /// The docker compose logs -f children, by service name.
    followers: HashMap<String, Child>,
    /// Longest line kept from a unit's output.
    line_chars: usize,
    /// Variables this process took from .env rather than from the shell.
    from_dotenv: HashSet<String>,
}

impl Runner {
    /// A runner working from the repo root.
    pub fn new(root: PathBuf, tx: UnboundedSender<Event>, line_chars: usize) -> Self {
        let from_dotenv = dotenv(&root)
            .into_iter()
            .filter(|(key, value)| std::env::var(key).is_ok_and(|v| v == *value))
            .map(|(key, _)| key)
            .collect();
        Self {
            root,
            tx,
            groups: HashMap::new(),
            followers: HashMap::new(),
            line_chars,
            from_dotenv,
        }
    }

    /// Starts a unit. Services come up detached and are then followed.
    pub fn start(&mut self, unit: &Unit) -> Result<(), RunnerError> {
        match &unit.kind {
            Kind::Service { service, profile } => {
                let mut cmd = self.compose(profile.as_deref());
                cmd.args(["up", "-d", service]);
                self.spawn_streaming(&unit.id, cmd, true)?;
                self.follow(service)
            }
            Kind::Process | Kind::Task => {
                let mut cmd = Command::new("setsid");
                cmd.arg("just").args(&unit.args);
                self.spawn_streaming(&unit.id, cmd, true)
            }
            // The engine starts and stops its own containers; the console asks it to.
            Kind::Sandbox(_) => Ok(()),
        }
    }

    /// Stops a unit. Host trees get SIGTERM, services get compose stop.
    pub fn stop(&mut self, unit: &Unit) -> Result<(), RunnerError> {
        match &unit.kind {
            Kind::Service { service, profile } => {
                self.unfollow(service);
                let mut cmd = self.compose(profile.as_deref());
                cmd.args(["stop", service]);
                self.spawn_streaming(&unit.id, cmd, false)
            }
            Kind::Process | Kind::Task => {
                if let Some(pgid) = self.groups.remove(&unit.id) {
                    self.note(&unit.id, format!("stopping process group {pgid}"));
                    let mut kill = Command::new("kill");
                    kill.args(["-TERM", "--", &format!("-{pgid}")]);
                    kill.stdout(Stdio::null()).stderr(Stdio::null());
                    kill.spawn().map_err(|source| RunnerError::Spawn {
                        cmd: format!("kill -TERM -- -{pgid}"),
                        source,
                    })?;
                }
                Ok(())
            }
            Kind::Sandbox(_) => Ok(()),
        }
    }

    /// Whether a host process or task started here is still tracked.
    pub fn owns(&self, unit_id: &str) -> bool {
        self.groups.contains_key(unit_id)
    }

    /// Drops the record of a process group after its child exited.
    pub fn forget(&mut self, unit_id: &str) {
        self.groups.remove(unit_id);
    }

    /// Begins streaming the container logs of a service, if not already.
    pub fn follow(&mut self, service: &str) -> Result<(), RunnerError> {
        if self.followers.contains_key(service) {
            return Ok(());
        }
        let mut cmd = self.compose_all_profiles();
        cmd.args(["logs", "-f", "--tail", "200", "--no-color", service]);
        let child = self.spawn_piped(service, cmd, false)?;
        self.followers.insert(service.to_owned(), child);
        Ok(())
    }

    /// Stops following the logs of a service.
    pub fn unfollow(&mut self, service: &str) {
        if let Some(mut child) = self.followers.remove(service)
            && let Err(e) = child.start_kill()
        {
            self.note(service, format!("stop following logs: {e}"));
        }
    }

    /// Kills every host process and follower. Compose services are left running.
    pub fn shutdown(&mut self) {
        let services: Vec<String> = self.followers.keys().cloned().collect();
        for s in services {
            self.unfollow(&s);
        }
        for pgid in self.groups.values() {
            // The terminal is being restored; a failed kill has no pane to report to.
            let _ = std::process::Command::new("kill")
                .args(["-TERM", "--", &format!("-{pgid}")])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        self.groups.clear();
    }

    /// One docker compose ps -a --format json snapshot, keyed by service.
    pub async fn service_states(root: &Path) -> Result<HashMap<String, ServiceState>, String> {
        let out = Command::new("docker")
            .args(["compose", "-f", COMPOSE_FILE])
            .args(all_profiles())
            .args(["ps", "-a", "--format", "json"])
            .current_dir(root)
            .output()
            .await
            .map_err(|e| format!("docker compose ps: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "docker compose ps: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        parse_ps(&String::from_utf8_lossy(&out.stdout))
    }

    fn compose(&self, profile: Option<&str>) -> Command {
        let mut cmd = Command::new("docker");
        cmd.args(["compose", "-f", COMPOSE_FILE]);
        if let Some(p) = profile {
            cmd.args(["--profile", p]);
        }
        cmd.current_dir(&self.root);
        cmd
    }

    fn compose_all_profiles(&self) -> Command {
        let mut cmd = Command::new("docker");
        cmd.args(["compose", "-f", COMPOSE_FILE]);
        cmd.args(all_profiles());
        cmd.current_dir(&self.root);
        cmd
    }

    /// Spawns a process, streams stdout and stderr as log lines, and reports the exit.
    fn spawn_streaming(
        &mut self,
        unit_id: &str,
        cmd: Command,
        track: bool,
    ) -> Result<(), RunnerError> {
        let mut child = self.spawn_piped(unit_id, cmd, true)?;
        if track && let Some(pid) = child.id() {
            self.groups.insert(unit_id.to_owned(), pid);
        }
        let tx = self.tx.clone();
        let id = unit_id.to_owned();
        tokio::spawn(async move {
            // A send error means the UI has exited.
            let code = match child.wait().await {
                Ok(status) => status.code(),
                Err(e) => {
                    let _ = tx.send(Event::Log {
                        unit: id.clone(),
                        line: LogLine::now(Stream::Meta, format!("wait failed: {e}")),
                    });
                    None
                }
            };
            let _ = tx.send(Event::Exited { unit: id, code });
        });
        Ok(())
    }

    fn spawn_piped(
        &self,
        unit_id: &str,
        mut cmd: Command,
        announce: bool,
    ) -> Result<Child, RunnerError> {
        let line = describe(cmd.as_std());
        cmd.current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(false)
            .env("CARGO_TERM_COLOR", "never")
            .env("NO_COLOR", "1");
        // Each child sees .env as it is now; a variable the shell set still wins.
        for key in &self.from_dotenv {
            cmd.env_remove(key);
        }
        for (key, value) in dotenv(&self.root) {
            if self.from_dotenv.contains(&key) || std::env::var_os(&key).is_none() {
                cmd.env(key, value);
            }
        }
        let mut child = cmd.spawn().map_err(|source| RunnerError::Spawn {
            cmd: line.clone(),
            source,
        })?;
        if announce {
            self.note(unit_id, format!("$ {line}"));
        }
        if let Some(out) = child.stdout.take() {
            pump(
                self.tx.clone(),
                unit_id.to_owned(),
                Stream::Out,
                out,
                self.line_chars,
            );
        }
        if let Some(err) = child.stderr.take() {
            pump(
                self.tx.clone(),
                unit_id.to_owned(),
                Stream::Err,
                err,
                self.line_chars,
            );
        }
        Ok(child)
    }

    fn note(&self, unit_id: &str, text: String) {
        // A send error means the UI has exited.
        let _ = self.tx.send(Event::Log {
            unit: unit_id.to_owned(),
            line: LogLine::now(Stream::Meta, text),
        });
    }
}

/// The pairs .env holds at this moment, or none when it is missing or unreadable.
fn dotenv(root: &Path) -> Vec<(String, String)> {
    dotenvy::from_path_iter(root.join(".env"))
        .map_or_default(|pairs| pairs.filter_map(Result::ok).collect())
}

fn all_profiles() -> [&'static str; 12] {
    [
        "--profile",
        "phoenix",
        "--profile",
        "model",
        "--profile",
        "crawl",
        "--profile",
        "db",
        "--profile",
        "metrics",
        "--profile",
        "gpu-metrics",
    ]
}

fn describe(cmd: &std::process::Command) -> String {
    let mut parts = vec![cmd.get_program().to_string_lossy().into_owned()];
    parts.extend(cmd.get_args().map(|a| a.to_string_lossy().into_owned()));
    parts.join(" ")
}

fn pump<R>(tx: UnboundedSender<Event>, unit: String, stream: Stream, reader: R, line_chars: usize)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            let text = match lines.next_line().await {
                Ok(Some(text)) => text,
                Ok(None) => break,
                Err(e) => {
                    // A send error means the UI has exited.
                    let _ = tx.send(Event::Log {
                        unit: unit.clone(),
                        line: LogLine::now(Stream::Meta, format!("log capture ended: {e}")),
                    });
                    break;
                }
            };
            let mut text = sanitize_line(&text);
            if line_chars > 0 && text.chars().count() > line_chars {
                text = text.chars().take(line_chars).collect::<String>() + " [line cut]";
            }
            if tx
                .send(Event::Log {
                    unit: unit.clone(),
                    line: LogLine::now(stream, text),
                })
                .is_err()
            {
                break;
            }
        }
    });
}
