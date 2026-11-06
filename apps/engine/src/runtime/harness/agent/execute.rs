//! Authorizing the capabilities a step asked for, and running the ones allowed.

use std::time::{Duration, Instant};

use futures::future::join_all;
use tracing::Instrument;
use tracing::field::Empty;

use super::{Agent, StepOutcome, ms};
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::{Message, ToolCall};
use crate::core::types::knowledge::evidence::Citation;
use crate::core::types::model::ModelError;
use crate::core::types::safety::policy::{
    ConfirmationRequest, Decision, PendingAction, ProposedAction,
};
use crate::core::types::safety::redact::{masked, redact, redact_text, truncate};
use crate::core::types::tools::{RiskClass, ToolError, ToolRun};
use crate::core::types::trace::{RunStatus, TraceEvent};
use crate::runtime::harness::agent::run::Run;

/// The workspace session a conversation's tool results are written to, one per conversation.
pub(super) fn workspace_session(ctx: &RequestContext) -> String {
    format!("turn{}", ctx.conversation_id.simple())
}

/// The head of content, and where the whole of it now is.
fn handle(content: &str, path: &str, session: &str, head: usize) -> String {
    let shown: String = content.chars().take(head).collect();
    let lines = content.lines().count();
    let bytes = content.len();
    format!(
        "{shown}\n\n[{bytes} bytes, {lines} lines. Only the first {head} characters are above. \
         The whole result is in the sandbox workspace at {path}. Counting, listing every item, \
         or finding one entry needs all of it: read it with run_sandbox, session {session}, for \
         example grep -c SUBJECT {path}, grep -i SUBJECT {path}, or sed -n 1,40p {path}]"
    )
}

/// Why authorize_all stopped.
pub(super) enum HeldError {
    /// The caller must approve before anything runs.
    Confirm(ConfirmationRequest),
    /// The action could not be held for later approval.
    Store(String),
}

impl Agent {
    /// Runs allowed calls. Repeats are refused and reported; an all-repeat step stalls the run.
    pub(super) async fn execute(
        &self,
        run: &mut Run<'_>,
        runnable: Vec<ToolCall>,
    ) -> Result<StepOutcome, ModelError> {
        let ctx = run.ctx;
        // A call already made with identical arguments is answered with a notice, not run.
        let mut fresh = Vec::with_capacity(runnable.len());
        let mut repeats = 0usize;
        for call in runnable {
            let key = format!("{}:{}", call.name, call.arguments);
            if run.seen_calls.insert(key) {
                fresh.push(call);
            } else {
                repeats += 1;
                run.new_turns.push(Message::tool_result(
                    &call.id,
                    &call.name,
                    "already called with these exact arguments earlier in this conversation; \
                     use that result or answer the user",
                ));
            }
        }
        if fresh.is_empty() && repeats > 0 {
            if run.force_answer {
                return Ok(StepOutcome::Stop(
                    RunStatus::Stalled,
                    "I kept repeating the same steps without getting further. Try rephrasing, or \
                     ask for something more specific."
                        .into(),
                    None,
                ));
            }
            run.force_answer = true;
            return Ok(StepOutcome::Continue);
        }

        // Calls run in parallel unless one of them is sequential.
        let step = run.steps;
        let stateful = fresh.iter().any(|call| {
            self.deps
                .tools
                .get(&call.name)
                .is_some_and(|t| t.definition().sequential)
        });
        let results: Vec<(Result<String, ToolError>, Vec<Citation>)> = if stateful {
            let mut out = Vec::with_capacity(fresh.len());
            for call in &fresh {
                out.push(self.run_tool(ctx, step, call, run.gather_room()).await);
            }
            out
        } else {
            let room = run.gather_room();
            join_all(
                fresh
                    .iter()
                    .map(|call| self.run_tool(ctx, step, call, room)),
            )
            .await
        };
        for (index, (call, (result, found))) in fresh.iter().zip(results).enumerate() {
            run.cite(found);
            run.tool_runs.push(ToolRun {
                tool: call.name.clone(),
                ok: result.is_ok(),
            });
            let content = result.unwrap_or_else(|error| format!("error: {error}"));
            let content = self.offloaded(ctx, step, index, &call.name, content).await;
            run.new_turns
                .push(Message::tool_result(&call.id, &call.name, content));
        }
        Ok(StepOutcome::Continue)
    }

    /// A result too long to carry, written to the workspace and replaced by its head and a path.
    async fn offloaded(
        &self,
        ctx: &RequestContext,
        step: u32,
        index: usize,
        tool: &str,
        content: String,
    ) -> String {
        let limit = self.cfg.tool_result_to_file_chars;
        if limit == 0 || content.chars().count() <= limit {
            return content;
        }
        let Some(sandbox) = &self.deps.sandbox else {
            return content;
        };
        let session = workspace_session(ctx);
        let name = format!("{tool}-{step}-{index}.txt");
        match sandbox.put(ctx, &session, &name, content.as_bytes()).await {
            Ok(path) => {
                self.deps.trace.emit(
                    ctx,
                    TraceEvent::ToolResultStored {
                        step,
                        tool: tool.to_owned(),
                        path: path.clone(),
                        bytes: content.len(),
                    },
                );
                handle(&content, &path, &session, limit)
            }
            Err(error) => {
                tracing::warn!(%error, tool, "could not offload the tool result; it is carried whole");
                content
            }
        }
    }

    /// Runs policy over every call first. Denials feed back as tool results; confirmation stops it.
    pub(super) async fn authorize_all(
        &self,
        run: &mut Run<'_>,
        calls: &[ToolCall],
    ) -> Result<Vec<ToolCall>, HeldError> {
        let deps = &self.deps;
        let mut runnable = Vec::new();
        for call in calls {
            let Some(tool) = deps.tools.get(&call.name) else {
                run.new_turns.push(Message::tool_result(
                    &call.id,
                    &call.name,
                    deps.tools.unknown(&call.name),
                ));
                continue;
            };
            let definition = tool.definition();
            let action = ProposedAction {
                tool: call.name.clone(),
                risk: definition.risk,
                arguments: call.arguments.clone(),
                private: definition.private,
            };
            let decision = deps.policy.authorize(run.ctx, &action).await;
            deps.trace.emit(
                run.ctx,
                TraceEvent::PolicyDecision {
                    step: run.steps,
                    tool: call.name.clone(),
                    decision: decision.clone(),
                },
            );
            match decision {
                Decision::Allow => runnable.push(call.clone()),
                Decision::Deny { reason } => {
                    run.new_turns.push(Message::tool_result(
                        &call.id,
                        &call.name,
                        format!("denied: {reason}"),
                    ));
                }
                Decision::Confirm(request) => {
                    let pending = PendingAction {
                        call_id: call.id.clone(),
                        action,
                    };
                    if let Some(store) = &deps.confirmations {
                        store
                            .hold(
                                run.ctx,
                                request.token,
                                &pending,
                                &request.payload_hash,
                                self.cfg.confirmation_ttl,
                            )
                            .await
                            .map_err(|error| HeldError::Store(error.to_string()))?;
                    }
                    return Err(HeldError::Confirm(request));
                }
            }
        }
        Ok(runnable)
    }

    /// Runs one tool call under its timeout and within room, tracing it, and returns its text and citations.
    pub(super) async fn run_tool(
        &self,
        ctx: &RequestContext,
        step: u32,
        call: &ToolCall,
        room: Duration,
    ) -> (Result<String, ToolError>, Vec<Citation>) {
        let deps = &self.deps;
        let Some(tool) = deps.tools.get(&call.name) else {
            let missing = ToolError::Failed(format!("no tool named {}", call.name));
            return (Err(missing), Vec::new());
        };
        let arguments = redact(&call.arguments);
        deps.trace.emit(
            ctx,
            TraceEvent::ToolStarted {
                step,
                call_id: call.id.clone(),
                tool: call.name.clone(),
                arguments: arguments.clone(),
            },
        );
        let started = Instant::now();
        // A declared tool timeout replaces the default; the time left for gathering caps both.
        let declared = tool
            .definition()
            .timeout_secs
            .map_or(self.cfg.tool_timeout, Duration::from_secs);
        let limit = declared.min(room);
        let risk = tool.definition().risk;
        let span = tracing::info_span!(
            "tool",
            "gen_ai.operation.name" = "execute_tool",
            "gen_ai.tool.name" = %call.name,
            "gen_ai.tool.call.id" = %call.id,
            "gen_ai.tool.call.arguments" = %arguments,
            "gen_ai.tool.call.result" = Empty,
            // OpenInference, read by the Phoenix trace UI.
            "openinference.span.kind" = "TOOL",
            "tool.name" = %call.name,
            "tool.call_id" = %call.id,
            "input.value" = %arguments,
            "output.value" = Empty,
            "session.id" = %ctx.conversation_id,
            "user.id" = %ctx.user_id,
            "sparky.step" = step,
            "sparky.risk" = ?risk,
            "otel.status_code" = Empty,
            "otel.status_message" = Empty,
        );
        let result = tokio::select! {
            () = ctx.cancel.cancelled() => Err(ToolError::Cancelled),
            outcome = tokio::time::timeout(limit, tool.call(ctx, call.arguments.clone()).instrument(span.clone())) => {
                outcome.unwrap_or(Err(ToolError::Timeout))
            }
        };
        let (content, traced, found) = match result {
            Ok(output) => {
                // Tool output is redacted before it reaches the trace; an authenticated result is masked.
                let traced = Ok(traced_result(risk, &output.content, 2_000));
                (Ok(output.content), traced, output.sources)
            }
            Err(error) => {
                let message = error.to_string();
                (Err(error), Err(message), Vec::new())
            }
        };
        deps.trace.emit(
            ctx,
            TraceEvent::ToolCall {
                step,
                call_id: call.id.clone(),
                tool: call.name.clone(),
                arguments,
                result: traced,
                duration_ms: ms(started),
            },
        );
        match &content {
            Ok(text) => {
                let shown = traced_result(risk, text, 4_000);
                span.record("gen_ai.tool.call.result", shown.as_str());
                span.record("output.value", shown.as_str());
                span.record("otel.status_code", "OK");
            }
            Err(error) => {
                let shown = format!("error: {error}");
                span.record("gen_ai.tool.call.result", shown.as_str());
                span.record("output.value", shown.as_str());
                span.record("otel.status_code", "ERROR");
                span.record("otel.status_message", error.to_string().as_str());
            }
        }
        (content, found)
    }
}

/// What a trace or span records for a tool result: masked when authenticated, else redacted and cut.
fn traced_result(risk: RiskClass, content: &str, max: usize) -> String {
    if risk == RiskClass::ReadAuthenticated {
        masked(content)
    } else {
        truncate(&redact_text(content), max)
    }
}
