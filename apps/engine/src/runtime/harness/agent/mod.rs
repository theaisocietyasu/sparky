//! The agent loop: model call, policy, tools, repeated until done, error, cancel, or a limit.

pub mod call;
mod conclude;
mod execute;
pub mod ground;
mod inputs;
pub mod prompt;
pub mod read;
mod run;
mod step;
pub mod task;
pub mod uploads;

use std::sync::Arc;
use std::time::{Duration, Instant};

use opentelemetry::trace::{TraceContextExt, TraceId};
use tracing::Instrument;
use tracing::field::Empty;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::conversation::compaction::Compactor;
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::model::ModelProvider;
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::traits::safety::guardrail::Guardrail;
use crate::core::traits::safety::policy::Policy;
use crate::core::traits::tools::files::FileSource;
use crate::core::traits::tools::sandbox::Sandbox;
use crate::core::traits::trace::TraceSink;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::agent::{AgentConfig, AgentError, Answer};
use crate::core::types::conversation::Visibility;
use crate::core::types::conversation::message::{Message, Role, ToolCall};
use crate::core::types::model::{ModelError, Usage};
use crate::core::types::safety::policy::{ConfirmationRequest, PendingAction};
use crate::core::types::tools::SANDBOX;
use crate::core::types::tools::{RiskClass, ToolRun};
use crate::core::types::trace::{RunStatus, TraceEvent};
use crate::runtime::harness::agent::run::{Inputs, Run};
use crate::runtime::harness::tools::ToolSet;

use crate::core::types::safety::redact::truncate;
pub use crate::runtime::harness::agent::prompt::PromptText;
use crate::runtime::harness::agent::prompt::capability;

/// The dependencies the loop drives. Every one is a trait with a test double.
#[derive(Clone)]
pub struct AgentDeps {
    /// The chat model.
    pub model: Arc<dyn ModelProvider>,
    /// Tools the model may call.
    pub tools: ToolSet,
    /// Gates every tool call.
    pub policy: Arc<dyn Policy>,
    /// Receives every event.
    pub trace: Arc<dyn TraceSink>,
    /// Conversation history, when configured.
    pub conversations: Option<Arc<dyn ConversationStore>>,
    /// Cross-conversation memory, when configured.
    pub memory: Option<Arc<dyn MemoryStore>>,
    /// Where actions wait for approval by the caller, when configured.
    pub confirmations: Option<Arc<dyn ConfirmationStore>>,
    /// Compacts history that no longer fits, when configured.
    pub compactor: Option<Arc<dyn Compactor>>,
    /// Checks every response, when configured.
    pub guardrail: Option<Arc<dyn Guardrail>>,
    /// Workspace a tool result too long to carry is written to, when configured.
    pub sandbox: Option<Arc<dyn Sandbox>>,
    /// Downloads the files a caller attached, when configured. Needs the sandbox too.
    pub files: Option<Arc<dyn FileSource>>,
}

/// Records the answer and how the run ended on the span it ran under.
fn record_outcome(span: &tracing::Span, result: &Result<Answer, AgentError>, limit: usize) {
    match result {
        Ok(answer) => {
            let text = truncate(&answer.text, limit);
            span.record("sparky.output", text.as_str());
            span.record("output.value", text.as_str());
            span.record("sparky.status", format!("{:?}", answer.status).as_str());
            let failed = matches!(answer.status, RunStatus::Error | RunStatus::Blocked);
            span.record("otel.status_code", if failed { "ERROR" } else { "OK" });
            if failed {
                span.record(
                    "otel.status_message",
                    format!("{:?}", answer.status).as_str(),
                );
            }
        }
        Err(error) => {
            span.record("otel.status_code", "ERROR");
            span.record("otel.status_message", error.to_string().as_str());
        }
    }
}

/// The agent loop over its dependencies and configuration.
#[derive(Clone)]
pub struct Agent {
    deps: Arc<AgentDeps>,
    cfg: AgentConfig,
    system_prompt: Arc<str>,
    /// Instructions an operator set in place of system_prompt, shared by every clone.
    instructions: Arc<std::sync::RwLock<Option<Arc<str>>>>,
    /// Wording written around the prompt sections.
    prompt: Arc<PromptText>,
    /// Names of the MCP tools, so a capability line says how each one runs.
    mcp_names: Arc<[String]>,
    /// The check that sends an answer written without tools back to search.
    grounding: Option<Arc<ground::Grounding>>,
}

/// What a step decided.
enum StepOutcome {
    /// Keep looping.
    Continue,
    /// Stop with this status and text.
    Stop(RunStatus, String, Option<ConfirmationRequest>),
}

impl Agent {
    /// Builds an agent over its dependencies, with the default prompt wording.
    pub fn new(deps: AgentDeps, cfg: AgentConfig, system_prompt: impl Into<Arc<str>>) -> Self {
        Self {
            deps: Arc::new(deps),
            cfg,
            system_prompt: system_prompt.into(),
            instructions: Arc::default(),
            prompt: Arc::new(PromptText::default()),
            mcp_names: Arc::from([]),
            grounding: None,
        }
    }

    /// A copy for background runs: read-only tools, nothing kept, nothing waiting on approval.
    pub fn background(&self) -> Self {
        let mut deps = (*self.deps).clone();
        let mut tools = ToolSet::new();
        for tool in self.deps.tools.all() {
            let definition = tool.definition();
            let reads = matches!(
                definition.risk,
                RiskClass::ReadPublic | RiskClass::ReadAuthenticated
            );
            if reads && self.deps.tools.is_on(&definition.name) {
                tools = tools.with(Arc::clone(tool));
            }
        }
        deps.tools = tools;
        deps.conversations = None;
        deps.confirmations = None;
        deps.files = None;
        Self {
            deps: Arc::new(deps),
            ..self.clone()
        }
    }

    /// The configured system instructions.
    pub fn default_instructions(&self) -> Arc<str> {
        Arc::clone(&self.system_prompt)
    }

    /// The system instructions in force: the operator's when set, else the configured ones.
    pub fn instructions(&self) -> Arc<str> {
        self.instructions
            .read()
            .ok()
            .and_then(|set| set.clone())
            .unwrap_or_else(|| Arc::clone(&self.system_prompt))
    }

    /// Sets the instructions for every clone of this agent; None returns to the configured ones.
    pub fn set_instructions(&self, text: Option<String>) {
        if let Ok(mut set) = self.instructions.write() {
            *set = text.map(Arc::from);
        }
    }

    /// The tools, sharing which are switched off with this agent.
    pub fn tools(&self) -> ToolSet {
        self.deps.tools.clone()
    }

    /// Names the MCP tools, which the capabilities section labels differently.
    pub fn with_mcp_names(mut self, names: impl Into<Arc<[String]>>) -> Self {
        self.mcp_names = names.into();
        self
    }

    /// The capabilities section for a turn of visibility. A tool switched off is not in it.
    fn capabilities(&self, visibility: Visibility) -> String {
        capability::render(&capability::from_definitions(
            &self.deps.tools.definitions_for(visibility),
            &self.mcp_names,
        ))
    }

    /// Checks every answer written without tools, and sends one that needs a look back to search.
    pub fn with_grounding(mut self, grounding: ground::Grounding) -> Self {
        self.grounding = Some(Arc::new(grounding));
        self
    }

    /// Replaces the wording written around the prompt sections.
    pub fn with_prompt_text(mut self, prompt: PromptText) -> Self {
        self.prompt = Arc::new(prompt);
        self
    }

    /// Runs one user message to completion; one CHAIN span per request, conversation as session.
    pub async fn run(&self, ctx: &RequestContext, input: &str) -> Result<Answer, AgentError> {
        let asked = truncate(input, self.cfg.max_span_value_chars);
        let span = tracing::info_span!(
            "agent.run",
            "gen_ai.operation.name" = "invoke_agent",
            "gen_ai.agent.name" = "sparky",
            // OpenInference, read by the Phoenix trace UI.
            "openinference.span.kind" = "CHAIN",
            "input.value" = %asked,
            "output.value" = Empty,
            "session.id" = %ctx.conversation_id,
            "user.id" = %ctx.user_id,
            "sparky.request_id" = %ctx.request_id,
            "sparky.tenant_id" = %ctx.tenant_id,
            "sparky.input" = %asked,
            "sparky.output" = Empty,
            "sparky.status" = Empty,
            "otel.status_code" = Empty,
            "otel.status_message" = Empty,
        );
        let result = self.run_inner(ctx, input).instrument(span.clone()).await;
        record_outcome(&span, &result, self.cfg.max_span_value_chars);
        result
    }

    /// Runs an action the caller approved and carries on to an answer.
    pub async fn resume(
        &self,
        ctx: &RequestContext,
        pending: PendingAction,
    ) -> Result<Answer, AgentError> {
        let span = tracing::info_span!(
            "agent.resume",
            "gen_ai.operation.name" = "invoke_agent",
            "gen_ai.agent.name" = "sparky",
            // OpenInference, read by the Phoenix trace UI.
            "openinference.span.kind" = "CHAIN",
            "input.value" = %pending.action.tool,
            "output.value" = Empty,
            "session.id" = %ctx.conversation_id,
            "user.id" = %ctx.user_id,
            "sparky.request_id" = %ctx.request_id,
            "sparky.tenant_id" = %ctx.tenant_id,
            "sparky.input" = %pending.action.tool,
            "sparky.output" = Empty,
            "sparky.status" = Empty,
            "otel.status_code" = Empty,
            "otel.status_message" = Empty,
        );
        let result = self
            .resume_inner(ctx, pending)
            .instrument(span.clone())
            .await;
        record_outcome(&span, &result, self.cfg.max_span_value_chars);
        result
    }

    async fn resume_inner(
        &self,
        ctx: &RequestContext,
        pending: PendingAction,
    ) -> Result<Answer, AgentError> {
        let mut run = Run::new(ctx, "", Vec::new(), 0);
        let call = ToolCall {
            id: pending.call_id,
            name: pending.action.tool,
            arguments: pending.action.arguments,
        };
        run.reserve = self.reserve_for(ctx);
        let (result, found) = self.run_tool(ctx, 0, &call, run.gather_room()).await;
        run.cite(found);
        run.tool_runs.push(ToolRun {
            tool: call.name.clone(),
            ok: result.is_ok(),
        });
        let content = result.unwrap_or_else(|error| format!("error: {error}"));
        run.new_turns
            .push(Message::tool_result(&call.id, &call.name, content));

        let inputs = Inputs {
            history: self.history(ctx).await?,
            memory: Vec::new(),
            uploads: Vec::new(),
        };
        self.loop_until_done(&mut run, &inputs).await
    }

    async fn run_inner(&self, ctx: &RequestContext, input: &str) -> Result<Answer, AgentError> {
        let mut run = Run::new(ctx, input, vec![Message::user(input)], 1);
        run.reserve = self.reserve_for(ctx);
        self.deps.trace.emit(
            ctx,
            TraceEvent::RequestStarted {
                input: input.to_owned(),
                tenant_id: ctx.tenant_id.clone(),
                user_id: ctx.user_id.clone(),
                trace_id: current_trace_id(),
            },
        );

        let mut inputs = self.load(ctx).await?;
        inputs.uploads = self.uploads(ctx, input).await;
        self.loop_until_done(&mut run, &inputs).await
    }

    /// Steps until the loop stops, then keeps the turns and builds the answer.
    async fn loop_until_done(
        &self,
        run: &mut Run<'_>,
        inputs: &Inputs,
    ) -> Result<Answer, AgentError> {
        loop {
            if let Some(stop) = self.check_limits(run) {
                return self.conclude(run, stop, String::new(), None).await;
            }
            self.wrap_up(run);
            run.steps += 1;

            let (status, text, confirmation) = match self.step(run, inputs).await {
                Ok(StepOutcome::Continue) => continue,
                Ok(StepOutcome::Stop(status, text, confirmation)) => (status, text, confirmation),
                Err(ModelError::Cancelled) => (RunStatus::Cancelled, String::new(), None),
                // Gathering ran into the answer reserve: the next step answers from what is in.
                Err(ModelError::Timeout) if !run.force_answer && !run.ctx.remaining().is_zero() => {
                    tracing::info!(
                        steps = run.steps,
                        "gathering ran out of time; answering now"
                    );
                    run.force_answer = true;
                    continue;
                }
                Err(ModelError::Timeout) => (RunStatus::Deadline, String::new(), None),
                Err(error) => {
                    // The turns of a failed request are still kept.
                    if let Err(kept) = self
                        .conclude(run, RunStatus::Error, String::new(), None)
                        .await
                    {
                        tracing::error!(error = %kept, "turns of a failed request were not kept");
                    }
                    return Err(error.into());
                }
            };
            if self.send_to_sandbox(run, &status, confirmation.as_ref()) {
                continue;
            }
            if self
                .send_to_search(run, &status, confirmation.as_ref(), &text)
                .await
            {
                continue;
            }
            return self.conclude(run, status, text, confirmation).await;
        }
    }

    /// Whether to spend one more step on the sandbox rather than answer.
    fn send_to_sandbox(
        &self,
        run: &mut Run<'_>,
        status: &RunStatus,
        confirmation: Option<&ConfirmationRequest>,
    ) -> bool {
        let line = self.prompt.sandbox_retry_line.trim();
        if line.is_empty()
            || run.force_answer
            || run.sent_to_sandbox
            || *status != RunStatus::Answered
            || confirmation.is_some()
        {
            return false;
        }
        let failed = run.tool_runs.iter().any(|t| !t.ok);
        let used = run.tool_runs.iter().any(|t| t.tool == SANDBOX);
        let offered = self.deps.tools.get(SANDBOX).is_some_and(|t| t.available());
        if !failed || used || !offered {
            return false;
        }
        run.sent_to_sandbox = true;
        run.new_turns.push(Message::system(line));
        tracing::info!(
            steps = run.steps,
            "a tool failed and the sandbox was not tried; the run goes back for one more step"
        );
        true
    }

    /// The time a request keeps for its final answer: the configured reserve, at most a quarter of what is left.
    fn reserve_for(&self, ctx: &RequestContext) -> Duration {
        self.cfg.answer_reserve.min(ctx.remaining() / 4)
    }

    /// Makes the next step answer once tools have run and the last step, the reserve or the call cap is reached.
    fn wrap_up(&self, run: &mut Run<'_>) {
        let gathered = run.new_turns.iter().any(|m| m.role == Role::Tool);
        if run.force_answer || !gathered {
            return;
        }
        let last_step = run.steps + 1 >= self.cfg.max_steps;
        let late = run.ctx.remaining() <= run.reserve;
        let spent = self.cfg.max_tool_calls > 0 && run.tool_runs.len() >= self.cfg.max_tool_calls;
        if last_step || late || spent {
            tracing::info!(
                steps = run.steps,
                tool_calls = run.tool_runs.len(),
                last_step,
                late,
                spent,
                "enough gathered; the next step answers"
            );
            run.force_answer = true;
        }
    }

    /// The status the loop stops with before the next step, if any.
    fn check_limits(&self, run: &Run<'_>) -> Option<RunStatus> {
        if run.ctx.cancel.is_cancelled() {
            Some(RunStatus::Cancelled)
        } else if run.ctx.remaining().is_zero() {
            Some(RunStatus::Deadline)
        } else if run.steps >= self.cfg.max_steps {
            Some(RunStatus::StepLimit)
        } else {
            None
        }
    }

    /// The dollar cost of usage at the configured rates.
    fn cost(&self, usage: Usage) -> f64 {
        (f64::from(usage.prompt_tokens) * self.cfg.usd_per_m_prompt
            + f64::from(usage.completion_tokens) * self.cfg.usd_per_m_completion)
            / 1_000_000.0
    }
}

/// Milliseconds elapsed since since.
fn ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The OpenTelemetry trace id of the current span, or None when no span is exported.
fn current_trace_id() -> Option<String> {
    let id = tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id();
    (id != TraceId::INVALID).then(|| id.to_string())
}
