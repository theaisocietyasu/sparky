//! The HTTP surface: the state each group of routes reads, and the router over them.

use std::sync::Arc;
use std::time::Duration;

use crate::core::config::Config;
use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::health::Probe;
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::memory::profile::ProfileGraph;
use crate::core::traits::oauth::OAuthStore;
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::traits::tools::sandbox::Sandbox;
use crate::routes::Limits;
use crate::routes::chat::ChatState;
use crate::routes::health::HealthState;
use crate::routes::memories::MemoryState;
use crate::routes::oauth::OAuthState;
use crate::routes::profile::ProfileState;
use crate::routes::rate_limit::RateLimiter;
use crate::runtime::harness::agent::Agent;
use crate::runtime::tools::sandbox::ContainerSandbox;

/// What the chat routes need, gathered from the settings that describe it.
pub(super) fn chat_state(
    cfg: &Config,
    agent: Agent,
    conversations: Arc<dyn ConversationStore>,
    confirmations: Arc<dyn ConfirmationStore>,
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
    }
}

/// What the profile routes need, gathered from the settings that describe it.
pub(super) fn profile_state(
    cfg: &Config,
    graph: Option<Arc<dyn ProfileGraph>>,
    rate_limit: RateLimiter,
) -> ProfileState {
    ProfileState {
        graph,
        list_limit: cfg.profile.list_limit,
        request_budget: Duration::from_secs(cfg.profile.request_timeout_secs),
        rate_limit,
        default_tenant: cfg.discord.guild_id.to_string(),
        service_token: cfg.engine.service_token.clone(),
    }
}

/// What the memory routes need: the store, the clock offset, and the chat rate limit.
pub(super) fn memory_state(
    cfg: &Config,
    store: Arc<dyn MemoryStore>,
    rate_limit: RateLimiter,
) -> MemoryState {
    MemoryState {
        store,
        request_budget: Duration::from_secs(cfg.profile.request_timeout_secs),
        utc_offset_hours: cfg.prompt.utc_offset_hours,
        rate_limit,
        default_tenant: cfg.discord.guild_id.to_string(),
        service_token: cfg.engine.service_token.clone(),
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

/// The state the account link routes read: the grant store.
pub(super) fn oauth_state(cfg: &Config, store: Arc<dyn OAuthStore>) -> OAuthState {
    OAuthState {
        store,
        service_token: cfg.engine.service_token.clone(),
    }
}

/// The whole HTTP surface, with the state each group of routes reads.
pub(super) fn http_router(
    cfg: &Config,
    state: ChatState,
    profile: ProfileState,
    memories: MemoryState,
    oauth: OAuthState,
    probes: Vec<Arc<dyn Probe>>,
    sandbox: Option<Arc<ContainerSandbox>>,
) -> axum::Router {
    let health = HealthState {
        probes,
        model_base_url: cfg.model.base_url.clone(),
    };
    let limits = Limits {
        max_body_bytes: cfg.http.max_body_bytes,
        concurrency: cfg.http.concurrency_limit,
    };
    let states = crate::routes::States {
        chat: state,
        health,
        profile,
        memories,
        sandbox: sandbox_state(cfg, sandbox),
        oauth,
    };
    crate::routes::router(states, limits, &cfg.http.cors_origins)
}
