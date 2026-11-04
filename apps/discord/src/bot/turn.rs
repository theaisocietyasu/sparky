//! One streamed turn, shown as a single message edited in place.

use std::time::Instant;

use serenity::all::{Context, MessageId};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::Instrument;

use super::destination::Destination;
use super::{Handler, error_kind};
use crate::core::types::{AnalyticsEvent, ChatRequest, ChatResponse, EngineError, Update};
use crate::render::card::{self, Pacer, Steps};
use crate::render::components::{self, ButtonSpec};
use crate::render::reply;

/// An analytics event for the asker of req, in the guild and channel of req.
fn turn_event(name: &'static str, req: &ChatRequest) -> AnalyticsEvent {
    AnalyticsEvent::new(name, &req.user_id)
        .with("guild_id", req.tenant_id.as_str())
        .with("channel_id", req.channel_id.as_str())
}

impl Handler {
    /// Runs req against the engine, shows the turn in dest. span gets the session id and answer.
    pub(super) async fn converse(
        &self,
        ctx: &Context,
        dest: &Destination,
        req: &ChatRequest,
        span: tracing::Span,
        place: &'static str,
    ) {
        let started = Instant::now();
        let limit = self.max_message_chars;
        let posted = dest
            .send(
                &ctx.http,
                card::thinking(&[], None, limit, 0),
                Vec::new(),
                true,
            )
            .await;
        let card_id = match posted {
            Ok(message) => Some(message.id),
            Err(e) => {
                tracing::warn!(error = %e, "could not post the turn message");
                None
            }
        };
        let mut steps = Steps::default();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let streaming = self.engine.chat_stream(req, tx).instrument(span.clone());
        let outcome = tokio::join!(
            streaming,
            self.watch(ctx, dest, card_id, &mut rx, &mut steps)
        )
        .1;
        let outcome = outcome.unwrap_or_else(|| {
            tracing::error!(user = %req.user_id, "stream ended with no outcome");
            Err(EngineError::Transport("no answer".into()))
        });
        match outcome {
            Ok(resp) => {
                let conversation = resp.conversation_id.to_string();
                span.record("session.id", conversation.as_str());
                let shown = resp.text.chars().take(2_000).collect::<String>();
                span.record("sparky.output", shown.as_str());
                span.record("output.value", shown.as_str());
                span.record("otel.status_code", "OK");
                let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                self.record(
                    turn_event("discord_answer", req)
                        .with("$session_id", conversation.as_str())
                        .with("conversation_id", conversation)
                        .with("status", resp.status.as_str())
                        .with("latency_ms", latency_ms)
                        .with("chars", resp.text.chars().count())
                        .with("place", place),
                );
                tracing::info!(
                    request_id = %resp.request_id,
                    status = %resp.status,
                    user = %req.user_id,
                    "answered"
                );
                let messages = card::answer(&steps.lines(), &resp, limit);
                let mut rows = components::rows_for(&resp);
                if resp.confirmation.is_none()
                    && let Ok(user) = req.user_id.parse::<u64>()
                {
                    rows.push(components::feedback_row(resp.request_id, user));
                }
                self.show(ctx, dest, card_id, messages, &rows).await;
            }
            Err(e) => {
                span.record("otel.status_code", "ERROR");
                span.record("otel.status_message", e.to_string().as_str());
                tracing::error!(error = %e, user = %req.user_id, "engine call failed");
                self.record(
                    turn_event("discord_error", req)
                        .with("stage", "chat")
                        .with("kind", error_kind(&e))
                        .with("place", place),
                );
                let text = card::failed(&steps.lines(), &reply::failure(&e), limit);
                self.show(ctx, dest, card_id, vec![text], &[]).await;
            }
        }
    }

    /// Puts the final render on the card; if it cannot be edited, posts anew, deletes stale card.
    async fn show(
        &self,
        ctx: &Context,
        dest: &Destination,
        card_id: Option<MessageId>,
        messages: Vec<String>,
        rows: &[Vec<ButtonSpec>],
    ) {
        let mut messages = messages.into_iter();
        let Some(first) = messages.next() else {
            return;
        };
        let edited = match card_id {
            Some(id) => dest
                .edit(
                    &ctx.http,
                    id,
                    first.clone(),
                    Some(components::to_action_rows(rows)),
                )
                .await
                .map(|_| ()),
            None => Err(serenity::Error::Other("no turn message")),
        };
        if let Err(edit_error) = edited {
            let sent = dest
                .send(&ctx.http, first, components::to_action_rows(rows), true)
                .await;
            match (sent, card_id) {
                (Ok(_), Some(stale)) => {
                    if let Err(e) = dest.delete(&ctx.http, stale).await {
                        tracing::warn!(error = %e, "could not delete the stale turn message");
                    }
                }
                (Ok(_), None) => {}
                (Err(e), _) => {
                    tracing::error!(
                        edit_error = %edit_error,
                        send_error = %e,
                        "the answer could not be shown"
                    );
                }
            }
        }
        for more in messages {
            if let Err(e) = dest.send(&ctx.http, more, Vec::new(), false).await {
                tracing::warn!(error = %e, "continuation message failed");
            }
        }
    }

    /// Collects steps while the turn runs and edits the card at most once per edit_every.
    async fn watch(
        &self,
        ctx: &Context,
        dest: &Destination,
        card_id: Option<MessageId>,
        rx: &mut UnboundedReceiver<Update>,
        steps: &mut Steps,
    ) -> Option<Result<ChatResponse, EngineError>> {
        let mut outcome = None;
        let mut pacer = Pacer::new(self.edit_every, Instant::now());
        // The spinner frame advances once per progress edit.
        let mut frame = 0usize;
        loop {
            let wait = pacer.wait(Instant::now()).filter(|_| outcome.is_none());
            tokio::select! {
                update = rx.recv() => match update {
                    None => break,
                    Some(Update::Progress(p)) => pacer.mark(steps.apply(&p)),
                    Some(Update::Answer(answer)) => outcome = Some(Ok(*answer)),
                    Some(Update::Failed(e)) => outcome = Some(Err(e)),
                },
                () = tokio::time::sleep(wait.unwrap_or_default()), if wait.is_some() => {
                    if let Some(id) = card_id {
                        frame = frame.wrapping_add(1);
                        let text = card::thinking(
                            &steps.lines(),
                            steps.draft(),
                            self.max_message_chars,
                            frame,
                        );
                        if let Err(e) = dest.edit(&ctx.http, id, text, None).await {
                            tracing::warn!(error = %e, "progress edit failed");
                        }
                    }
                    pacer.flushed(Instant::now());
                }
            }
        }
        outcome
    }
}
