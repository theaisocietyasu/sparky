//! One model call: thinking decision, request, span, retries, streamed relay, thought split out.

pub mod draft;
mod relay;
mod retry;
mod spans;
pub mod thinking;
pub mod thought;

use std::collections::HashSet;
use std::time::Instant;

use tracing::Instrument;
use tracing::field::Empty;

pub use self::retry::backoff;
pub(super) use self::spans::record_reply;
use super::{Agent, ms};
use crate::core::types::agent::context::RequestContext;
use crate::core::types::agent::thinking::{ThinkingChoice, ThinkingReason};
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::model::{FinishReason, ModelError, ModelRequest, ModelResponse};
use crate::core::types::safety::redact::{json, masked, truncate};
use crate::core::types::tools::RiskClass;
use crate::core::types::trace::TraceEvent;
use crate::runtime::harness::agent::run::Run;

/// Takes the thought out of the content of a response and returns it.
fn lift_thought(response: &mut ModelResponse) -> Option<String> {
    let (thought, visible) = thought::split(&response.reasoning, &response.content);
    response.content = visible;
    thought
}

impl Agent {
    /// Whether the next model call of run thinks.
    pub(super) fn thinking(&self, run: &Run<'_>) -> ThinkingChoice {
        thinking::decide(
            &self.cfg.thinking,
            &thinking::StepSignals {
                answer_only: run.force_answer,
                tool_results: run.new_turns.iter().any(|m| m.role == Role::Tool),
                tool_failed: run.tool_runs.last().is_some_and(|t| !t.ok)
                    || run
                        .new_turns
                        .last()
                        .is_some_and(|m| m.role == Role::Tool && m.content.trim().is_empty()),
                input: run.input,
            },
        )
    }

    /// Calls the model, splits thought from answer. Retries without thinking if thought, no answer.
    pub(super) async fn answer_with_thinking(
        &self,
        run: &mut Run<'_>,
        messages: &[Message],
        thinking: ThinkingChoice,
    ) -> Result<(ModelResponse, Option<String>), ModelError> {
        let (ctx, step, answer_only) = (run.ctx, run.steps, run.force_answer);
        let mut response = self
            .call_model(ctx, step, messages, answer_only, thinking)
            .await?;
        run.usage.add(response.usage);
        let mut thought = lift_thought(&mut response);
        let spent = response.content.is_empty()
            && response.tool_calls.is_empty()
            && (thought.is_some() || response.finish_reason == FinishReason::Length);
        if thinking.on && spent && self.cfg.thinking.retry_without {
            tracing::info!(step, "thinking left no answer; calling again without it");
            let plain = ThinkingChoice {
                on: false,
                reason: ThinkingReason::Retry,
            };
            response = self
                .call_model(ctx, step, messages, answer_only, plain)
                .await?;
            run.usage.add(response.usage);
            thought = lift_thought(&mut response);
        }
        Ok((response, thought))
    }

    /// The prompt as a span records it: a tool turn answering an authenticated call is masked.
    fn traced_messages(&self, messages: &[Message]) -> Vec<Message> {
        let authenticated: HashSet<&str> = messages
            .iter()
            .flat_map(|m| &m.tool_calls)
            .filter(|call| {
                self.deps
                    .tools
                    .get(&call.name)
                    .is_some_and(|tool| tool.definition().risk == RiskClass::ReadAuthenticated)
            })
            .map(|call| call.id.as_str())
            .collect();
        messages
            .iter()
            .map(|m| {
                let answers_authenticated = m.role == Role::Tool
                    && m.tool_call_id
                        .as_deref()
                        .is_some_and(|id| authenticated.contains(id));
                if answers_authenticated {
                    Message {
                        content: masked(&m.content),
                        ..m.clone()
                    }
                } else {
                    m.clone()
                }
            })
            .collect()
    }

    /// Span of one model call: full prompt and reply as JSON, under gen_ai and OpenInference names.
    fn model_span(
        &self,
        ctx: &RequestContext,
        step: u32,
        attempt: u32,
        request: &ModelRequest,
        thinking: ThinkingReason,
    ) -> tracing::Span {
        let limit = self.cfg.max_span_value_chars;
        let prompt = truncate(&json(&self.traced_messages(&request.messages)), limit);
        tracing::info_span!(
            "llm",
            "gen_ai.operation.name" = "chat",
            "gen_ai.provider.name" = %self.cfg.provider_name,
            "gen_ai.request.model" = %self.cfg.model_name,
            "gen_ai.response.model" = Empty,
            "gen_ai.request.max_tokens" = request.max_tokens,
            "gen_ai.request.temperature" = f64::from(request.temperature),
            "gen_ai.usage.input_tokens" = Empty,
            "gen_ai.usage.output_tokens" = Empty,
            "gen_ai.input.messages" = %prompt,
            "gen_ai.output.messages" = Empty,
            "openinference.span.kind" = "LLM",
            "input.value" = %prompt,
            "input.mime_type" = "application/json",
            "output.value" = Empty,
            "output.mime_type" = "application/json",
            "llm.model_name" = Empty,
            "llm.token_count.prompt" = Empty,
            "llm.token_count.completion" = Empty,
            "session.id" = %ctx.conversation_id,
            "user.id" = %ctx.user_id,
            "sparky.span" = "llm",
            "sparky.tools" = %truncate(&json(&request.tools), limit),
            "sparky.step" = step,
            "sparky.attempt" = attempt,
            "sparky.thinking" = request.thinking,
            "sparky.thinking_reason" = thinking.as_str(),
            "otel.status_code" = Empty,
            "otel.status_message" = Empty,
        )
    }

    /// One model call with retries, under cancellation and the request deadline.
    async fn call_model(
        &self,
        ctx: &RequestContext,
        step: u32,
        messages: &[Message],
        answer_only: bool,
        thinking: ThinkingChoice,
    ) -> Result<ModelResponse, ModelError> {
        let deps = &self.deps;
        let mut attempt = 0u32;
        loop {
            let request = ModelRequest {
                messages: messages.to_vec(),
                tools: if answer_only {
                    Vec::new()
                } else {
                    deps.tools.definitions_for(ctx.visibility)
                },
                max_tokens: if thinking.on {
                    self.cfg.max_tokens
                } else {
                    self.cfg.max_tokens_without_thinking
                },
                temperature: self.cfg.temperature,
                thinking: thinking.on,
            };
            let started = Instant::now();
            let limit = self.cfg.max_span_value_chars;
            let span = self.model_span(ctx, step, attempt, &request, thinking.reason);
            let result = tokio::select! {
                () = ctx.cancel.cancelled() => Err(ModelError::Cancelled),
                outcome = tokio::time::timeout(ctx.remaining(), self.relay(ctx, step, request).instrument(span.clone())) => {
                    outcome.unwrap_or(Err(ModelError::Timeout))
                }
            };
            match result {
                Ok(response) => {
                    span.record("otel.status_code", "OK");
                    spans::record_reply(&span, &response, limit);
                    deps.trace.emit(
                        ctx,
                        TraceEvent::ModelCall {
                            step,
                            model: response.model.clone(),
                            finish_reason: response.finish_reason,
                            usage: response.usage,
                            duration_ms: ms(started),
                            attempt,
                        },
                    );
                    return Ok(response);
                }
                Err(error) => {
                    span.record("otel.status_code", "ERROR");
                    span.record("otel.status_message", error.to_string().as_str());
                    let retry = error.is_retryable() && attempt < self.cfg.max_model_retries;
                    deps.trace.emit(
                        ctx,
                        TraceEvent::ModelError {
                            step,
                            attempt,
                            error: error.to_string(),
                            retried: retry,
                        },
                    );
                    if !retry {
                        return Err(error);
                    }
                    attempt += 1;
                    tokio::time::sleep(backoff(
                        attempt,
                        ctx.request_id,
                        ctx.remaining(),
                        self.cfg.retry_base_ms,
                        self.cfg.retry_cap_ms,
                    ))
                    .await;
                }
            }
        }
    }
}
