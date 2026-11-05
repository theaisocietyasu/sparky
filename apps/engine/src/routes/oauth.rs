//! Per-user account links: authorize hands out the store's login link; disconnect removes a grant;
//! status says whether a web session has one. The bot calls with the service token, the web with its
//! session, which always acts on its own user.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;

use crate::core::traits::oauth::OAuthStore;
use crate::core::types::http::oauth::{
    AuthorizeRequest, AuthorizeResponse, DisconnectResponse, LinkStatus,
};
use crate::core::types::tools::oauth::USER_SCOPE;
use crate::routes::auth::{Caller, caller};
use crate::routes::session::WebAuth;

/// What the OAuth routes read.
#[derive(Clone)]
pub struct OAuthState {
    /// Where grants are held and logins start.
    pub store: Arc<dyn OAuthStore>,
    /// Bearer token the bot calls with.
    pub service_token: SecretString,
    /// Reads web sessions. None while the web sign-in is off.
    pub web: Option<WebAuth>,
    /// Providers whose account tool is on. Any other is refused.
    pub providers: Vec<String>,
}

impl OAuthState {
    /// The user a request acts for: the body's for the service token, the session's for the web.
    fn user(&self, headers: &HeaderMap, claimed: String) -> Result<String, Response> {
        let user = match caller(headers, &self.service_token, self.web.as_ref())? {
            Caller::Service => claimed,
            Caller::Web(session) => session.user_id,
        };
        if user.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "user is empty").into_response());
        }
        Ok(user)
    }

    /// Refuses a provider whose account tool is off.
    fn offered(&self, provider: &str) -> Result<(), Response> {
        if self.providers.iter().any(|p| p == provider) {
            Ok(())
        } else {
            Err(not_enabled())
        }
    }
}

/// Hands out the login link for a provider.
pub async fn authorize(
    State(state): State<OAuthState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Option<Json<AuthorizeRequest>>,
) -> Response {
    let claimed = body.map(|Json(req)| req.user).unwrap_or_default();
    let user = match state.user(&headers, claimed) {
        Ok(user) => user,
        Err(refused) => return refused,
    };
    if let Err(refused) = state.offered(&provider) {
        return refused;
    }
    match state.store.login_link(&user, &provider).await {
        Ok(Some(url)) => Json(AuthorizeResponse { url }).into_response(),
        Ok(None) => not_enabled(),
        Err(error) => {
            tracing::error!(%error, %provider, "could not start a login");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not start login").into_response()
        }
    }
}

/// Removes a caller's grant.
pub async fn disconnect(
    State(state): State<OAuthState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    body: Option<Json<AuthorizeRequest>>,
) -> Response {
    let claimed = body.map(|Json(req)| req.user).unwrap_or_default();
    let user = match state.user(&headers, claimed) {
        Ok(user) => user,
        Err(refused) => return refused,
    };
    match state.store.delete_grant(USER_SCOPE, &user, &provider).await {
        Ok(removed) => Json(DisconnectResponse { removed }).into_response(),
        Err(error) => {
            tracing::error!(%error, %provider, "could not delete a grant");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not disconnect").into_response()
        }
    }
}

/// Whether a web session has an account linked for a provider.
pub async fn status(
    State(state): State<OAuthState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(session) = state.web.as_ref().and_then(|w| w.session(&headers)) else {
        return (StatusCode::UNAUTHORIZED, "sign in first").into_response();
    };
    if let Err(refused) = state.offered(&provider) {
        return refused;
    }
    match state
        .store
        .load_grant(USER_SCOPE, &session.user_id, &provider)
        .await
    {
        Ok(grant) => Json(LinkStatus {
            linked: grant.is_some(),
        })
        .into_response(),
        Err(error) => {
            tracing::error!(%error, %provider, "could not read a grant");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not read the link").into_response()
        }
    }
}

/// The answer when no login is offered for a provider.
fn not_enabled() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "login is not enabled for this provider",
    )
        .into_response()
}
