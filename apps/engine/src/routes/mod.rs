//! Router assembly: /chat, /conversation, /memories, /sandbox, /oauth, /canvas, and /health.

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::HeaderValue;
use axum::routing::{delete, get, post};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub mod auth;
pub mod canvas;
pub mod chat;
pub mod conversation;
pub mod failure;
pub mod health;
pub mod memories;
pub mod oauth;
pub mod rate_limit;
pub mod sandbox;

/// Limits applied to the whole HTTP surface. Built from the http section; no Default.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Largest request body accepted, in bytes.
    pub max_body_bytes: usize,
    /// Requests handled at once. Zero removes the limit.
    pub concurrency: usize,
}

/// Cross-origin policy. A single * in origins allows any; an empty list adds no CORS headers.
pub fn cors(origins: &[String]) -> Option<CorsLayer> {
    if origins.is_empty() {
        return None;
    }
    if origins.iter().any(|o| o == "*") {
        return Some(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );
    }
    let mut parsed: Vec<HeaderValue> = Vec::with_capacity(origins.len());
    for origin in origins {
        match origin.parse() {
            Ok(value) => parsed.push(value),
            Err(error) => {
                tracing::warn!(%origin, %error, "cors origin is not a valid header value; skipped");
            }
        }
    }
    Some(
        CorsLayer::new()
            .allow_origin(parsed)
            .allow_methods(Any)
            .allow_headers(Any),
    )
}

/// The state each group of routes reads.
pub struct States {
    /// Chat, confirm and conversation routes.
    pub chat: chat::ChatState,
    /// Health routes.
    pub health: health::HealthState,
    /// Memory routes.
    pub memories: memories::MemoryState,
    /// Sandbox routes.
    pub sandbox: sandbox::SandboxState,
    /// Account link routes.
    pub oauth: oauth::OAuthState,
    /// Canvas feed link routes.
    pub canvas: canvas::CanvasState,
}

/// Full application router.
pub fn router(states: States, limits: Limits, cors_origins: &[String]) -> Router {
    let States {
        chat: chat_state,
        health: health_state,
        memories: memory_state,
        sandbox: sandbox_state,
        oauth: oauth_state,
        canvas: canvas_state,
    } = states;
    let health = Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .with_state(health_state);
    let chat = Router::new()
        .route("/chat", post(chat::chat))
        .route("/chat/stream", post(chat::stream))
        .route("/confirm", post(chat::confirm))
        .route("/conversation/reset", post(conversation::reset))
        .with_state(chat_state);
    let memories = Router::new()
        .route("/memories/list", post(memories::list))
        .route("/memories/forget", post(memories::forget))
        .with_state(memory_state);
    let sandbox = Router::new()
        .route("/sandbox", get(sandbox::report))
        .route("/sandbox/enabled", post(sandbox::switch))
        .route("/sandbox/{name}", delete(sandbox::kill))
        .with_state(sandbox_state);
    let oauth = Router::new()
        .route("/oauth/{provider}/authorize", post(oauth::authorize))
        .route("/oauth/{provider}/logout", post(oauth::disconnect))
        .with_state(oauth_state);
    let canvas = Router::new()
        .route("/canvas/feed", post(canvas::save))
        .route("/canvas/feed/remove", post(canvas::remove))
        .with_state(canvas_state);
    let mut router = Router::new()
        .merge(health)
        .merge(chat)
        .merge(memories)
        .merge(sandbox)
        .merge(oauth)
        .merge(canvas)
        .layer(DefaultBodyLimit::max(limits.max_body_bytes))
        .layer(TraceLayer::new_for_http());
    if let Some(layer) = cors(cors_origins) {
        router = router.layer(layer);
    }
    if limits.concurrency > 0 {
        router = router.layer(tower::limit::ConcurrencyLimitLayer::new(limits.concurrency));
    }
    router
}
