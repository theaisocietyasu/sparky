//! What every route checks first: the service bearer token or a web session, and the per-user rate limit.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use secrecy::{ExposeSecret, SecretString};

use crate::core::types::http::web::Session;
use crate::routes::session::WebAuth;

/// Whether the headers carry the service bearer token.
pub(crate) fn authorized(headers: &HeaderMap, token: &SecretString) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|presented| {
            same_secret(presented.as_bytes(), token.expose_secret().as_bytes())
        })
}

/// Whether two secrets are equal, in time that depends only on their lengths.
pub(crate) fn same_secret(presented: &[u8], expected: &[u8]) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    presented
        .iter()
        .zip(expected)
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

/// The 429 returned when a caller is over the limit.
pub fn too_many(user: &str) -> Response {
    tracing::warn!(user, "rate limited");
    (
        StatusCode::TOO_MANY_REQUESTS,
        "too many requests; wait a minute and ask again",
    )
        .into_response()
}

/// Who is calling: the bot with the service token, or a student with a web session.
pub(crate) enum Caller {
    /// The bot. The user and roles come from the request body.
    Service,
    /// A signed-in student. The user and roles come from the session, never the body.
    Web(Session),
}

/// The caller of a request, or the 401 that refuses it.
pub(crate) fn caller(
    headers: &HeaderMap,
    token: &SecretString,
    web: Option<&WebAuth>,
) -> Result<Caller, Response> {
    if authorized(headers, token) {
        return Ok(Caller::Service);
    }
    if let Some(session) = web.and_then(|w| w.session(headers)) {
        return Ok(Caller::Web(session));
    }
    Err((
        StatusCode::UNAUTHORIZED,
        "missing or wrong bearer token or session",
    )
        .into_response())
}
