//! What a request loads before its first model call: history and memory.

use super::Agent;
use crate::core::types::agent::AgentError;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::conversation::{Stored, Visibility};
use crate::core::types::memory::Memory;
use crate::core::types::trace::TraceEvent;
use crate::runtime::harness::agent::run::Inputs;

/// Estimated tokens of every turn.
fn cost(turns: &[Stored], chars_per_token: usize) -> usize {
    turns
        .iter()
        .map(|s| s.message.estimated_tokens(chars_per_token))
        .sum()
}

/// Where the kept turns start: the newest turns within keep tokens, moved up to a user turn.
fn tail_start(turns: &[Stored], keep: usize, chars_per_token: usize) -> usize {
    let mut spent = 0;
    let mut start = turns.len();
    for (at, stored) in turns.iter().enumerate().rev() {
        let cost = stored.message.estimated_tokens(chars_per_token);
        if spent + cost > keep {
            break;
        }
        spent += cost;
        start = at;
    }
    while turns
        .get(start)
        .is_some_and(|s| s.message.role != Role::User)
    {
        start += 1;
    }
    start
}

impl Agent {
    /// The stored history of the conversation, compacted when it overflows its budget.
    pub(super) async fn history(&self, ctx: &RequestContext) -> Result<Vec<Message>, AgentError> {
        let Some(store) = &self.deps.conversations else {
            return Ok(Vec::new());
        };
        let loaded = store
            .load(ctx, self.cfg.history_turns)
            .await
            .map_err(|error| AgentError::Store(error.to_string()))?;
        Ok(self.compacted(ctx, loaded).await)
    }

    /// Replaces older turns with one summary once history is over budget; returns prompt history.
    async fn compacted(&self, ctx: &RequestContext, turns: Vec<Stored>) -> Vec<Message> {
        let Some(compactor) = &self.deps.compactor else {
            return turns.into_iter().map(|s| s.message).collect();
        };
        let cpt = self.cfg.budget.chars_per_token;
        if cost(&turns, cpt) <= self.cfg.budget.history {
            return turns.into_iter().map(|s| s.message).collect();
        }
        let overflow = tail_start(&turns, self.cfg.history_keep, cpt);
        let Some(covers) = overflow
            .checked_sub(1)
            .and_then(|last| turns.get(last))
            .map(|s| s.position)
        else {
            return turns.into_iter().map(|s| s.message).collect();
        };
        let replaced: Vec<Message> = turns[..overflow]
            .iter()
            .map(|s| s.message.clone())
            .collect();
        self.deps.trace.emit(
            ctx,
            TraceEvent::Compaction {
                turns: replaced.len(),
            },
        );
        let summary = match compactor.compact(ctx, &replaced).await {
            Ok(summary) => summary,
            Err(error) => {
                tracing::warn!(error = %error, "compaction failed; history is trimmed instead");
                return turns.into_iter().map(|s| s.message).collect();
            }
        };
        if let Some(store) = &self.deps.conversations
            && let Err(error) = store.append_summary(ctx, &summary, covers).await
        {
            // The prompt still gets the summary.
            tracing::warn!(error = %error, "compacted turn was not stored");
        }
        let mut out = Vec::with_capacity(turns.len() - overflow + 1);
        out.push(summary);
        out.extend(turns.into_iter().skip(overflow).map(|s| s.message));
        out
    }

    /// Loads the history and memory of one request.
    pub(super) async fn load(&self, ctx: &RequestContext) -> Result<Inputs, AgentError> {
        let deps = &self.deps;
        let history = self.history(ctx).await?;
        // Memories are personal and reach the prompt only in a private turn.
        let memory = if ctx.visibility == Visibility::Private {
            let recalled = match &deps.memory {
                Some(store) => match store.recall(ctx).await {
                    Ok(memories) => memories,
                    Err(error) => {
                        tracing::error!(error = %error, "memory recall failed; none is recalled");
                        Vec::new()
                    }
                },
                None => Vec::new(),
            };
            self.with_profile(ctx, recalled).await
        } else {
            Vec::new()
        };
        deps.trace.emit(
            ctx,
            TraceEvent::MemoryRecalled {
                count: memory.len(),
            },
        );
        Ok(Inputs {
            history,
            memory,
            uploads: Vec::new(),
        })
    }

    /// Appends this user's profile nodes and relations to recalled memories. Failed reads add none.
    async fn with_profile(&self, ctx: &RequestContext, mut memory: Vec<Memory>) -> Vec<Memory> {
        let Some(graph) = &self.deps.profile_graph else {
            return memory;
        };
        let limit = self.cfg.memory_recall_limit;
        match graph.recall(ctx, limit).await {
            Ok(nodes) => memory.extend(nodes.iter().map(Memory::from)),
            Err(error) => tracing::warn!(error = %error, "profile graph recall failed"),
        }
        match graph.relations(ctx, limit).await {
            Ok(relations) => memory.extend(relations.iter().map(Memory::from)),
            Err(error) => tracing::warn!(error = %error, "profile relation recall failed"),
        }
        memory
    }
}
