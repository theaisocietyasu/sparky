//! POST /memories/list shows what Sparky remembers about one user. POST /memories/forget removes it.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::FixedOffset;
use secrecy::SecretString;

use crate::core::traits::memory::MemoryStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::listing::{
    ListedMemory, MemoryForgetRequest, MemoryForgetResponse, MemoryList, MemoryListRequest,
};
use crate::core::types::memory::{Memory, MemoryKind};
use crate::routes::auth::{Caller, caller, too_many};
use crate::routes::rate_limit::RateLimiter;
use crate::routes::session::WebAuth;

/// What the memory routes need.
#[derive(Clone)]
pub struct MemoryState {
    /// Where the memories are.
    pub store: Arc<dyn MemoryStore>,
    /// Wall-clock budget for one call.
    pub request_budget: Duration,
    /// Hours from UTC the dates are written in.
    pub utc_offset_hours: i32,
    /// Per-user request limit, shared with the chat routes.
    pub rate_limit: RateLimiter,
    /// Tenant used when the client sends none.
    pub default_tenant: String,
    /// Bearer token every caller must present.
    pub service_token: SecretString,
    /// Reads web sessions. None while the web sign-in is off.
    pub web: Option<WebAuth>,
}

impl MemoryState {
    /// The context of the caller, or the response that refuses the call.
    fn caller(
        &self,
        headers: &HeaderMap,
        user_id: String,
        tenant_id: Option<String>,
    ) -> Result<RequestContext, Response> {
        let (user_id, tenant_id) = match caller(headers, &self.service_token, self.web.as_ref())? {
            Caller::Service => (user_id, tenant_id),
            Caller::Web(session) => (session.user_id, None),
        };
        if !self.rate_limit.allow(&user_id) {
            return Err(too_many(&user_id));
        }
        if user_id.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "user_id is empty").into_response());
        }
        let tenant = tenant_id.unwrap_or_else(|| self.default_tenant.clone());
        Ok(RequestContext::new(tenant, user_id, self.request_budget))
    }

    /// One memory as the list shows it.
    fn listed(&self, number: usize, memory: &Memory) -> ListedMemory {
        let offset = FixedOffset::east_opt(self.utc_offset_hours * 3600);
        let day = |t: chrono::DateTime<chrono::Utc>| match offset {
            Some(offset) => t.with_timezone(&offset).format("%Y-%m-%d").to_string(),
            None => t.format("%Y-%m-%d").to_string(),
        };
        ListedMemory {
            number,
            id: memory.short_id(),
            kind: memory.kind.as_str().to_owned(),
            text: memory.content.clone(),
            created_at: day(memory.created_at),
            due: match memory.kind {
                MemoryKind::Task => memory.expires_at.map(day),
                _ => None,
            },
        }
    }
}

/// A store failure, logged.
fn unavailable(error: &impl std::fmt::Display, what: &str) -> Response {
    tracing::error!(error = %error, "memory {what} failed");
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "could not reach the memory store",
    )
        .into_response()
}

/// Lists the caller's memories, numbered from 1 in recall order.
pub async fn list(
    State(state): State<MemoryState>,
    headers: HeaderMap,
    Json(req): Json<MemoryListRequest>,
) -> Response {
    let ctx = match state.caller(&headers, req.user_id, req.tenant_id) {
        Ok(ctx) => ctx,
        Err(refused) => return refused,
    };
    match state.store.recall(&ctx).await {
        Ok(memories) => Json(MemoryList {
            memories: memories
                .iter()
                .enumerate()
                .map(|(at, m)| state.listed(at + 1, m))
                .collect(),
        })
        .into_response(),
        Err(error) => unavailable(&error, "list"),
    }
}

/// Removes one memory by its number in the list, or every memory when no number is sent.
pub async fn forget(
    State(state): State<MemoryState>,
    headers: HeaderMap,
    Json(req): Json<MemoryForgetRequest>,
) -> Response {
    let ctx = match state.caller(&headers, req.user_id, req.tenant_id) {
        Ok(ctx) => ctx,
        Err(refused) => return refused,
    };
    let removed = match req.number {
        None => state.store.forget_all(&ctx).await,
        Some(number) => {
            let held = match state.store.recall(&ctx).await {
                Ok(held) => held,
                Err(error) => return unavailable(&error, "forget"),
            };
            let Some(memory) = number.checked_sub(1).and_then(|at| held.get(at)) else {
                return (StatusCode::NOT_FOUND, "no memory with that number").into_response();
            };
            state.store.forget(&ctx, memory.id).await.map(u64::from)
        }
    };
    match removed {
        Ok(removed) => {
            tracing::info!(user = %ctx.user_id, removed, "memories forgotten");
            Json(MemoryForgetResponse { removed }).into_response()
        }
        Err(error) => unavailable(&error, "forget"),
    }
}
