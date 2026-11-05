//! POST /conversation/reset ends the open conversations of one caller in one channel. The web
//! session finds its home conversation, and lists, reads and deletes its own web conversations.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::Role;
use crate::core::types::conversation::{
    Home, ResetRequest, ResetResponse, Turn, Visibility, home_id,
};
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

/// The web session's context for conversation id, or the refusal.
fn web_context(
    state: &ChatState,
    headers: &HeaderMap,
    id: Option<&str>,
) -> Result<RequestContext, Response> {
    let Some(session) = state.web.as_ref().and_then(|w| w.session(headers)) else {
        return Err((StatusCode::UNAUTHORIZED, "sign in first").into_response());
    };
    if !state.rate_limit.allow(&session.user_id) {
        return Err(too_many(&session.user_id));
    }
    if state.conversations.is_none() {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            "this engine keeps no conversations",
        )
            .into_response());
    }
    let ctx = RequestContext::new(
        state.default_tenant.clone(),
        session.user_id,
        state.request_budget,
    )
    .with_visibility(Visibility::Private);
    match id {
        None => Ok(ctx),
        Some(id) => match uuid::Uuid::parse_str(id) {
            Ok(id) => Ok(ctx.with_conversation(id)),
            Err(_) => Err((StatusCode::NOT_FOUND, "no conversation by that id").into_response()),
        },
    }
}

/// The store failed; the caller is told only that.
fn unavailable(ctx: &RequestContext, error: &crate::core::types::store::StoreError) -> Response {
    tracing::error!(%error, "conversation store failed");
    Failure::new(
        StatusCode::SERVICE_UNAVAILABLE,
        ctx.request_id,
        "conversation store unavailable",
    )
    .into_response()
}

/// GET /web/conversations: the signed-in student's open web conversations, newest first.
pub async fn list(State(state): State<ChatState>, headers: HeaderMap) -> Response {
    let ctx = match web_context(&state, &headers, None) {
        Ok(ctx) => ctx,
        Err(refused) => return refused,
    };
    let Some(store) = &state.conversations else {
        return StatusCode::NOT_IMPLEMENTED.into_response();
    };
    match store
        .list(&ctx, WEB_CHANNEL, state.web_lists.conversations)
        .await
    {
        Ok(mut list) => {
            for item in &mut list {
                item.title = item
                    .title
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(state.web_lists.title_chars)
                    .collect();
            }
            Json(list).into_response()
        }
        Err(e) => unavailable(&ctx, &e),
    }
}

/// GET /web/home: the id of the student's home conversation, made when missing.
pub async fn home(State(state): State<ChatState>, headers: HeaderMap) -> Response {
    let ctx = match web_context(&state, &headers, None) {
        Ok(ctx) => ctx,
        Err(refused) => return refused,
    };
    let id = home_id(&ctx.tenant_id, &ctx.user_id);
    let ctx = ctx.with_conversation(id);
    let Some(store) = &state.conversations else {
        return StatusCode::NOT_IMPLEMENTED.into_response();
    };
    match store.ensure(&ctx, WEB_CHANNEL).await {
        Ok(()) => Json(Home { id }).into_response(),
        Err(e) => unavailable(&ctx, &e),
    }
}

/// GET /web/conversations/{id}: the questions and answers of one of the student's conversations.
pub async fn read(
    State(state): State<ChatState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let ctx = match web_context(&state, &headers, Some(&id)) {
        Ok(ctx) => ctx,
        Err(refused) => return refused,
    };
    let Some(store) = &state.conversations else {
        return StatusCode::NOT_IMPLEMENTED.into_response();
    };
    match store.owns(&ctx).await {
        Ok(true) => {}
        Ok(false) => return (StatusCode::NOT_FOUND, "no conversation by that id").into_response(),
        Err(e) => return unavailable(&ctx, &e),
    }
    match store.load(&ctx, state.web_lists.turns).await {
        Ok(stored) => {
            let turns: Vec<Turn> = stored
                .into_iter()
                .filter_map(|s| {
                    let role = match s.message.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                        _ => return None,
                    };
                    let text = s.message.content.trim();
                    (!text.is_empty()).then(|| Turn {
                        position: s.position,
                        role: role.to_owned(),
                        text: text.to_owned(),
                        notice: s.message.notice.clone(),
                    })
                })
                .collect();
            Json(turns).into_response()
        }
        Err(e) => unavailable(&ctx, &e),
    }
}

/// DELETE /web/conversations/{id}: deletes one of the student's conversations and its messages.
pub async fn delete(
    State(state): State<ChatState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let ctx = match web_context(&state, &headers, Some(&id)) {
        Ok(ctx) => ctx,
        Err(refused) => return refused,
    };
    let Some(store) = &state.conversations else {
        return StatusCode::NOT_IMPLEMENTED.into_response();
    };
    match store.delete(&ctx).await {
        Ok(true) => Json(serde_json::json!({ "deleted": true })).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "no conversation by that id").into_response(),
        Err(e) => unavailable(&ctx, &e),
    }
}
