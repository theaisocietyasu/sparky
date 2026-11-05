//! The HTTP surface: the state each group of routes reads, and the router over them.

use std::sync::Arc;
use std::time::Duration;

use super::{Stores, background};
use crate::core::config::Config;
use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::identity::DiscordIdentity;
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::nudge::NudgeStore;
use crate::core::traits::oauth::OAuthStore;
use crate::core::traits::reminder::ReminderStore;
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::core::traits::tools::sandbox::Sandbox;
use crate::core::traits::trace::TraceReader;
use crate::core::types::admin::{AdminGuardrails, AdminLimits};
use crate::routes::Limits;
use crate::routes::admin::AdminState;
use crate::routes::canvas::CanvasState;
use crate::routes::chat::{ChatState, WebLists};
use crate::routes::health::HealthState;
use crate::routes::home::HomeRules;
use crate::routes::memories::MemoryState;
use crate::routes::notifications::NotificationState;
use crate::routes::oauth::OAuthState;
use crate::routes::rate_limit::RateLimiter;
use crate::routes::session::WebAuth;
use crate::routes::web::WebState;
use crate::runtime::harness::agent::Agent;
use crate::runtime::harness::trace::JsonlTraces;
use crate::runtime::tools::sandbox::ContainerSandbox;
use crate::stores::discord::DiscordOAuth;

/// What the chat routes need, gathered from the settings that describe it.
pub(super) fn chat_state(
    cfg: &Config,
    agent: Agent,
    conversations: Arc<dyn ConversationStore>,
    confirmations: Arc<dyn ConfirmationStore>,
    web: Option<WebAuth>,
    home: HomeRules,
) -> ChatState {
    ChatState {
        confirmations: Some(confirmations),
        agent,
        conversations: Some(conversations),
        request_budget: Duration::from_secs(cfg.agent.request_timeout_secs),
        default_tenant: cfg.discord.guild_id.to_string(),
        max_images: cfg.agent.max_images,
        max_files: cfg.agent.max_files,
        max_file_bytes: cfg.agent.max_file_bytes,
        turns: Arc::new(tokio::sync::Semaphore::new(cfg.agent.max_turns.max(1))),
        turn_wait: Duration::from_secs(cfg.agent.turn_queue_wait_secs),
        attachments_only_input: cfg.prompt.attachments_only_input.clone(),
        service_token: cfg.engine.service_token.clone(),
        rate_limit: RateLimiter::new(cfg.http.rate_limit_per_min),
        web,
        web_lists: WebLists {
            conversations: cfg.web.conversation_list,
            turns: cfg.web.conversation_turns,
            title_chars: cfg.web.title_chars,
        },
        home,
    }
}

/// The session reader, when the web sign-in is on.
pub(super) fn web_auth(cfg: &Config) -> anyhow::Result<Option<WebAuth>> {
    if !cfg.web.enabled {
        return Ok(None);
    }
    let key = cfg.web.key()?;
    Ok(Some(WebAuth::new(
        &key,
        &cfg.web.cookie_name,
        cfg.web.site(),
    )))
}

/// The sign-in routes and the Discord client behind them, when the web sign-in is on.
pub(super) fn web_state(cfg: &Config, auth: Option<WebAuth>) -> anyhow::Result<Option<WebState>> {
    let Some(auth) = auth else {
        return Ok(None);
    };
    let client_id = cfg.web.discord_client_id.clone().unwrap_or_default();
    let secret = cfg
        .web
        .discord_client_secret
        .clone()
        .ok_or_else(|| anyhow::anyhow!("SPARKY_WEB__DISCORD_CLIENT_SECRET is not set"))?;
    let identity = DiscordOAuth::new(
        &cfg.web.discord_api,
        &client_id,
        secret,
        Duration::from_secs(cfg.web.timeout_secs),
    )
    .map_err(|e| anyhow::anyhow!("web sign-in: {e}"))?;
    tracing::info!(site = %cfg.web.site(), "web sign-in on");
    Ok(Some(WebState {
        identity: Arc::new(identity) as Arc<dyn DiscordIdentity>,
        auth,
        client_id,
        authorize_url: cfg.web.discord_authorize_url.clone(),
        public_url: cfg.web.public().to_owned(),
        guild_id: cfg.discord.guild_id.to_string(),
        capability: capability(cfg),
        session_secs: i64::try_from(cfg.web.session_hours.saturating_mul(3600)).unwrap_or(i64::MAX),
    }))
}

/// What the admin routes need: the store, the agent they change, and who may call them.
pub(super) fn admin_state(
    cfg: &Config,
    agent: Agent,
    store: Option<Arc<dyn crate::core::traits::admin::AdminStore>>,
    queries: Option<Arc<dyn crate::core::traits::knowledge::query::SourceQueries>>,
    web: Option<WebAuth>,
) -> AdminState {
    AdminState {
        store,
        agent,
        tenant: cfg.discord.guild_id.to_string(),
        service_token: cfg.engine.service_token.clone(),
        web,
        capability: capability(cfg),
        limits: AdminLimits {
            max_turns: cfg.agent.max_turns,
            model_slots: cfg.agent.model_slots,
            rate_limit_per_min: cfg.http.rate_limit_per_min,
            memory_max_per_user: cfg.memory.max_per_user,
            conversation_idle_days: cfg.store.conversation_idle_days,
        },
        max_instructions_chars: cfg.admin.max_instructions_chars,
        history_limit: cfg.admin.history_limit,
        traces: cfg
            .trace
            .enabled
            .then(|| Arc::new(JsonlTraces::new(&cfg.trace.dir)) as Arc<dyn TraceReader>),
        traces_limit: cfg.admin.traces_limit,
        queries,
        guardrails: guardrails(cfg),
        phoenix_ui_url: cfg
            .telemetry
            .phoenix_ui_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_owned),
    }
}

/// The guardrail and policy settings as GET /admin/guardrails shows them.
fn guardrails(cfg: &Config) -> AdminGuardrails {
    let g = &cfg.guardrail;
    AdminGuardrails {
        enabled: g.enabled,
        denied_phrases: g.denied_phrases.clone(),
        protected_terms: g.protected_terms.clone(),
        redaction: g.redaction.clone(),
        max_answer_chars: g.max_answer_chars,
        replacement: g.replacement.clone(),
        write_roles: cfg.policy.write_roles.clone(),
        allow_authenticated_reads: cfg.policy.allow_authenticated_reads,
        confirm_from: serde_json::to_value(cfg.policy.confirm_from)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
    }
}

/// The role a session needs for the write side: the first write role.
fn capability(cfg: &Config) -> String {
    cfg.policy
        .write_roles
        .first()
        .cloned()
        .unwrap_or_else(|| "MANAGE_GUILD".to_owned())
}

/// What the memory routes need: the store, the clock offset, and the chat rate limit.
pub(super) fn memory_state(
    cfg: &Config,
    store: Arc<dyn MemoryStore>,
    rate_limit: RateLimiter,
    web: Option<WebAuth>,
) -> MemoryState {
    MemoryState {
        store,
        request_budget: Duration::from_secs(cfg.memory.request_timeout_secs),
        utc_offset_hours: cfg.prompt.utc_offset_hours,
        rate_limit,
        default_tenant: cfg.discord.guild_id.to_string(),
        service_token: cfg.engine.service_token.clone(),
        web,
    }
}

/// What the sandbox routes read and drive.
pub(super) fn sandbox_state(
    cfg: &Config,
    sandbox: Option<Arc<ContainerSandbox>>,
) -> crate::routes::sandbox::SandboxState {
    crate::routes::sandbox::SandboxState {
        sandbox: sandbox.map(|s| s as Arc<dyn Sandbox>),
        service_token: cfg.engine.service_token.clone(),
    }
}

/// The state the account link routes read: the grant store and the providers whose tool is on.
pub(super) fn oauth_state(
    cfg: &Config,
    store: Arc<dyn OAuthStore>,
    web: Option<WebAuth>,
) -> OAuthState {
    let providers = [
        ("google", cfg.gcal.enabled),
        ("microsoft", cfg.outlook.enabled),
    ]
    .into_iter()
    .filter(|(_, on)| *on)
    .map(|(name, _)| name.to_owned())
    .collect();
    OAuthState {
        store,
        service_token: cfg.engine.service_token.clone(),
        web,
        providers,
    }
}

/// The state the Canvas feed link routes read: the link store when Canvas is on.
pub(super) fn canvas_state(
    cfg: &Config,
    feeds: Option<Arc<dyn CanvasFeeds>>,
    web: Option<WebAuth>,
) -> CanvasState {
    CanvasState {
        feeds: feeds.filter(|_| cfg.canvas.enabled),
        hosts: cfg.canvas.hosts.clone(),
        service_token: cfg.engine.service_token.clone(),
        web,
    }
}

/// The state the reminder and notification routes read: the store when reminders are on.
pub(super) fn notification_state(
    cfg: &Config,
    store: Option<Arc<dyn ReminderStore>>,
    nudges: Option<Arc<dyn NudgeStore>>,
    web: Option<WebAuth>,
) -> NotificationState {
    NotificationState {
        store: store.filter(|_| cfg.reminders.enabled),
        nudges: nudges.filter(|_| cfg.nudges.enabled),
        nudge_defaults: background::defaults(cfg),
        default_tenant: cfg.discord.guild_id.to_string(),
        list_limit: cfg.reminders.list_limit,
        service_token: cfg.engine.service_token.clone(),
        web,
    }
}

/// The whole HTTP surface, with the state each group of routes reads.
pub(super) fn http_router(
    cfg: &Config,
    agent: Agent,
    stores: Stores,
    sandbox: Option<Arc<ContainerSandbox>>,
    home: HomeRules,
) -> anyhow::Result<axum::Router> {
    let web = web_auth(cfg)?;
    let admin = admin_state(
        cfg,
        agent.clone(),
        stores.admin,
        Some(Arc::clone(&stores.queries)),
        web.clone(),
    );
    let state = chat_state(
        cfg,
        agent,
        stores.conversations,
        stores.confirmations,
        web.clone(),
        home,
    );
    let memories = memory_state(
        cfg,
        Arc::clone(&stores.memory),
        state.rate_limit.clone(),
        web.clone(),
    );
    let health = HealthState {
        probes: stores.probes,
        model_base_url: cfg.model.base_url.clone(),
        model_api_key: cfg.model.api_key.clone(),
    };
    let limits = Limits {
        max_body_bytes: cfg.http.max_body_bytes,
        concurrency: cfg.http.concurrency_limit,
    };
    let states = crate::routes::States {
        chat: state,
        health,
        memories,
        sandbox: sandbox_state(cfg, sandbox),
        oauth: oauth_state(cfg, stores.oauth_store, web.clone()),
        canvas: canvas_state(cfg, stores.canvas, web.clone()),
        notifications: notification_state(cfg, stores.reminders, stores.nudges, web.clone()),
        web: web_state(cfg, web)?,
        admin,
    };
    Ok(crate::routes::router(
        states,
        limits,
        &cfg.http.cors_origins,
    ))
}
