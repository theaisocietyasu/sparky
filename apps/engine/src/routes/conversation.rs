//! POST /conversation/reset: ends the open conversations of one caller in one channel.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::{ResetRequest, ResetResponse};
use crate::routes::auth::{Caller, caller, too_many};
use crate::routes::chat::ChatState;
use crate::routes::failure::Failure;
use crate::routes::session::WEB_CHANNEL;

/// Ends every open conversation of the caller in the channel, any visibility. Next turn starts new.
pub async fn reset(
    State(state): State<ChatState>,
    headers: HeaderMap,
    Json(mut req): Json<ResetRequest>,
) -> Response {
    match caller(&headers, &state.service_token, state.web.as_ref()) {
        Ok(Caller::Service) => {}
        Ok(Caller::Web(session)) => {
            req.user = session.user_id;
            req.tenant = None;
            WEB_CHANNEL.clone_into(&mut req.channel);
        }
        Err(refused) => return refused,
    }
    if !state.rate_limit.allow(&req.user) {
        return too_many(&req.user);
    }
    if req.user.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "user_id is empty").into_response();
    }
    if req.channel.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "channel_id is empty").into_response();
    }
    let Some(store) = &state.conversations else {
        return (
            StatusCode::NOT_IMPLEMENTED,
            "this engine keeps no conversations",
        )
            .into_response();
    };
    let tenant = req.tenant.unwrap_or_else(|| state.default_tenant.clone());
    let ctx = RequestContext::new(tenant, req.user, state.request_budget);
    match store.end(&ctx, &req.channel).await {
        Ok(ended) => {
            tracing::info!(user = %ctx.user_id, ended, "conversations reset");
            Json(ResetResponse { ended }).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "conversation reset failed");
            Failure::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ctx.request_id,
                "conversation store unavailable",
            )
            .into_response()
        }
    }
}
