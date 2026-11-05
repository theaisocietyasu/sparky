//! Configuration. Every external service and harness knob is configured here, nowhere else.

pub mod admin;
pub mod harness;
pub mod http;
pub mod integrations;
pub mod services;
pub mod store;
pub mod web;

use figment::Figment;
use figment::providers::{Env, Format, Toml};
use secrecy::ExposeSecret;
use serde::Deserialize;

use crate::core::types::agent::AgentConfig;
use crate::core::types::agent::assemble::Budget;

pub use self::admin::*;
pub use self::harness::*;
pub use self::http::*;
pub use self::integrations::*;
pub use self::services::*;
pub use self::store::*;
pub use self::web::*;

/// TOML layer read when SPARKY_CONFIG_FILE is unset. Missing is not an error.
pub const DEFAULT_CONFIG_FILE: &str = "sparky.toml";

/// Root configuration, loaded from a TOML file and SPARKY_* environment variables.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Process-level settings.
    pub app: App,
    /// What clients must present to call the engine.
    pub engine: Engine,
    /// Discord bot settings.
    pub discord: Discord,
    /// Chat model endpoint.
    pub model: Model,
    /// Embedding endpoint, used to embed queries at retrieval time.
    pub embedding: Embedding,
    /// OpenTelemetry export.
    #[serde(default)]
    pub telemetry: Telemetry,
    /// Loop limits and prompt budgets.
    #[serde(default)]
    pub agent: Agent,
    /// The text the harness writes around every prompt.
    #[serde(default)]
    pub prompt: Prompt,
    /// What the risk policy allows, denies, and holds for confirmation.
    #[serde(default)]
    pub policy: Policy,
    /// Hybrid retrieval tuning.
    #[serde(default)]
    pub retrieval: Retrieval,
    /// Which tools are registered.
    #[serde(default)]
    pub tools: Tools,
    /// How a live source query runs.
    #[serde(default)]
    pub query: Query,
    /// How history that no longer fits is compacted.
    #[serde(default)]
    pub compaction: Compaction,
    /// What the guardrail refuses.
    #[serde(default)]
    pub guardrail: Guardrail,
    /// How a sandboxed command runs.
    #[serde(default)]
    pub sandbox: SandboxSettings,
    /// JSONL trace recording.
    #[serde(default)]
    pub trace: Trace,
    /// HTTP surface limits.
    #[serde(default)]
    pub http: Http,
    /// MCP servers exposed as tools.
    #[serde(default)]
    pub mcp: Mcp,
    /// Outlook (Microsoft Graph) integration. Off by default.
    #[serde(default)]
    pub outlook: Outlook,
    /// Google Calendar integration. Off by default.
    #[serde(default)]
    pub gcal: Gcal,
    /// Canvas calendar feed integration.
    #[serde(default)]
    pub canvas: Canvas,
    /// The upcoming tool over tasks, Canvas and calendars.
    #[serde(default)]
    pub upcoming: Upcoming,
    /// Reminders and notifications.
    #[serde(default)]
    pub reminders: Reminders,
    /// Academic paper search.
    #[serde(default)]
    pub papers: Papers,
    /// Arithmetic and date math.
    #[serde(default)]
    pub calculate: Calculate,
    /// Searching earlier private chats.
    #[serde(default)]
    pub chat_search: ChatSearch,
    /// Notifications Sparky sends on its own.
    #[serde(default)]
    pub nudges: Nudges,
    /// Reading one public web page.
    #[serde(default)]
    pub read_page: ReadPage,
    /// Wikipedia lookups.
    #[serde(default)]
    pub wikipedia: Wikipedia,
    /// The platform that holds the knowledge base, the live queries, and the linked accounts.
    #[serde(default)]
    pub platform: Platform,
    /// Where conversations, memories and pending actions live.
    #[serde(default)]
    pub store: Store,
    /// What Sparky remembers and the key that seals sensitive memories.
    #[serde(default)]
    pub memory: MemorySettings,
    /// Discord sign-in for the web page.
    #[serde(default)]
    pub web: Web,
    /// The admin surface.
    #[serde(default)]
    pub admin: Admin,
}

/// Default budgets. Every field comes from the agent section.
impl Default for Budget {
    fn default() -> Self {
        Agent::default().budget()
    }
}

/// Default loop settings. Every field comes from the agent section.
impl Default for AgentConfig {
    fn default() -> Self {
        let agent = Agent::default();
        AgentConfig {
            provider_name: Telemetry::default().provider_name.into(),
            model_name: std::sync::Arc::from(""),
            max_steps: agent.max_steps,
            max_model_retries: agent.max_model_retries,
            tool_timeout: std::time::Duration::from_secs(agent.tool_timeout_secs),
            answer_reserve: std::time::Duration::from_secs(agent.answer_reserve_secs),
            max_tool_calls: agent.max_tool_calls,
            tool_result_to_file_chars: agent.tool_result_to_file_chars,
            confirmation_ttl: std::time::Duration::from_secs(agent.confirmation_ttl_secs),
            temperature: agent.temperature,
            upload_preview_chars: agent.upload_preview_chars,
            upload_match_chars: agent.upload_match_chars,
            upload_ocr_pages: agent.upload_ocr_pages,
            history_turns: agent.history_turns,
            history_keep: Compaction::default().keep_tokens(agent.history_budget_tokens),
            retry_base_ms: agent.retry_base_ms,
            retry_cap_ms: agent.retry_cap_ms,
            max_span_value_chars: agent.max_span_value_chars,
            budget: agent.budget(),
            stream: agent.stream,
            stream_block_chars: agent.stream_block_chars,
            thinking: agent.thinking,
            // The model section has no defaults; these are fixed values.
            max_tokens: 1024,
            max_tokens_without_thinking: 1024,
            usd_per_m_prompt: 0.0,
            usd_per_m_completion: 0.0,
        }
    }
}

/// Why configuration was rejected.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A layer could not be read or a field could not be parsed.
    #[error("config: {0}")]
    Load(String),
    /// The values parsed but cannot work together.
    #[error("config: {0}")]
    Invalid(String),
}

/// Settings that moved to another section. A variable named here fails boot.
const RENAMED: [(&str, &str); 4] = [
    ("SPARKY_AGENT__TRACE_DIR", "SPARKY_TRACE__DIR"),
    ("SPARKY_AGENT__RETRIEVAL_TOP_K", "SPARKY_RETRIEVAL__TOP_K"),
    ("SPARKY_MODEL__THINKING", "SPARKY_AGENT__THINKING__MODE"),
    ("SPARKY_TOOLS__QUERY_SOURCE", "SPARKY_TOOLS__SEARCH"),
];

impl Config {
    /// Fails on a variable that has moved, naming what replaced it.
    pub fn reject_renamed(is_set: impl Fn(&str) -> bool) -> Result<(), ConfigError> {
        match RENAMED.into_iter().find(|(old, _)| is_set(old)) {
            Some((old, new)) => Err(ConfigError::Invalid(format!(
                "{old} moved to {new}; set that instead"
            ))),
            None => Ok(()),
        }
    }

    /// Loads the TOML layer then SPARKY_* variables, __ separating nesting: SPARKY_PLATFORM__URL.
    pub fn load() -> Result<Self, ConfigError> {
        Self::reject_renamed(|key| std::env::var_os(key).is_some())?;
        let path =
            std::env::var("SPARKY_CONFIG_FILE").unwrap_or_else(|_| DEFAULT_CONFIG_FILE.to_owned());
        let cfg: Self = Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("SPARKY_").split("__"))
            .extract()
            .map_err(|e| ConfigError::Load(e.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Rejects combinations that would leave the engine unable to answer.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let invalid = |m: String| Err(ConfigError::Invalid(m));
        if !(-14..=14).contains(&self.prompt.utc_offset_hours) {
            return invalid(format!(
                "prompt.utc_offset_hours must be between -14 and 14, got {}",
                self.prompt.utc_offset_hours
            ));
        }
        validate_agent(&self.agent)?;
        validate_thinking(&self.agent.thinking)?;
        validate_telemetry(&self.telemetry)?;
        validate_sandbox(&self.sandbox)?;
        validate_model(&self.model)?;
        if self.compaction.enabled && self.compaction.max_tokens == 0 {
            return invalid("compaction.max_tokens must be at least 1".into());
        }
        validate_compaction(&self.compaction, self.agent.history_budget_tokens)?;
        validate_tools(&self.tools)?;
        validate_mcp_risks(&self.mcp)?;
        validate_integrations(self)?;
        validate_platform(self)?;
        validate_store(&self.store, &self.memory)?;
        validate_web(self)?;
        if self.admin.max_instructions_chars == 0
            || self.admin.history_limit == 0
            || self.admin.traces_limit == 0
        {
            return invalid(
                "admin.max_instructions_chars, admin.history_limit and admin.traces_limit must be above 0".into(),
            );
        }
        if self.retrieval.window < 0 {
            return invalid(format!(
                "retrieval.window must not be negative, got {}",
                self.retrieval.window
            ));
        }
        if self.agent.retry_cap_ms < self.agent.retry_base_ms {
            return invalid("agent.retry_cap_ms is below agent.retry_base_ms".into());
        }
        let mut names = std::collections::BTreeSet::new();
        for server in &self.mcp.servers {
            if !names.insert(server.name.as_str()) {
                return invalid(format!(
                    "mcp.servers has two servers named {:?}",
                    server.name
                ));
            }
        }
        // Each section budget fits within the total.
        for (name, value) in [
            ("history", self.agent.history_budget_tokens),
            ("memory", self.agent.memory_budget_tokens),
            ("capabilities", self.agent.capabilities_budget_tokens),
        ] {
            if value > self.agent.prompt_budget_tokens {
                return invalid(format!(
                    "agent.{name}_budget_tokens ({value}) exceeds agent.prompt_budget_tokens ({})",
                    self.agent.prompt_budget_tokens
                ));
            }
        }
        Ok(())
    }

    /// System instructions: prompt.system_file, else prompt.system, else fallback.
    pub fn system_prompt(&self, fallback: &str) -> Result<String, ConfigError> {
        if let Some(path) = self
            .prompt
            .system_file
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        {
            return std::fs::read_to_string(path)
                .map(|s| s.trim().to_owned())
                .map_err(|e| ConfigError::Invalid(format!("prompt.system_file {path}: {e}")));
        }
        Ok(self
            .prompt
            .system
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback)
            .to_owned())
    }
}

/// Largest top_k the platform search accepts.
const PLATFORM_MAX_TOP_K: usize = 50;
/// Largest neighbor window the platform search accepts.
const PLATFORM_MAX_WINDOW: i32 = 5;
/// Longest query text the platform search accepts.
const PLATFORM_MAX_QUERY_CHARS: usize = 1000;
/// Longest hold the platform keeps a pending action, in seconds.
const PLATFORM_MAX_TTL_SECS: u64 = 86_400;

/// Rejects a platform section the engine cannot reach, or settings the platform does not accept.
fn validate_platform(cfg: &Config) -> Result<(), ConfigError> {
    let invalid = |m: String| Err(ConfigError::Invalid(m));
    let platform = &cfg.platform;
    let url = platform.url.as_deref().map(str::trim).unwrap_or_default();
    if url::Url::parse(url).is_err() || !(url.starts_with("https://") || url.starts_with("http://"))
    {
        return invalid("set SPARKY_PLATFORM__URL, an http or https URL".into());
    }
    if platform
        .token
        .as_ref()
        .is_none_or(|t| t.expose_secret().trim().is_empty())
    {
        return invalid("set SPARKY_PLATFORM__TOKEN, the platform machine token".into());
    }
    if platform.timeout_secs == 0 {
        return invalid("platform.timeout_secs must be at least 1".into());
    }
    if let Some(mcp) = platform.mcp_endpoint()
        && (url::Url::parse(mcp).is_err()
            || !(mcp.starts_with("https://") || mcp.starts_with("http://")))
    {
        return invalid("platform.mcp_url must be an http or https URL".into());
    }
    if !(1..=PLATFORM_MAX_QUERY_CHARS).contains(&platform.max_query_chars) {
        return invalid(format!(
            "platform.max_query_chars must be from 1 to {PLATFORM_MAX_QUERY_CHARS}"
        ));
    }
    if !(1..=PLATFORM_MAX_TOP_K).contains(&cfg.retrieval.top_k) {
        return invalid(format!(
            "retrieval.top_k must be from 1 to {PLATFORM_MAX_TOP_K} on the platform"
        ));
    }
    if cfg.retrieval.window > PLATFORM_MAX_WINDOW {
        return invalid(format!(
            "retrieval.window must be at most {PLATFORM_MAX_WINDOW} on the platform"
        ));
    }
    if !(1..=PLATFORM_MAX_TTL_SECS).contains(&cfg.agent.confirmation_ttl_secs) {
        return invalid(format!(
            "agent.confirmation_ttl_secs must be from 1 to {PLATFORM_MAX_TTL_SECS} on the platform"
        ));
    }
    if platform.mcp_private.iter().any(|p| p.trim().is_empty()) {
        return invalid(
            "platform.mcp_private holds an empty prefix, which would match every tool".into(),
        );
    }
    if platform.mcp_skip.iter().any(|p| p.trim().is_empty()) {
        return invalid(
            "platform.mcp_skip holds an empty prefix, which would match every tool".into(),
        );
    }
    Ok(())
}

/// Rejects search wording the model would read as an empty string, and a missing fallback source.
fn validate_compaction(compaction: &Compaction, history: usize) -> Result<(), ConfigError> {
    if !compaction.enabled {
        return Ok(());
    }
    if !(compaction.keep_share > 0.0 && compaction.keep_share < 1.0) {
        return Err(ConfigError::Invalid(
            "compaction.keep_share must be above 0 and below 1".into(),
        ));
    }
    let needed = compaction.keep_tokens(history) + compaction.max_tokens as usize;
    if needed > history {
        return Err(ConfigError::Invalid(format!(
            "a compaction keeps {} tokens of turns and writes a summary of up to {} \
             (compaction.keep_share and compaction.max_tokens), {needed} in all, over \
             agent.history_budget_tokens = {history}; the summary would be trimmed from the prompt",
            compaction.keep_tokens(history),
            compaction.max_tokens
        )));
    }
    Ok(())
}

fn validate_sandbox(sandbox: &SandboxSettings) -> Result<(), ConfigError> {
    if !sandbox.enabled {
        return Ok(());
    }
    if sandbox.runtime.trim().is_empty() {
        return Err(ConfigError::Invalid("sandbox.runtime is empty".into()));
    }
    if sandbox.recent_commands == 0 {
        return Err(ConfigError::Invalid(
            "sandbox.recent_commands is 0, which keeps no record of what the agent ran".into(),
        ));
    }
    if sandbox.max_sessions == 0 {
        return Err(ConfigError::Invalid(
            "sandbox.max_sessions is 0, which lets no caller hold a session".into(),
        ));
    }
    if sandbox.egress {
        for (key, value) in [
            ("sandbox.egress_network", &sandbox.egress_network),
            ("sandbox.egress_proxy_image", &sandbox.egress_proxy_image),
            ("sandbox.egress_proxy_name", &sandbox.egress_proxy_name),
        ] {
            if value.trim().is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "{key} is empty and sandbox.egress is on"
                )));
            }
        }
    }
    Ok(())
}

/// A pinned risk must name a tool the server entry exposes.
fn validate_mcp_risks(mcp: &Mcp) -> Result<(), ConfigError> {
    for server in &mcp.servers {
        if server.tools.is_empty() {
            continue;
        }
        if let Some(name) = server.risks.keys().find(|k| !server.tools.contains(k)) {
            return Err(ConfigError::Invalid(format!(
                "mcp server {} pins a risk for {name}, which is not in its tools",
                server.name
            )));
        }
    }
    Ok(())
}

fn validate_integrations(cfg: &Config) -> Result<(), ConfigError> {
    validate_canvas(cfg)?;
    validate_upcoming(&cfg.upcoming)?;
    validate_reminders(&cfg.reminders)?;
    validate_nudges(&cfg.nudges)?;
    if cfg.outlook.enabled {
        if !cfg.outlook.base_url.starts_with("https://") {
            return Err(ConfigError::Invalid(
                "outlook: base_url must be https".into(),
            ));
        }
        if cfg.outlook.timeout_secs == 0 || cfg.outlook.max_items == 0 {
            return Err(ConfigError::Invalid(
                "outlook: timeout_secs and max_items must be at least 1".into(),
            ));
        }
    }
    Ok(())
}

/// Rejects nudge settings that could never send or would send at an hour that does not exist.
fn validate_nudges(n: &Nudges) -> Result<(), ConfigError> {
    if !n.enabled {
        return Ok(());
    }
    let invalid = |m: &str| Err(ConfigError::Invalid(format!("nudges: {m}")));
    if n.tick_secs == 0 || n.active_days == 0 || n.ask_timeout_secs == 0 || n.keep_days == 0 {
        return invalid(
            "tick_secs, active_days, ask_timeout_secs and keep_days must be at least 1",
        );
    }
    let hours = [
        u32::from(n.quiet_start),
        u32::from(n.quiet_end),
        n.brief_hour,
        n.week_plan_hour,
        n.event_hour,
        n.study_hour,
        n.calendar_hour,
    ];
    if hours.iter().any(|h| *h > 23) {
        return invalid("every hour must be 0 to 23");
    }
    if !(1..=7).contains(&n.calendar_weekday) {
        return invalid("calendar_weekday must be 1 for Monday to 7 for Sunday");
    }
    if n.brief_days < 1 || n.due_soon_hours < 1 || n.start_early_days < 1 || n.new_item_days < 1 {
        return invalid(
            "brief_days, due_soon_hours, start_early_days and new_item_days must be at least 1",
        );
    }
    if n.follow_up_days < 1 || n.follow_up_max_days <= n.follow_up_days {
        return invalid(
            "follow_up_max_days must be more than follow_up_days, which must be at least 1",
        );
    }
    if n.nothing.trim().is_empty() {
        return invalid("nothing is empty");
    }
    Ok(())
}

fn validate_web(cfg: &Config) -> Result<(), ConfigError> {
    let web = &cfg.web;
    if !web.enabled {
        return Ok(());
    }
    let invalid = |m: &str| Err(ConfigError::Invalid(format!("web: {m}")));
    if web
        .discord_client_id
        .as_deref()
        .is_none_or(|id| id.trim().is_empty())
        || web
            .discord_client_secret
            .as_ref()
            .is_none_or(|s| s.expose_secret().trim().is_empty())
    {
        return invalid("set SPARKY_WEB__DISCORD_CLIENT_ID and SPARKY_WEB__DISCORD_CLIENT_SECRET");
    }
    web.key()?;
    let https = |u: &str| url::Url::parse(u).is_ok() && u.starts_with("https://");
    if !https(web.public()) {
        return invalid("set SPARKY_WEB__PUBLIC_URL, the https URL of this engine");
    }
    if !https(web.site()) {
        return invalid("set SPARKY_WEB__SITE_URL, the https origin of the page");
    }
    if !cfg
        .http
        .cors_origins
        .iter()
        .any(|o| o.trim_end_matches('/') == web.site())
    {
        return invalid("http.cors_origins must list SPARKY_WEB__SITE_URL");
    }
    if cfg.http.cors_origins.iter().any(|o| o == "*") {
        return invalid("http.cors_origins must not be * while the web sign-in is on");
    }
    if web.session_hours == 0 || web.timeout_secs == 0 || web.cookie_name.trim().is_empty() {
        return invalid("session_hours, timeout_secs and cookie_name must be set");
    }
    Ok(())
}

fn validate_reminders(reminders: &Reminders) -> Result<(), ConfigError> {
    if !reminders.enabled {
        return Ok(());
    }
    if reminders.tick_secs == 0
        || reminders.max_per_user == 0
        || reminders.max_chars == 0
        || reminders.max_days_ahead == 0
        || reminders.keep_days == 0
        || reminders.list_limit == 0
    {
        return Err(ConfigError::Invalid(
            "reminders: every limit must be at least 1".into(),
        ));
    }
    Ok(())
}

fn validate_upcoming(upcoming: &Upcoming) -> Result<(), ConfigError> {
    if !upcoming.enabled {
        return Ok(());
    }
    if upcoming.default_days == 0 || upcoming.default_days > upcoming.max_days {
        return Err(ConfigError::Invalid(
            "upcoming: default_days must be from 1 to max_days".into(),
        ));
    }
    if upcoming.max_items == 0 {
        return Err(ConfigError::Invalid(
            "upcoming: max_items must be at least 1".into(),
        ));
    }
    Ok(())
}

fn validate_canvas(cfg: &Config) -> Result<(), ConfigError> {
    let canvas = &cfg.canvas;
    if !canvas.enabled {
        return Ok(());
    }
    let invalid = |m: &str| Err(ConfigError::Invalid(format!("canvas: {m}")));
    if canvas.hosts.is_empty() || canvas.hosts.iter().any(|h| h.trim().is_empty()) {
        return invalid("hosts must name at least one host and no empty one");
    }
    if canvas.timeout_secs == 0 || canvas.max_bytes == 0 || canvas.max_items == 0 {
        return invalid("timeout_secs, max_bytes and max_items must be at least 1");
    }
    if canvas.default_days == 0 || canvas.default_days > canvas.max_days {
        return invalid("default_days must be from 1 to max_days");
    }
    Ok(())
}

fn validate_tools(tools: &Tools) -> Result<(), ConfigError> {
    if !tools.search {
        return Ok(());
    }
    if tools.live_default_source.trim().is_empty() {
        return Err(ConfigError::Invalid(
            "tools.live_default_source is empty; name the source search_live falls back to".into(),
        ));
    }
    for (name, text) in [
        ("knowledge_description", &tools.knowledge_description),
        ("live_description", &tools.live_description),
        ("query_description", &tools.query_description),
        ("source_description", &tools.source_description),
        ("nothing_stored", &tools.nothing_stored),
        ("stale_note", &tools.stale_note),
    ] {
        if text.trim().is_empty() {
            return Err(ConfigError::Invalid(format!(
                "tools.{name} is empty; the model would be given no wording for the search tools"
            )));
        }
    }
    Ok(())
}

/// Rejects telemetry settings that cannot export.
fn validate_telemetry(telemetry: &Telemetry) -> Result<(), ConfigError> {
    let invalid = |m: String| Err(ConfigError::Invalid(m));
    if !(0.0..=1.0).contains(&telemetry.sample_ratio) {
        return invalid(format!(
            "telemetry.sample_ratio must be between 0 and 1, got {}",
            telemetry.sample_ratio
        ));
    }
    if telemetry.provider_name.trim().is_empty() {
        return invalid("telemetry.provider_name is empty".into());
    }
    if telemetry.project_name.trim().is_empty() {
        return invalid("telemetry.project_name is empty".into());
    }
    Ok(())
}
