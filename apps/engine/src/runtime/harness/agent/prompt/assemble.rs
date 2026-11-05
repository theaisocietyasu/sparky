//! Context assembly: fixed order, per-section budgets. History trims first; the rest is kept.

use crate::core::types::agent::assemble::{Assembled, Budget, Sections};
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::memory::MemoryKind;
use crate::core::types::model::tokens::estimate;

/// Builds the message list within budget.
pub fn assemble(ctx: &RequestContext, s: &Sections<'_>, budget: Budget) -> Assembled {
    let mut messages = Vec::new();
    let mut used = 0usize;

    let cpt = budget.chars_per_token;
    let role_line = if ctx.roles.is_empty() {
        s.templates
            .role_line_no_roles
            .replace("{user}", &ctx.user_id)
    } else {
        s.templates
            .role_line
            .replace("{user}", &ctx.user_id)
            .replace("{roles}", &ctx.roles.join(", "))
    };
    let mut system = format!("{}\n\n{role_line}", s.system.trim());
    // A student with no memories and no earlier turns in this conversation is met for the first time.
    let first = s.memory.is_empty() && s.history.is_empty();
    let onboarding = if first {
        s.templates.onboarding_rules
    } else {
        ""
    };
    let private = [
        s.templates.memory_rules,
        s.templates.discovery_rules,
        s.templates.proactive_rules,
        onboarding,
    ];
    for rules in private {
        let rules = rules.trim();
        if ctx.visibility == Visibility::Private && !rules.is_empty() {
            system.push_str("\n\n");
            system.push_str(rules);
        }
    }
    if !s.date.is_empty() {
        let date_line = s.templates.date_line.replace("{date}", s.date);
        system.push('\n');
        system.push_str(&date_line);
    }
    used += estimate(&system, cpt);
    messages.push(Message::system(system));

    if !s.capabilities.trim().is_empty() {
        let block = format!(
            "{}\n{}",
            s.templates.capabilities_header.trim(),
            s.capabilities.trim()
        );
        let cost = estimate(&block, cpt);
        if cost <= budget.capabilities {
            used += cost;
            messages.push(Message::system(block));
        }
    }

    let mut memory_used = 0;
    if let Some((block, spent, count)) = memory_block(s, budget.memory, cpt) {
        used += spent;
        memory_used = count;
        messages.push(Message::system(block));
    }

    let reply = reply_block(ctx, s, budget.reply, cpt);
    let reply_cost = reply.as_deref().map_or(0, |b| estimate(b, cpt));
    let uploads = (!s.uploads.is_empty()).then(|| s.uploads.join("\n\n"));
    used += uploads.as_deref().map_or(0, |b| estimate(b, cpt));
    let input_only = if s.input.is_empty() {
        0
    } else {
        estimate(s.input, cpt)
    };
    let turn_room = budget.total.saturating_sub(used + reply_cost + input_only);
    let turn = fit_turn(s.turn, turn_room, cpt, s.templates.result_cut_line);
    let turn_cost: usize = turn.iter().map(|m| m.estimated_tokens(cpt)).sum();

    let input_cost = input_only + turn_cost + reply_cost;

    let remaining_total = budget.total.saturating_sub(used + input_cost);
    let (history, spent) = history_within(s.history, budget.history.min(remaining_total), cpt);
    used += spent;
    messages.extend(history);

    if let Some(block) = reply {
        messages.push(Message::system(block));
    }

    if let Some(block) = uploads {
        messages.push(Message::system(block));
    }

    if !s.input.is_empty() {
        messages.push(Message::user_with_images(s.input, ctx.images.clone()));
    }
    used += input_cost;
    messages.extend(turn);

    Assembled {
        messages,
        estimated_tokens: used,
        memory_used,
    }
}

/// Prior turns within budget, keeping the newest, and what they cost.
fn history_within(history: &[Message], budget: usize, cpt: usize) -> (Vec<Message>, usize) {
    let (summary, rest) = match history.split_first() {
        Some((first, rest)) if first.role == Role::Summary => (Some(first), rest),
        _ => (None, history),
    };
    let summary = summary.filter(|m| m.estimated_tokens(cpt) <= budget);
    let mut spent = summary.map_or(0, |m| m.estimated_tokens(cpt));
    let mut kept: Vec<&Message> = Vec::new();
    for m in rest.iter().rev() {
        let cost = m.estimated_tokens(cpt);
        if spent + cost > budget {
            break;
        }
        kept.push(m);
        spent += cost;
    }
    while kept.last().is_some_and(|m| m.role == Role::Tool) {
        if let Some(dropped) = kept.pop() {
            spent -= dropped.estimated_tokens(cpt);
        }
    }
    kept.reverse();
    (summary.into_iter().chain(kept).cloned().collect(), spent)
}

/// The turns of this request within room tokens, cutting tool results over their share.
fn fit_turn(turn: &[Message], room: usize, cpt: usize, cut_line: &str) -> Vec<Message> {
    let mut out = turn.to_vec();
    let cost: usize = out.iter().map(|m| m.estimated_tokens(cpt)).sum();
    if cost <= room {
        return out;
    }
    let fixed: usize = out
        .iter()
        .filter(|m| m.role != Role::Tool)
        .map(|m| m.estimated_tokens(cpt))
        .sum();
    let mut left = room.saturating_sub(fixed);
    let mut results: Vec<usize> = (0..out.len())
        .filter(|&i| out.get(i).is_some_and(|m| m.role == Role::Tool))
        .collect();
    results.sort_by_key(|&i| out.get(i).map_or(0, |m| m.estimated_tokens(cpt)));
    let mut waiting = results.len();
    for i in results {
        let Some(m) = out.get_mut(i) else {
            continue;
        };
        let share = left / waiting.max(1);
        if m.estimated_tokens(cpt) > share {
            m.content = cut(&m.content, share, cpt, cut_line);
        }
        left = left.saturating_sub(m.estimated_tokens(cpt));
        waiting -= 1;
    }
    out
}

/// The head of content that fits tokens, followed by the cut line naming what was left out.
fn cut(content: &str, tokens: usize, cpt: usize, cut_line: &str) -> String {
    let line = |dropped: usize| cut_line.trim().replace("{chars}", &dropped.to_string());
    let reserve = estimate(&line(content.chars().count()), cpt) + 2;
    let room = tokens.saturating_sub(reserve).saturating_mul(cpt.max(1));
    let mut end = 0;
    for (at, ch) in content.char_indices() {
        if at + ch.len_utf8() > room {
            break;
        }
        end = at + ch.len_utf8();
    }
    let head = content.get(..end).unwrap_or_default().trim_end();
    let dropped = content.chars().count() - head.chars().count();
    format!("{head}\n{}", line(dropped))
}

/// The memory section: the header and every memory that fits budget. None when there is no memory.
fn memory_block(s: &Sections<'_>, budget: usize, cpt: usize) -> Option<(String, usize, usize)> {
    if s.memory.is_empty() {
        return None;
    }
    let mut block = format!("{}\n", s.templates.memory_header.trim());
    let mut spent = estimate(&block, cpt);
    let mut count = 0;
    for m in s.memory {
        let when = match (m.kind, m.expires_at) {
            (MemoryKind::Task, Some(due)) => format!("due {}", due.format("%Y-%m-%d")),
            _ => m.created_at.format("%Y-%m-%d").to_string(),
        };
        let line = format!(
            "- [{}] {}: {} ({when})\n",
            m.short_id(),
            m.kind.as_str(),
            m.content.trim()
        );
        let cost = estimate(&line, cpt);
        if spent + cost > budget {
            break;
        }
        block.push_str(&line);
        spent += cost;
        count += 1;
    }
    Some((block, spent, count))
}

/// The message of ours the caller replied to, quoted under its header and cut to budget.
fn reply_block(
    ctx: &RequestContext,
    s: &Sections<'_>,
    budget: usize,
    cpt: usize,
) -> Option<String> {
    let quoted = ctx.reply_to.as_deref()?.trim();
    if quoted.is_empty() {
        return None;
    }
    let header = s.templates.reply_header.trim();
    let room = budget.saturating_sub(estimate(header, cpt));
    if room == 0 {
        return None;
    }
    let mut kept = String::new();
    for line in quoted.lines() {
        let candidate = if kept.is_empty() {
            line.to_owned()
        } else {
            format!("{kept}\n{line}")
        };
        if estimate(&candidate, cpt) > room {
            break;
        }
        kept = candidate;
    }
    if kept.is_empty() {
        kept = quoted.chars().take(room.saturating_mul(cpt)).collect();
    }
    Some(format!("{header}\n{kept}"))
}
