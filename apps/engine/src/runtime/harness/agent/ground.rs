//! The grounding check: an answer written without any tool goes back once when it rests on facts that need a look.

use std::sync::Arc;

use super::Agent;
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::safety::policy::ConfirmationRequest;
use crate::core::types::tools::SEARCH;
use crate::core::types::trace::{RunStatus, TraceEvent};
use crate::runtime::harness::agent::run::Run;
use crate::runtime::harness::agent::task::Task;

/// The prompted check and the line that sends a run back to search.
pub struct Grounding {
    task: Task,
    retry_line: Arc<str>,
}

impl Grounding {
    /// Builds the check over its task and the line a run gets when it goes back.
    pub fn new(task: Task, retry_line: impl Into<Arc<str>>) -> Self {
        Self {
            task,
            retry_line: retry_line.into(),
        }
    }
}

/// Whether the check answered that the reply needs a look.
pub fn needs_look(verdict: &str) -> bool {
    verdict
        .trim()
        .trim_start_matches(|c: char| !c.is_ascii_alphabetic())
        .to_ascii_uppercase()
        .starts_with("LOOK")
}

impl Agent {
    /// Whether to withdraw an answer written without tools and send the run back to search.
    pub(super) async fn send_to_search(
        &self,
        run: &mut Run<'_>,
        status: &RunStatus,
        confirmation: Option<&ConfirmationRequest>,
        text: &str,
    ) -> bool {
        let Some(grounding) = self.grounding.as_deref() else {
            return false;
        };
        let line = grounding.retry_line.trim();
        let offered = self.deps.tools.get(SEARCH).is_some_and(|t| t.available());
        if line.is_empty()
            || run.force_answer
            || run.sent_to_search
            || *status != RunStatus::Answered
            || confirmation.is_some()
            || !run.tool_runs.is_empty()
            || !offered
            || text.trim().is_empty()
        {
            return false;
        }
        run.sent_to_search = true;
        let input = format!("Student: {}\n\nReply: {text}", run.input);
        let verdict = match grounding.task.run(run.ctx, &input).await {
            Ok(verdict) => verdict,
            Err(error) => {
                tracing::warn!(error = %error, "grounding check failed; the answer stands");
                return false;
            }
        };
        if !needs_look(&verdict) {
            return false;
        }
        if run
            .new_turns
            .last()
            .is_some_and(|m| m.role == Role::Assistant && m.tool_calls.is_empty())
        {
            run.new_turns.pop();
        }
        self.deps
            .trace
            .emit(run.ctx, TraceEvent::AnswerDraftCleared { step: run.steps });
        run.new_turns.push(Message::system(line));
        tracing::info!(
            steps = run.steps,
            "an answer used no tools and needs a look; the run goes back to search"
        );
        true
    }
}
