//! What the policy allows, what the guardrail refuses, and how a sandboxed command runs.

use serde::Deserialize;

use crate::core::types::tools::RiskClass;

/// What the risk policy allows, denies, and holds for confirmation.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Policy {
    /// Roles allowed to run external_write and above. Empty denies everyone.
    pub write_roles: Vec<String>,
    /// Let tools read inside the authenticated session of the user.
    pub allow_authenticated_reads: bool,
    /// Lowest risk class that must be confirmed before it runs.
    pub confirm_from: RiskClass,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            write_roles: vec!["MANAGE_GUILD".into()],
            allow_authenticated_reads: false,
            confirm_from: RiskClass::ExternalWrite,
        }
    }
}

/// The gate every model response passes.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Guardrail {
    /// Check responses at all.
    pub enabled: bool,
    /// Phrases that block a response wherever they appear. Matched without case.
    pub denied_phrases: Vec<String>,
    /// Terms removed from an answer wherever they appear as a whole word, matched without case.
    pub protected_terms: Vec<String>,
    /// Shown in an answer in place of a protected term.
    pub redaction: String,
    /// Longest answer allowed. 0 removes the limit.
    pub max_answer_chars: usize,
    /// Shown in place of a blocked response.
    pub replacement: String,
}

impl Default for Guardrail {
    fn default() -> Self {
        Self {
            enabled: true,
            denied_phrases: Vec::new(),
            protected_terms: Vec::new(),
            redaction: "[hidden]".into(),
            max_answer_chars: 8_000,
            replacement: "I cannot answer that. Ask a moderator if you need help.".into(),
        }
    }
}

/// A command in an isolated environment.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct SandboxSettings {
    /// Offer the sandbox to the model. The engine needs access to the container runtime.
    pub enabled: bool,
    /// Refuse to start when the runtime is unreachable. Off leaves the tool unregistered instead.
    pub required: bool,
    /// Container runtime binary, or wasm to run commands in WebAssembly inside the engine.
    pub runtime: String,
    /// Image the command runs in.
    pub image: String,
    /// Memory ceiling.
    pub memory: String,
    /// CPU ceiling.
    pub cpus: String,
    /// Process ceiling.
    pub pids: u32,
    /// Wall-clock budget for one command.
    pub timeout_secs: u64,
    /// Longest stdout or stderr handed back to the model.
    pub max_output_chars: usize,
    /// Risk class the tool declares to Policy.
    pub risk: RiskClass,
    /// How long a session container stays up with nothing running in it.
    pub session_idle_secs: u64,
    /// Sessions one user may hold at once. Starting one past the cap reaps the least recently used.
    pub max_sessions: usize,
    /// Session containers across every user, capped by reaping the least recently used.
    pub max_sessions_total: usize,
    /// Commands running at once across every user. A command past it waits for a slot.
    pub max_running: usize,
    /// Names this engine's containers apart from another engine's on the same runtime.
    pub instance: String,
    /// Size of the writable workspace, in mebibytes.
    pub workspace_mb: u32,
    /// Commands kept for the operator view of what the agent ran.
    pub recent_commands: usize,
    /// Let commands reach the public internet through the egress proxy. Off runs with no network.
    pub egress: bool,
    /// Internal network the containers join when egress is on. Created at boot if missing.
    pub egress_network: String,
    /// Image of the proxy that is the only way out of egress_network.
    pub egress_proxy_image: String,
    /// Container name of that proxy, which is also its host name on egress_network.
    pub egress_proxy_name: String,
    /// What the tool tells the model it is for.
    pub description: String,
    /// What the tool tells the model the command argument is.
    pub command_description: String,
    /// What the tool tells the model the session argument is.
    pub session_description: String,
    /// Folder of the WebAssembly packages, each file named namespace__name@version.webc.
    pub wasm_packages: String,
    /// Folder the compiled packages are kept in, so a restart does not compile them again.
    pub wasm_cache: String,
    /// Folder that holds one workspace folder per student.
    pub wasm_work: String,
    /// What the tool tells the model it is for when runtime is wasm.
    pub wasm_description: String,
}

impl Default for SandboxSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            required: false,
            runtime: "docker".into(),
            image: "ghcr.io/theaisocietyasu/sparkyai-sandbox:main".into(),
            memory: "256m".into(),
            cpus: "1".into(),
            pids: 128,
            timeout_secs: 20,
            max_output_chars: 4_000,
            risk: RiskClass::PrepareWrite,
            session_idle_secs: 900,
            max_sessions: 4,
            max_sessions_total: 16,
            max_running: 8,
            instance: "default".into(),
            workspace_mb: 64,
            recent_commands: 200,
            egress: false,
            egress_network: "sparky-sandbox".into(),
            egress_proxy_image: "ghcr.io/theaisocietyasu/sparkyai-sandbox-proxy:main".into(),
            egress_proxy_name: "sparky-sandbox-proxy".into(),
            description: DESCRIPTION.into(),
            command_description: COMMAND_DESCRIPTION.into(),
            session_description: SESSION_DESCRIPTION.into(),
            wasm_packages: "/opt/sparky/wasm".into(),
            wasm_cache: "data/sandbox/cache".into(),
            wasm_work: "data/sandbox/work".into(),
            wasm_description: WASM_DESCRIPTION.into(),
        }
    }
}

/// What the sandbox tells the model it is for, when sandbox.description is unset.
pub const DESCRIPTION: &str = "Run a shell command in an isolated environment. Use it to work \
    something out rather than doing it in your head: arithmetic, dates, sorting, filtering, or \
    reading a file the user attached or an earlier tool left in the workspace. python3 with \
    pandas, pypdf, docx and openpyxl, pdftotext, tesseract, jq, rg and the usual text tools are \
    installed. There is no network, so it cannot fetch anything or reach ASU.";

/// The sandbox.runtime that runs commands in WebAssembly inside the engine.
pub const WASM_RUNTIME: &str = "wasm";

/// What the sandbox tells the model it is for when runtime is wasm.
pub const WASM_DESCRIPTION: &str = "Run a bash command in the student's own workspace, kept \
    between calls. Use it to work something out rather than doing it in your head, and to read \
    or change a file the user attached. It has python3 with the standard library, grep, sed, rg \
    and the coreutils. There is no network.";

/// What the sandbox tells the model the command argument is.
pub const COMMAND_DESCRIPTION: &str = "The shell command to run.";

/// What the sandbox tells the model the session argument is.
pub const SESSION_DESCRIPTION: &str = "Name a session to keep files in the workspace between \
    calls. Reuse the same name to resume it. Results an earlier tool wrote to the workspace name \
    the session they are in.";
