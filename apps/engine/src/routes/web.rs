//! Discord sign-in for the web page: login, callback, me and logout.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{AeadCore, OsRng};

use crate::core::traits::identity::DiscordIdentity;
use crate::core::types::http::web::{ADMINISTRATOR, MANAGE_GUILD, MeResponse, Session};
use crate::routes::auth::same_secret;
use crate::routes::session::{WebAuth, cookie, set_cookie};

/// How long a sign-in may take between login and callback, in seconds.
const STATE_SECS: i64 = 600;

/// What the sign-in routes read.
#[derive(Clone)]
pub struct WebState {
    /// Turns a Discord code into the account and its guilds.
    pub identity: Arc<dyn DiscordIdentity>,
    /// Seals and opens sessions.
    pub auth: WebAuth,
    /// Discord application id.
    pub client_id: String,
    /// The Discord page a student signs in on.
    pub authorize_url: String,
    /// Public URL of this engine, no trailing slash.
    pub public_url: String,
    /// The guild a student must be a member of.
    pub guild_id: String,
    /// Role name given to a member who may manage the guild, as the bot gives it.
    pub capability: String,
    /// How long a session lasts, in seconds.
    pub session_secs: i64,
}

impl WebState {
    /// Name of the cookie that carries the sign-in state.
    fn state_cookie(&self) -> String {
        format!("{}_state", self.auth.cookie_name)
    }

    /// Path of the sign-in routes as the browser sees them, under the path of the public URL.
    fn state_path(&self) -> String {
        let base = url::Url::parse(&self.public_url)
            .map(|u| u.path().trim_end_matches('/').to_owned())
            .unwrap_or_default();
        format!("{base}/web")
    }

    /// Where Discord sends the student back.
    fn redirect_uri(&self) -> String {
        format!("{}/web/callback", self.public_url)
    }

    /// A 303 to the page, with signin set to outcome, clearing the state cookie.
    fn back(&self, outcome: &str, session: Option<String>) -> Response {
        let target = format!("{}/chat?signin={outcome}", self.auth.site);
        let mut response = (StatusCode::SEE_OTHER, [(header::LOCATION, target)]).into_response();
        let clear = set_cookie(&self.state_cookie(), "", &self.state_path(), 0);
        append_cookie(&mut response, &clear);
        if let Some(value) = session {
            let set = set_cookie(&self.auth.cookie_name, &value, "/", self.session_secs);
            append_cookie(&mut response, &set);
        }
        response
    }
}

/// Adds one Set-Cookie header.
fn append_cookie(response: &mut Response, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

/// Sends the student to Discord with a fresh state value held in a short-lived cookie.
pub async fn login(State(state): State<WebState>) -> Response {
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let value = URL_SAFE_NO_PAD.encode(nonce);
    let target = url::Url::parse_with_params(
        &state.authorize_url,
        &[
            ("response_type", "code"),
            ("client_id", state.client_id.as_str()),
            ("scope", "identify guilds"),
            ("redirect_uri", state.redirect_uri().as_str()),
            ("state", value.as_str()),
            ("prompt", "none"),
        ],
    );
    let Ok(target) = target else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "sign-in is misconfigured",
        )
            .into_response();
    };
    let mut response = (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, target.to_string())],
    )
        .into_response();
    append_cookie(
        &mut response,
        &set_cookie(
            &state.state_cookie(),
            &value,
            &state.state_path(),
            STATE_SECS,
        ),
    );
    response
}

/// Checks the state, reads the Discord account, and signs a guild member in.
pub async fn callback(
    State(state): State<WebState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let sent = query.get("state").map(String::as_str).unwrap_or_default();
    let held = cookie(&headers, &state.state_cookie()).unwrap_or_default();
    if sent.is_empty() || !same_secret(sent.as_bytes(), held.as_bytes()) {
        return state.back("expired", None);
    }
    let Some(code) = query.get("code").filter(|c| !c.is_empty()) else {
        return state.back("cancelled", None);
    };
    let user = match state.identity.sign_in(code, &state.redirect_uri()).await {
        Ok(user) => user,
        Err(error) => {
            tracing::warn!(%error, "web sign-in failed");
            return state.back("failed", None);
        }
    };
    let Some(guild) = user.guilds.iter().find(|g| g.id == state.guild_id) else {
        tracing::info!(user = %user.id, "web sign-in refused: not a member of the guild");
        return state.back("not_member", None);
    };
    let mut roles = Vec::new();
    if guild.permissions & (MANAGE_GUILD | ADMINISTRATOR) != 0 {
        roles.push(state.capability.clone());
    }
    let session = Session {
        user_id: user.id,
        name: user.name,
        roles,
        expires_at: chrono::Utc::now().timestamp() + state.session_secs,
    };
    let Some(value) = state.auth.seal(&session) else {
        return state.back("failed", None);
    };
    tracing::info!(user = %session.user_id, "web sign-in");
    state.back("ok", Some(value))
}

/// The signed-in student, or 401.
pub async fn me(State(state): State<WebState>, headers: HeaderMap) -> Response {
    match state.auth.session(&headers) {
        Some(s) => Json(MeResponse {
            user_id: s.user_id,
            name: s.name,
            roles: s.roles,
            expires_at: s.expires_at,
        })
        .into_response(),
        None => (StatusCode::UNAUTHORIZED, "not signed in").into_response(),
    }
}

/// Ends the session. Only the site may ask.
pub async fn logout(State(state): State<WebState>, headers: HeaderMap) -> Response {
    if !state.auth.is_from_site(&headers) {
        return (StatusCode::FORBIDDEN, "wrong origin").into_response();
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    append_cookie(
        &mut response,
        &set_cookie(&state.auth.cookie_name, "", "/", 0),
    );
    response
}
