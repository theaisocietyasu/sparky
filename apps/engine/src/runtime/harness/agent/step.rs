//! One loop step: assemble the prompt, call the model, check the response, run what it asked.

use super::{Agent, StepOutcome};
use crate::core::types::agent::assemble::Sections;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::Message;
use crate::core::types::model::{FinishReason, ModelError, ModelResponse};
use crate::core::types::safety::guardrail::{Stage, Verdict};
use crate::core::types::safety::redact::truncate;
use crate::core::types::tools::{REMEMBER, SEARCH_LIVE};
use crate::core::types::trace::{RunStatus, TraceEvent};
use crate::runtime::harness::agent::execute::HeldError;
use crate::runtime::harness::agent::prompt::assemble;
use crate::runtime::harness::agent::run::{Inputs, Run};

/// What the guardrail did to a response on a step.
enum Guarded {
    /// The response proceeds as written.
    Pass,
    /// The response proceeds with this text in place of the original.
    Redacted(String),
    /// The response is refused and the run stops here.
    Blocked(StepOutcome),
}

/// The outcome of a step that asked for no capabilities.
fn answered(response: &ModelResponse, content: &str, force_answer: bool) -> StepOutcome {
    let written_call = force_answer && is_call_text(content);
    if !content.trim().is_empty() && !written_call {
        return StepOutcome::Stop(RunStatus::Answered, content.to_owned(), None);
    }
    let (status, text) = if force_answer {
        (
            RunStatus::Stalled,
            "I could not turn what I found into an answer. Try rephrasing.",
        )
    } else if response.finish_reason == FinishReason::Length
        || !response.reasoning.trim().is_empty()
    {
        (
            RunStatus::Answered,
            "I ran out of room before finishing the answer.",
        )
    } else {
        (
            RunStatus::Answered,
            "The model returned nothing. Try rephrasing.",
        )
    };
    StepOutcome::Stop(status, text.to_owned(), None)
}

/// Whether text is a tool call written out as JSON rather than an answer.
fn is_call_text(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text.trim())
        .is_ok_and(|v| v.get("name").is_some() && v.get("arguments").is_some())
}

impl Agent {
    /// One model call and whatever tool calls it asks for.
    pub(super) async fn step(
        &self,
        run: &mut Run<'_>,
        inputs: &Inputs,
    ) -> Result<StepOutcome, ModelError> {
        let ctx = run.ctx;
        let messages = self.prompt_for(run, inputs);
        let thinking = self.thinking(run);
        self.deps
            .trace
            .emit(ctx, TraceEvent::ModelStarted { step: run.steps });
        let (response, thought) = self.answer_with_thinking(run, &messages, thinking).await?;
        run.new_turns.push(response.as_message());

        let thought_shown = self.report_thought(ctx, run.steps, thought, &response);

        let stage = if response.tool_calls.is_empty() {
            Stage::Answer
        } else {
            Stage::Capability
        };
        let content = match self.guarded(ctx, run.steps, stage, &response.content).await {
            Guarded::Blocked(stop) => return Ok(stop),
            Guarded::Redacted(text) => text,
            Guarded::Pass => response.content.clone(),
        };

        if response.tool_calls.is_empty() {
            // A step that showed a thought keeps it in place of the answered line.
            if !thought_shown {
                self.deps
                    .trace
                    .emit(ctx, TraceEvent::ModelAnswered { step: run.steps });
            }
            return Ok(answered(&response, &content, run.force_answer));
        }

        let runnable = match self.authorize_all(run, &response.tool_calls).await {
            Ok(calls) => calls,
            Err(HeldError::Store(error)) => {
                return Err(ModelError::Transport(format!(
                    "could not hold the action for approval: {error}"
                )));
            }
            Err(HeldError::Confirm(request)) => {
                let text = format!("Before I do that, please confirm: {}", request.summary);
                return Ok(StepOutcome::Stop(
                    RunStatus::AwaitingConfirmation,
                    text,
                    Some(request),
                ));
            }
        };

        self.execute(run, runnable).await
    }

    /// Assembles the next prompt within budget left after tool schemas, and records what fit.
    fn prompt_for(&self, run: &mut Run<'_>, inputs: &Inputs) -> Vec<Message> {
        let ctx = run.ctx;
        // The turns of this request after the input, which assembly appends itself.
        let turn = run
            .new_turns
            .get(run.appended_by_assembly..)
            .unwrap_or_default();
        let cpt = self.cfg.budget.chars_per_token;
        // Tool schemas ride along with every request and come out of the same budget.
        let tool_tokens = self.deps.tools.estimated_tokens(cpt);
        let mut budget = self.cfg.budget;
        budget.total = budget.total.saturating_sub(tool_tokens);
        // The memory rules go only where the memory tools are offered.
        let mut templates = self.prompt.templates();
        if self.deps.tools.get(REMEMBER).is_none() {
            templates.memory_rules = "";
        }
        // The discovery rule goes only where search_live is offered.
        if self.deps.tools.get(SEARCH_LIVE).is_none() {
            templates.discovery_rules = "";
        }
        let assembled = assemble::assemble(
            ctx,
            &Sections {
                system: &self.system_prompt,
                memory: &inputs.memory,
                uploads: &inputs.uploads,
                history: &inputs.history,
                turn,
                capabilities: &self.capabilities(ctx.visibility),
                input: run.input,
                date: &self.prompt.today(),
                templates,
            },
            budget,
        );
        run.memories_in_prompt = inputs
            .memory
            .iter()
            .take(assembled.memory_used)
            .map(|m| m.content.clone())
            .collect();
        self.deps.trace.emit(
            ctx,
            TraceEvent::ContextAssembled {
                step: run.steps,
                message_count: assembled.messages.len(),
                estimated_tokens: assembled.estimated_tokens + tool_tokens,
            },
        );
        let mut messages = assembled.messages;
        if run.force_answer {
            messages.push(Message::system(self.prompt.answer_only_line.clone()));
        }
        messages
    }

    /// Reports what the model was thinking on this step, and says whether anything was shown.
    fn report_thought(
        &self,
        ctx: &RequestContext,
        step: u32,
        reasoned: Option<String>,
        response: &ModelResponse,
    ) -> bool {
        let shown = reasoned.or_else(|| {
            let preamble = response.content.trim();
            (!response.tool_calls.is_empty() && !preamble.is_empty()).then(|| preamble.to_owned())
        });
        let Some(text) = shown else {
            return false;
        };
        self.deps.trace.emit(
            ctx,
            TraceEvent::ModelThought {
                step,
                text: truncate(&text, self.cfg.max_span_value_chars),
            },
        );
        true
    }

    /// Checks a response against the guardrail. A block ends the run; a redaction rewrites the text.
    async fn guarded(&self, ctx: &RequestContext, step: u32, stage: Stage, text: &str) -> Guarded {
        let Some(guardrail) = self.deps.guardrail.as_ref() else {
            return Guarded::Pass;
        };
        // An empty capability branch is not checked.
        if stage == Stage::Capability && text.trim().is_empty() {
            return Guarded::Pass;
        }
        match guardrail.check(ctx, stage, text).await {
            Verdict::Pass => Guarded::Pass,
            Verdict::Block {
                replacement,
                reason,
            } => {
                tracing::warn!(
                    stage = stage.as_str(),
                    reason,
                    "guardrail blocked a response"
                );
                self.deps.trace.emit(
                    ctx,
                    TraceEvent::GuardrailBlocked {
                        step,
                        stage,
                        reason,
                    },
                );
                Guarded::Blocked(StepOutcome::Stop(RunStatus::Blocked, replacement, None))
            }
            Verdict::Redact { text, reason } => {
                tracing::info!(
                    stage = stage.as_str(),
                    reason,
                    "guardrail redacted a response"
                );
                self.deps.trace.emit(
                    ctx,
                    TraceEvent::GuardrailRedacted {
                        step,
                        stage,
                        reason,
                    },
                );
                Guarded::Redacted(text)
            }
        }
    }
}
