//! /admin/*: moderators switch tools, edit the instructions with history and rollback, and deny held actions.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde_json::{Value, json};

use crate::core::traits::admin::AdminStore;
use crate::core::types::admin::{
    AdminChange, AdminLimits, ChangeKind, INSTRUCTIONS_KEY, InstructionsState, InstructionsUpdate,
    NewChange, ToolState, ToolSwitch,
};
use crate::core::types::store::StoreError;
use crate::routes::auth::{Caller, caller};
use crate::routes::session::WebAuth;
use crate::runtime::harness::agent::Agent;

/// What the admin routes need.
#[derive(Clone)]
pub struct AdminState {
    /// Where changes are recorded. None when the engine keeps no local store.
    pub store: Option<Arc<dyn AdminStore>>,
    /// The agent whose tools and instructions change.
    pub agent: Agent,
    /// Scope the changes are recorded under.
    pub tenant: String,
    /// Bearer token that also admits a caller.
    pub service_token: SecretString,
    /// Reads web sessions. None while the web sign-in is off.
    pub web: Option<WebAuth>,
    /// The role a session needs.
    pub capability: String,
    /// The settings that bound load.
    pub limits: AdminLimits,
    /// The longest instructions accepted.
    pub max_instructions_chars: usize,
    /// Changes the history returns.
    pub history_limit: usize,
}

/// Applies one recorded change to the running agent.
pub fn apply(agent: &Agent, change: &AdminChange) {
    match change.kind {
        ChangeKind::Tool => {
            if let Some(on) = change.value.as_bool()
                && !agent.tools().switch(&change.key, on)
            {
                tracing::warn!(tool = %change.key, "an admin change names a tool this engine does not have");
            }
        }
        ChangeKind::Instructions => {
            agent.set_instructions(change.value.as_str().map(str::to_owned));
        }
    }
}

/// Applies every change in force in tenant, at boot.
pub async fn restore(
    store: &dyn AdminStore,
    tenant: &str,
    agent: &Agent,
) -> Result<usize, StoreError> {
    let current = store.current(tenant).await?;
    for change in &current {
        apply(agent, change);
    }
    Ok(current.len())
}

impl AdminState {
    /// Who is acting, or the response that refuses the call.
    fn actor(&self, headers: &HeaderMap) -> Result<String, Response> {
        match caller(headers, &self.service_token, self.web.as_ref())? {
            Caller::Service => Ok("service".to_owned()),
            Caller::Web(session) if session.roles.contains(&self.capability) => Ok(session.user_id),
            Caller::Web(session) => {
                tracing::warn!(user = %session.user_id, "admin route refused to a session without the write role");
                Err((StatusCode::FORBIDDEN, "this needs the server manager role").into_response())
            }
        }
    }

    /// The store, or the 501 that says there is none.
    fn store(&self) -> Result<&Arc<dyn AdminStore>, Response> {
        self.store.as_ref().ok_or_else(|| {
            (
                StatusCode::NOT_IMPLEMENTED,
                "this engine keeps no admin store",
            )
                .into_response()
        })
    }

    /// Records a change, applies it, and answers with it.
    async fn commit(&self, kind: ChangeKind, key: &str, value: Value, actor: String) -> Response {
        let store = match self.store() {
            Ok(store) => store,
            Err(refused) => return refused,
        };
        let new = NewChange {
            tenant_id: self.tenant.clone(),
            kind,
            key: key.to_owned(),
            value,
            actor,
        };
        match store.record(new).await {
            Ok(change) => {
                apply(&self.agent, &change);
                tracing::info!(id = change.id, kind = kind.as_str(), key, actor = %change.actor, "admin change");
                Json(change).into_response()
            }
            Err(e) => unavailable(&e),
        }
    }

    /// The instructions as GET shows them.
    fn instructions_state(&self) -> InstructionsState {
        let text = self.agent.instructions();
        let default_text = self.agent.default_instructions();
        InstructionsState {
            changed: text != default_text,
            text: text.to_string(),
            default_text: default_text.to_string(),
            max_chars: self.max_instructions_chars,
        }
    }
}

fn unavailable(e: &StoreError) -> Response {
    tracing::error!(error = %e, "admin store failed");
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "the admin store is unavailable",
    )
        .into_response()
}

/// GET /admin/tools: every registered tool and whether it is offered.
pub async fn tools(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    if let Err(refused) = state.actor(&headers) {
        return refused;
    }
    let set = state.agent.tools();
    let listed: Vec<ToolState> = set
        .all()
        .map(|t| {
            let d = t.definition();
            ToolState {
                on: set.is_on(&d.name),
                name: d.name,
                description: d.description,
                private: d.private,
            }
        })
        .collect();
    Json(listed).into_response()
}

/// POST /admin/tools/{name}: offers the tool, or stops offering it, from the next turn.
pub async fn switch_tool(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(req): Json<ToolSwitch>,
) -> Response {
    let actor = match state.actor(&headers) {
        Ok(actor) => actor,
        Err(refused) => return refused,
    };
    if state
        .agent
        .tools()
        .all()
        .all(|t| t.definition().name != name)
    {
        return (StatusCode::NOT_FOUND, "no tool by that name").into_response();
    }
    state
        .commit(ChangeKind::Tool, &name, Value::Bool(req.on), actor)
        .await
}

/// GET /admin/instructions: the instructions in force and the configured ones.
pub async fn instructions(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    if let Err(refused) = state.actor(&headers) {
        return refused;
    }
    Json(state.instructions_state()).into_response()
}

/// PUT /admin/instructions: replaces the instructions, or returns to the configured ones with null.
pub async fn set_instructions(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Json(req): Json<InstructionsUpdate>,
) -> Response {
    let actor = match state.actor(&headers) {
        Ok(actor) => actor,
        Err(refused) => return refused,
    };
    let value = match req.text {
        None => Value::Null,
        Some(text) if text.trim().is_empty() => {
            return (StatusCode::BAD_REQUEST, "the instructions are empty").into_response();
        }
        Some(text) if text.chars().count() > state.max_instructions_chars => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "the instructions are over {} characters",
                    state.max_instructions_chars
                ),
            )
                .into_response();
        }
        Some(text) => Value::String(text),
    };
    state
        .commit(ChangeKind::Instructions, INSTRUCTIONS_KEY, value, actor)
        .await
}

/// GET /admin/history: the latest changes, newest first.
pub async fn history(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    if let Err(refused) = state.actor(&headers) {
        return refused;
    }
    let store = match state.store() {
        Ok(store) => store,
        Err(refused) => return refused,
    };
    match store.history(&state.tenant, state.history_limit).await {
        Ok(changes) => Json(changes).into_response(),
        Err(e) => unavailable(&e),
    }
}

/// POST /admin/rollback/{id}: records the value of an earlier change again and applies it.
pub async fn rollback(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Response {
    let actor = match state.actor(&headers) {
        Ok(actor) => actor,
        Err(refused) => return refused,
    };
    let store = match state.store() {
        Ok(store) => store,
        Err(refused) => return refused,
    };
    match store.change(&state.tenant, id).await {
        Ok(Some(earlier)) => {
            state
                .commit(earlier.kind, &earlier.key, earlier.value, actor)
                .await
        }
        Ok(None) => (StatusCode::NOT_FOUND, "no change by that id").into_response(),
        Err(e) => unavailable(&e),
    }
}

/// GET /admin/limits: the settings that bound load, read only.
pub async fn limits(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    if let Err(refused) = state.actor(&headers) {
        return refused;
    }
    Json(state.limits.clone()).into_response()
}

/// GET /admin/approvals: actions waiting on a student, without their arguments.
pub async fn approvals(State(state): State<AdminState>, headers: HeaderMap) -> Response {
    if let Err(refused) = state.actor(&headers) {
        return refused;
    }
    let store = match state.store() {
        Ok(store) => store,
        Err(refused) => return refused,
    };
    match store.held(&state.tenant).await {
        Ok(held) => Json(held).into_response(),
        Err(e) => unavailable(&e),
    }
}

/// POST /admin/approvals/{token}/deny: denies a held action so it can no longer run.
pub async fn deny(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> Response {
    let actor = match state.actor(&headers) {
        Ok(actor) => actor,
        Err(refused) => return refused,
    };
    let store = match state.store() {
        Ok(store) => store,
        Err(refused) => return refused,
    };
    match store.deny(&state.tenant, &token).await {
        Ok(true) => {
            tracing::info!(%token, %actor, "held action denied by a moderator");
            Json(json!({ "denied": true })).into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, "that action is not waiting").into_response(),
        Err(e) => unavailable(&e),
    }
}
