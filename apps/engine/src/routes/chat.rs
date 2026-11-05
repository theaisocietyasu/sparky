//! POST /chat: one message, one answer with citations. /chat/stream: same turn, step events too.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use futures::{StreamExt, stream};
use opentelemetry::trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState};
use secrecy::SecretString;
use serde::Serialize;
use serde_json::json;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::oneshot;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::Instrument;
use tracing::field::Empty;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::types::agent::Answer;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::conversation::file::FileAttachment;
use crate::core::types::conversation::image::Attachment;
use crate::core::types::http::chat::{ChatRequest, ChatResponse, ConfirmRequest};
use crate::core::types::http::web::Session;
use crate::core::types::store::StoreError;
use crate::core::types::trace::progress::Progress;
use crate::routes::auth::{Caller, caller, too_many};
use crate::routes::failure::{Failure, NO_SUCH_CONVERSATION};
use crate::routes::rate_limit::RateLimiter;
use crate::routes::session::{WEB_CHANNEL, WebAuth};
use crate::runtime::harness::agent::Agent;
use uuid::Uuid;

/// What the chat route needs.
#[derive(Clone)]
pub struct ChatState {
    /// The agent.
    pub agent: Agent,
    /// To create the conversation row before the run.
    pub conversations: Option<Arc<dyn ConversationStore>>,
    /// Where actions wait for approval.
    pub confirmations: Option<Arc<dyn ConfirmationStore>>,
    /// Per-request wall-clock budget.
    pub request_budget: Duration,
    /// Tenant used when the client sends none (single-guild deployments).
    pub default_tenant: String,
    /// Images of one message sent to the model.
    pub max_images: usize,
    /// Files of one message opened in the sandbox.
    pub max_files: usize,
    /// One permit per turn allowed to run at once.
    pub turns: Arc<tokio::sync::Semaphore>,
    /// Longest a turn waits for a permit.
    pub turn_wait: Duration,
    /// The question an empty message carrying attachments stands for.
    pub attachments_only_input: String,
    /// Largest file opened in the sandbox, in bytes.
    pub max_file_bytes: u64,
    /// Bearer token every caller must present.
    pub service_token: SecretString,
    /// Per-user request limit.
    pub rate_limit: RateLimiter,
    /// Reads web sessions. None while the web sign-in is off.
    pub web: Option<WebAuth>,
    /// What the web conversation list and view show.
    pub web_lists: WebLists,
}

/// Bounds of the web conversation list and view.
#[derive(Debug, Clone, Copy)]
pub struct WebLists {
    /// Conversations listed.
    pub conversations: usize,
    /// Messages one conversation shows.
    pub turns: usize,
    /// Characters a title keeps.
    pub title_chars: usize,
}

/// Rewrites a turn to run as the signed-in student in their private web chat.
fn as_web(req: &mut ChatRequest, session: Session) {
    req.user_id = session.user_id;
    req.roles = session.roles;
    req.tenant_id = None;
    req.visibility = Visibility::Private;
    WEB_CHANNEL.clone_into(&mut req.channel_id);
}

/// Parses a W3C traceparent header into a remote parent context.
pub fn parse_traceparent(value: &str) -> Option<opentelemetry::Context> {
    let mut parts = value.trim().split('-');
    let (_version, trace_id, span_id, flags) =
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let trace_id = TraceId::from_hex(trace_id).ok()?;
    let span_id = SpanId::from_hex(span_id).ok()?;
    let flags = TraceFlags::new(u8::from_str_radix(flags, 16).ok()?);
    let remote = SpanContext::new(trace_id, span_id, flags, true, TraceState::default());
    remote
        .is_valid()
        .then(|| opentelemetry::Context::new().with_remote_span_context(remote))
}

/// The span a request runs under, carrying attributes both trace UIs read. Records answer, outcome.
macro_rules! route_span {
    ($name:literal, $req:expr) => {
        tracing::info_span!(
            $name,
            "openinference.span.kind" = "CHAIN",
            "input.value" = %$req.message,
            "output.value" = Empty,
            "user.id" = %$req.user_id,
            "session.id" = Empty,
            "otel.status_code" = Empty,
            "otel.status_message" = Empty,
        )
    };
}

/// Parents span to the traceparent header of the caller, when it carries a valid one.
fn adopt_parent(span: &tracing::Span, headers: &HeaderMap) {
    let Some(value) = headers.get("traceparent") else {
        return;
    };
    let Some(parent) = value.to_str().ok().and_then(parse_traceparent) else {
        tracing::debug!("traceparent unreadable; ignored");
        return;
    };
    if let Err(e) = span.set_parent(parent) {
        tracing::debug!(error = %e, "traceparent ignored");
    }
}

/// The response body for a finished run.
fn chat_response(request_id: Uuid, conversation_id: Uuid, answer: Answer) -> ChatResponse {
    ChatResponse {
        request_id,
        conversation_id,
        citations: answer.citations(),
        text: answer.text,
        confirmation: answer.confirmation,
        status: answer.status,
        steps: answer.steps,
        tools: answer.tool_runs,
        memories: answer.memories,
        tokens: answer.usage.total(),
        cost_usd: answer.cost_usd,
    }
}

/// Records how a turn ended on the span it ran under.
fn record_outcome(span: &tracing::Span, outcome: &Result<ChatResponse, Failure>) {
    match outcome {
        Ok(answer) => {
            span.record("session.id", answer.conversation_id.to_string().as_str());
            span.record(
                "output.value",
                answer.text.chars().take(4_000).collect::<String>().as_str(),
            );
            span.record("otel.status_code", "OK");
        }
        Err(failure) => {
            span.record("otel.status_code", "ERROR");
            span.record("otel.status_message", failure.body().error.as_str());
        }
    }
}

/// Handles one chat turn. Runs under an http.chat span parented to the traceparent of the caller.
pub async fn chat(
    State(state): State<ChatState>,
    headers: HeaderMap,
    Json(mut req): Json<ChatRequest>,
) -> Response {
    match caller(&headers, &state.service_token, state.web.as_ref()) {
        Ok(Caller::Service) => {}
        Ok(Caller::Web(session)) => as_web(&mut req, session),
        Err(refused) => return refused,
    }
    if !state.rate_limit.allow(&req.user_id) {
        return too_many(&req.user_id);
    }
    let span = route_span!("http.chat", req);
    adopt_parent(&span, &headers);
    let outcome = run_turn(state, req, None).instrument(span.clone()).await;
    record_outcome(&span, &outcome);
    match outcome {
        Ok(answer) => Json(answer).into_response(),
        Err(failure) => failure.into_response(),
    }
}

/// The same turn as chat, reported live: progress events, then one answer event, then done.
pub async fn stream(
    State(state): State<ChatState>,
    headers: HeaderMap,
    Json(mut req): Json<ChatRequest>,
) -> Response {
    match caller(&headers, &state.service_token, state.web.as_ref()) {
        Ok(Caller::Service) => {}
        Ok(Caller::Web(session)) => as_web(&mut req, session),
        Err(refused) => return refused,
    }
    if !state.rate_limit.allow(&req.user_id) {
        return too_many(&req.user_id);
    }
    let span = route_span!("http.chat.stream", req);
    adopt_parent(&span, &headers);

    let (progress_tx, progress_rx) = mpsc::unbounded_channel();
    let (answer_tx, answer_rx) = oneshot::channel();
    let outcome_span = span.clone();
    tokio::spawn(
        async move {
            let outcome = run_turn(state, req, Some(progress_tx)).await;
            record_outcome(&outcome_span, &outcome);
            // The receiver is gone only when the client has disconnected.
            let _ = answer_tx.send(outcome);
        }
        .instrument(span),
    );

    let progress = UnboundedReceiverStream::new(progress_rx).map(|p| sse("progress", &p));
    let tail = stream::once(async move {
        match answer_rx.await {
            Ok(Ok(answer)) => sse("answer", &answer),
            Ok(Err(failure)) => sse("error", failure.body()),
            Err(e) => {
                tracing::error!(error = %e, "the turn ended without an answer");
                sse("error", &json!({ "error": e.to_string() }))
            }
        }
    })
    .chain(stream::once(async { sse("done", &json!({})) }));

    Sse::new(progress.chain(tail).map(Ok::<Event, Infallible>)).into_response()
}

/// One server-sent event. An unserialisable payload becomes a client error and is logged.
fn sse<T: Serialize>(name: &str, body: &T) -> Event {
    match serde_json::to_string(body) {
        Ok(json) => Event::default().event(name).data(json),
        Err(e) => {
            tracing::error!(error = %e, event = name, "could not serialise a stream event");
            Event::default()
                .event("error")
                .data(json!({ "error": "internal serialisation failure" }).to_string())
        }
    }
}

/// One turn, with the progress channel the caller wants events on, if any.
async fn run_turn(
    state: ChatState,
    req: ChatRequest,
    watcher: Option<UnboundedSender<Progress>>,
) -> Result<ChatResponse, Failure> {
    let mut req = req;
    if req.message.trim().is_empty() {
        if req.images.is_empty() && req.files.is_empty() {
            return Err(Failure::new(
                StatusCode::BAD_REQUEST,
                Uuid::nil(),
                "message is empty",
            ));
        }
        req.message.clone_from(&state.attachments_only_input);
    }
    // Held for the whole turn, detached streaming turns included, so together they stay bounded.
    let Ok(Ok(_turn)) =
        tokio::time::timeout(state.turn_wait, Arc::clone(&state.turns).acquire_owned()).await
    else {
        tracing::warn!("every turn slot is busy; the turn is refused");
        return Err(Failure::new(
            StatusCode::SERVICE_UNAVAILABLE,
            Uuid::nil(),
            "busy answering other questions; ask again in a minute",
        ));
    };
    let tenant = req
        .tenant_id
        .unwrap_or_else(|| state.default_tenant.clone());
    let parent = req.parent.filter(|_| req.conversation_id.is_none());
    let ctx = RequestContext::new(tenant, req.user_id, state.request_budget)
        .with_widgets(req.channel_id == WEB_CHANNEL)
        .with_roles(req.roles)
        .with_visibility(req.visibility)
        .replying_to(req.reply_to)
        .with_images(Attachment::accepted(req.images, state.max_images))
        .with_files(FileAttachment::accepted(
            req.files,
            state.max_files,
            state.max_file_bytes,
        ));
    let mut ctx = open(
        &state,
        ctx,
        req.conversation_id,
        req.continue_channel,
        &req.channel_id,
    )
    .await?;
    if let (Some(parent), Some(store)) = (parent, &state.conversations)
        && let Err(error) = store.link(&ctx, &parent).await
    {
        tracing::warn!(%error, "could not make the conversation a thread");
    }
    if let Some(tx) = watcher {
        ctx = ctx.listening_to(tx);
    }
    let id = ctx.request_id;
    state
        .agent
        .run(&ctx, &req.message)
        .await
        .map(|answer| chat_response(id, ctx.conversation_id, answer))
        .map_err(|error| Failure::from_agent(id, error))
}

/// Picks the conversation a turn continues, checked for ownership; falls back to open, then new.
async fn open(
    state: &ChatState,
    mut ctx: RequestContext,
    conversation_id: Option<Uuid>,
    continue_channel: bool,
    channel_id: &str,
) -> Result<RequestContext, Failure> {
    let Some(store) = &state.conversations else {
        if let Some(id) = conversation_id {
            ctx = ctx.with_conversation(id);
        }
        return Ok(ctx);
    };
    if let Some(id) = conversation_id {
        ctx = ctx.with_conversation(id);
    } else if continue_channel {
        match store.latest(&ctx, channel_id).await {
            Ok(Some(id)) => ctx = ctx.with_conversation(id),
            Ok(None) => {}
            Err(e) => {
                tracing::error!(error = %e, "conversation store unavailable");
                return Err(Failure::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    ctx.request_id,
                    "conversation store unavailable",
                ));
            }
        }
    }
    match store.ensure(&ctx, channel_id).await {
        Ok(()) => Ok(ctx),
        Err(StoreError::NotOwned) => {
            tracing::warn!(user = %ctx.user_id, "conversation held by another caller");
            Err(Failure::new(
                StatusCode::NOT_FOUND,
                ctx.request_id,
                NO_SUCH_CONVERSATION,
            ))
        }
        Err(e) => {
            tracing::error!(error = %e, "conversation store unavailable");
            Err(Failure::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ctx.request_id,
                "conversation store unavailable",
            ))
        }
    }
}

/// Answers a held action: approve runs it, deny drops it. Token is single use, tied to the caller.
pub async fn confirm(
    State(state): State<ChatState>,
    headers: HeaderMap,
    Json(mut req): Json<ConfirmRequest>,
) -> Response {
    match caller(&headers, &state.service_token, state.web.as_ref()) {
        Ok(Caller::Service) => {}
        Ok(Caller::Web(session)) => {
            req.user_id = session.user_id;
            req.tenant_id = None;
            req.visibility = Visibility::Private;
        }
        Err(refused) => return refused,
    }
    let Some(store) = &state.confirmations else {
        return (
            StatusCode::NOT_IMPLEMENTED,
            "this engine holds no confirmations",
        )
            .into_response();
    };
    let tenant = req
        .tenant_id
        .unwrap_or_else(|| state.default_tenant.clone());
    let ctx = RequestContext::new(tenant, req.user_id, state.request_budget)
        .with_conversation(req.conversation_id)
        .with_visibility(req.visibility);
    if let Some(conversations) = &state.conversations {
        match conversations.owns(&ctx).await {
            Ok(true) => {}
            Ok(false) => {
                return Failure::new(StatusCode::NOT_FOUND, ctx.request_id, NO_SUCH_CONVERSATION)
                    .into_response();
            }
            Err(e) => {
                tracing::error!(error = %e, "conversation store failed");
                return Failure::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    ctx.request_id,
                    "a store is unavailable",
                )
                .into_response();
            }
        }
    }

    let claimed = match store.claim(&ctx, req.token, req.approve).await {
        Ok(claimed) => claimed,
        Err(e) => {
            tracing::error!(error = %e, "confirmation store failed");
            return Failure::new(
                StatusCode::SERVICE_UNAVAILABLE,
                ctx.request_id,
                "a store is unavailable",
            )
            .into_response();
        }
    };
    let Some(pending) = claimed else {
        // Expired, already answered, or held for another caller.
        return Failure::new(
            StatusCode::CONFLICT,
            ctx.request_id,
            "that approval is no longer open",
        )
        .into_response();
    };
    if !req.approve {
        return Json(json!({
            "request_id": ctx.request_id,
            "conversation_id": ctx.conversation_id,
            "status": "denied",
            "text": "Fine, I will not do that.",
        }))
        .into_response();
    }
    match state.agent.resume(&ctx, pending).await {
        Ok(answer) => {
            Json(chat_response(ctx.request_id, ctx.conversation_id, answer)).into_response()
        }
        Err(error) => Failure::from_agent(ctx.request_id, error).into_response(),
    }
}
