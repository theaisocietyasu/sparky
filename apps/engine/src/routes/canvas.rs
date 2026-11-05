//! Canvas feed links: save checks and seals a caller's Calendar Feed link; remove deletes it;
//! status says whether a web session has one. The bot calls with the service token, the web chat
//! with its session, which always acts on its own user.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;

use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::core::types::http::canvas::{
    FeedStatus, RemoveFeedRequest, RemoveFeedResponse, SaveFeedRequest, SaveFeedResponse,
};
use crate::core::types::tools::canvas::feed_link;
use crate::routes::auth::{Caller, caller};
use crate::routes::session::WebAuth;

/// What the Canvas routes read.
#[derive(Clone)]
pub struct CanvasState {
    /// Where links are kept. None when Canvas is off.
    pub feeds: Option<Arc<dyn CanvasFeeds>>,
    /// Hosts a link may point at.
    pub hosts: Vec<String>,
    /// Bearer token the bot calls with.
    pub service_token: SecretString,
    /// Reads web sessions. None while the web sign-in is off.
    pub web: Option<WebAuth>,
}

impl CanvasState {
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
}

/// Checks and keeps a caller's feed link.
pub async fn save(
    State(state): State<CanvasState>,
    headers: HeaderMap,
    Json(req): Json<SaveFeedRequest>,
) -> Response {
    let user = match state.user(&headers, req.user) {
        Ok(user) => user,
        Err(refused) => return refused,
    };
    let Some(feeds) = state.feeds else {
        return not_enabled();
    };
    let link = match feed_link(&req.link, &state.hosts) {
        Ok(link) => link,
        Err(reason) => return (StatusCode::UNPROCESSABLE_ENTITY, reason).into_response(),
    };
    match feeds.save(&user, &link).await {
        Ok(()) => Json(SaveFeedResponse { saved: true }).into_response(),
        Err(error) => {
            tracing::error!(%error, "could not keep a Canvas feed link");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not save the link").into_response()
        }
    }
}

/// Removes a caller's feed link.
pub async fn remove(
    State(state): State<CanvasState>,
    headers: HeaderMap,
    Json(req): Json<RemoveFeedRequest>,
) -> Response {
    let user = match state.user(&headers, req.user) {
        Ok(user) => user,
        Err(refused) => return refused,
    };
    let Some(feeds) = state.feeds else {
        return not_enabled();
    };
    match feeds.delete(&user).await {
        Ok(removed) => Json(RemoveFeedResponse { removed }).into_response(),
        Err(error) => {
            tracing::error!(%error, "could not remove a Canvas feed link");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not remove the link",
            )
                .into_response()
        }
    }
}

/// GET /canvas/feed: whether the signed-in student has a link. Web sessions only.
pub async fn status(State(state): State<CanvasState>, headers: HeaderMap) -> Response {
    let Some(session) = state.web.as_ref().and_then(|w| w.session(&headers)) else {
        return (StatusCode::UNAUTHORIZED, "sign in first").into_response();
    };
    let Some(feeds) = state.feeds else {
        return not_enabled();
    };
    match feeds.load(&session.user_id).await {
        Ok(link) => Json(FeedStatus {
            linked: link.is_some(),
        })
        .into_response(),
        Err(error) => {
            tracing::error!(%error, "could not read a Canvas feed link");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not read the link").into_response()
        }
    }
}

/// The answer when Canvas is off.
fn not_enabled() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, "Canvas is not enabled").into_response()
}
