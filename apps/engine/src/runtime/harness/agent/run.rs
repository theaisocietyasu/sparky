//! Per-request state the loop carries: what loaded before the first call, what steps accumulate.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::Message;
use crate::core::types::knowledge::evidence::Citation;
use crate::core::types::memory::Memory;
use crate::core::types::model::Usage;
use crate::core::types::tools::ToolRun;

/// What one request loaded before its first model call.
pub(super) struct Inputs {
    pub(super) history: Vec<Message>,
    pub(super) memory: Vec<Memory>,
    /// One prompt line per file the user attached.
    pub(super) uploads: Vec<String>,
}

/// Mutable state carried across steps.
pub(super) struct Run<'a> {
    pub(super) ctx: &'a RequestContext,
    pub(super) input: &'a str,
    pub(super) started: Instant,
    pub(super) steps: u32,
    pub(super) usage: Usage,
    /// Turns produced during this request, persisted at the end. First is the user input.
    pub(super) new_turns: Vec<Message>,
    /// Every tool and arguments pair already executed this request.
    pub(super) seen_calls: HashSet<String>,
    /// Tools that ran, in order, for the answer and the client.
    pub(super) tool_runs: Vec<ToolRun>,
    /// Content of the memories that fit the prompt on the most recent step.
    pub(super) memories_in_prompt: Vec<String>,
    /// Pages the tools read, in the order they read them.
    pub(super) tool_sources: Vec<Citation>,
    /// Set after a step of nothing but repeats. The next model call gets no tools.
    pub(super) force_answer: bool,
    /// Set once the run has been sent back to try the sandbox. It is offered once a turn.
    pub(super) sent_to_sandbox: bool,
    /// Set once an answer written without tools has gone to the grounding check. It runs once a turn.
    pub(super) sent_to_search: bool,
    /// Leading new_turns entries that assembly appends itself, which the prompt must not repeat.
    pub(super) appended_by_assembly: usize,
    /// Time kept for the final answer. Gathering stops when less is left.
    pub(super) reserve: Duration,
}

impl<'a> Run<'a> {
    /// Fresh state for one request; new_turns holds the turns known before the first step.
    pub(super) fn new(
        ctx: &'a RequestContext,
        input: &'a str,
        new_turns: Vec<Message>,
        appended_by_assembly: usize,
    ) -> Self {
        Self {
            ctx,
            input,
            started: Instant::now(),
            steps: 0,
            usage: Usage::default(),
            new_turns,
            seen_calls: HashSet::new(),
            tool_runs: Vec::new(),
            memories_in_prompt: Vec::new(),
            tool_sources: Vec::new(),
            force_answer: false,
            sent_to_sandbox: false,
            sent_to_search: false,
            appended_by_assembly,
            reserve: Duration::ZERO,
        }
    }

    /// Adds the sources a tool read, skipping any that would show as the same chip.
    pub(super) fn cite(&mut self, found: Vec<Citation>) {
        for citation in found {
            let shown = |c: &Citation| (c.title.clone(), host(c.url.as_deref()));
            let duplicate = self.tool_sources.iter().any(|kept| {
                (kept.url.is_some() && kept.url == citation.url) || shown(kept) == shown(&citation)
            });
            if !duplicate {
                self.tool_sources.push(citation);
            }
        }
    }

    /// Time left for gathering before the answer reserve. All of what is left once that runs out.
    pub(super) fn gather_room(&self) -> Duration {
        let left = self.ctx.remaining();
        let room = left.saturating_sub(self.reserve);
        if room.is_zero() { left } else { room }
    }
}

/// The host of a URL without its www prefix, as a source chip shows it.
fn host(url: Option<&str>) -> Option<String> {
    let url = url?;
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    Some(host.trim_start_matches("www.").to_ascii_lowercase())
}
