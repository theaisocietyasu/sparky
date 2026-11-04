//! Canvas feed links: save checks and seals a caller's Calendar Feed link; remove deletes it.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;

use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::core::types::http::canvas::{
    RemoveFeedRequest, RemoveFeedResponse, SaveFeedRequest, SaveFeedResponse,
};
use crate::core::types::tools::canvas::feed_link;
use crate::routes::auth::authorized;

/// What the Canvas routes read.
#[derive(Clone)]
pub struct CanvasState {
    /// Where links are kept. None when Canvas is off.
    pub feeds: Option<Arc<dyn CanvasFeeds>>,
    /// Hosts a link may point at.
    pub hosts: Vec<String>,
    /// Bearer token both routes require.
    pub service_token: SecretString,
}

/// Checks and keeps a caller's feed link. Requires the service token.
pub async fn save(
    State(state): State<CanvasState>,
    headers: HeaderMap,
    Json(req): Json<SaveFeedRequest>,
) -> Response {
    if !authorized(&headers, &state.service_token) {
        return (StatusCode::UNAUTHORIZED, "missing or wrong bearer token").into_response();
    }
    let Some(feeds) = state.feeds else {
        return not_enabled();
    };
    let link = match feed_link(&req.link, &state.hosts) {
        Ok(link) => link,
        Err(reason) => return (StatusCode::UNPROCESSABLE_ENTITY, reason).into_response(),
    };
    match feeds.save(&req.user, &link).await {
        Ok(()) => Json(SaveFeedResponse { saved: true }).into_response(),
        Err(error) => {
            tracing::error!(%error, "could not keep a Canvas feed link");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not save the link").into_response()
        }
    }
}

/// Removes a caller's feed link. Requires the service token.
pub async fn remove(
    State(state): State<CanvasState>,
    headers: HeaderMap,
    Json(req): Json<RemoveFeedRequest>,
) -> Response {
    if !authorized(&headers, &state.service_token) {
        return (StatusCode::UNAUTHORIZED, "missing or wrong bearer token").into_response();
    }
    let Some(feeds) = state.feeds else {
        return not_enabled();
    };
    match feeds.delete(&req.user).await {
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

/// The answer when Canvas is off.
fn not_enabled() -> Response {
    (StatusCode::SERVICE_UNAVAILABLE, "Canvas is not enabled").into_response()
}
