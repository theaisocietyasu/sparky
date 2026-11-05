//! Router assembly: /chat, /conversation, /memories, /sandbox, /oauth, /canvas, /notifications, /web, /admin, and /health.

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method, header};
use axum::routing::{delete, get, post, put};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub mod admin;
pub mod auth;
pub mod canvas;
pub mod chat;
pub mod conversation;
pub mod failure;
pub mod health;
pub mod memories;
pub mod notifications;
pub mod oauth;
pub mod rate_limit;
pub mod sandbox;
pub mod session;
pub mod web;

/// Limits applied to the whole HTTP surface. Built from the http section; no Default.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Largest request body accepted, in bytes.
    pub max_body_bytes: usize,
    /// Requests handled at once. Zero removes the limit.
    pub concurrency: usize,
}

/// Cross-origin policy. A single * in origins allows any; an empty list adds no CORS headers.
/// With credentials, the listed origins may send cookies and only the headers the page uses pass.
pub fn cors(origins: &[String], credentials: bool) -> Option<CorsLayer> {
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
    if credentials {
        return Some(
            CorsLayer::new()
                .allow_origin(parsed)
                .allow_methods([Method::GET, Method::POST, Method::PUT])
                .allow_headers([header::CONTENT_TYPE])
                .allow_credentials(true),
        );
    }
    Some(
        CorsLayer::new()
            .allow_origin(parsed)
            .allow_methods(Any)
            .allow_headers(Any),
    )
}

/// The sign-in routes, or none while the web sign-in is off.
fn web_routes(state: Option<web::WebState>) -> Router {
    let Some(state) = state else {
        return Router::new();
    };
    Router::new()
        .route("/web/login", get(web::login))
        .route("/web/callback", get(web::callback))
        .route("/web/me", get(web::me))
        .route("/web/logout", post(web::logout))
        .with_state(state)
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
    /// Reminder and notification routes.
    pub notifications: notifications::NotificationState,
    /// Web sign-in routes. None while the web sign-in is off.
    pub web: Option<web::WebState>,
    /// Admin routes.
    pub admin: admin::AdminState,
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
        notifications: notification_state,
        web: web_state,
        admin: admin_state,
    } = states;
    let credentials = web_state.is_some();
    let health = Router::new()
        .route("/health/live", get(health::live))
        .route("/health/ready", get(health::ready))
        .with_state(health_state);
    let chat = Router::new()
        .route("/chat", post(chat::chat))
        .route("/chat/stream", post(chat::stream))
        .route("/confirm", post(chat::confirm))
        .route("/conversation/reset", post(conversation::reset))
        .route("/web/conversations", get(conversation::list))
        .route(
            "/web/conversations/{id}",
            get(conversation::read).delete(conversation::delete),
        )
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
        .route("/canvas/feed", post(canvas::save).get(canvas::status))
        .route("/canvas/feed/remove", post(canvas::remove))
        .with_state(canvas_state);
    let notifications = Router::new()
        .route("/web/notifications", get(notifications::list))
        .route("/web/notifications/read", post(notifications::read))
        .route("/web/reminders", get(notifications::reminders))
        .route("/web/reminders/{id}", delete(notifications::cancel))
        .route("/notifications/outgoing", get(notifications::outgoing))
        .route("/notifications/sent", post(notifications::sent))
        .with_state(notification_state);
    let admin = Router::new()
        .route("/admin/tools", get(admin::tools))
        .route("/admin/tools/{name}", post(admin::switch_tool))
        .route("/admin/instructions", get(admin::instructions))
        .route("/admin/instructions", put(admin::set_instructions))
        .route("/admin/history", get(admin::history))
        .route("/admin/rollback/{id}", post(admin::rollback))
        .route("/admin/limits", get(admin::limits))
        .route("/admin/guardrails", get(admin::guardrails))
        .route("/admin/traces", get(admin::traces))
        .route("/admin/traces/{id}", get(admin::trace))
        .route("/admin/sources", get(admin::sources))
        .route("/admin/approvals", get(admin::approvals))
        .route("/admin/approvals/{token}/deny", post(admin::deny))
        .with_state(admin_state);
    let mut router = Router::new()
        .merge(health)
        .merge(admin)
        .merge(chat)
        .merge(memories)
        .merge(sandbox)
        .merge(oauth)
        .merge(canvas)
        .merge(notifications)
        .merge(web_routes(web_state))
        .layer(DefaultBodyLimit::max(limits.max_body_bytes))
        .layer(TraceLayer::new_for_http());
    if let Some(layer) = cors(cors_origins, credentials) {
        router = router.layer(layer);
    }
    if limits.concurrency > 0 {
        router = router.layer(tower::limit::ConcurrencyLimitLayer::new(limits.concurrency));
    }
    router
}
