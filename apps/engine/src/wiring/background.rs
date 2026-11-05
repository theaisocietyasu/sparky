//! The nudge loop: the background agent over the read-only tools, and the loop that drives it.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::FixedOffset;

use super::Stores;
use crate::core::config::Config;
use crate::core::traits::nudge::Background;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::nudge::NudgePrefs;
use crate::core::types::trace::RunStatus;
use crate::runtime::harness::agent::Agent;
use crate::runtime::nudge::rules::Rules;
use crate::runtime::nudge::{Nudger, Settings, run_nudges};

/// The agent run for one student with no conversation kept.
struct AgentBackground {
    agent: Agent,
}

#[async_trait]
impl Background for AgentBackground {
    async fn ask(&self, ctx: &RequestContext, instruction: &str) -> Result<String, String> {
        let answer = self
            .agent
            .run(ctx, instruction)
            .await
            .map_err(|e| e.to_string())?;
        match answer.status {
            RunStatus::Answered => Ok(answer.text),
            other => Err(format!("the run ended {other:?}")),
        }
    }
}

/// Quiet time and kinds for a student who set nothing.
pub(super) fn defaults(cfg: &Config) -> NudgePrefs {
    NudgePrefs {
        quiet_start: cfg.nudges.quiet_start,
        quiet_end: cfg.nudges.quiet_end,
        off: Vec::new(),
    }
}

/// Starts the nudge loop when nudges are on and the local store holds them.
pub(super) fn start(
    cfg: &Config,
    agent: &Agent,
    stores: &Stores,
    sources: Vec<Arc<dyn UpcomingSource>>,
) -> anyhow::Result<()> {
    let Some(store) = stores.nudges.clone() else {
        return Ok(());
    };
    let n = &cfg.nudges;
    let offset = FixedOffset::east_opt(cfg.prompt.utc_offset_hours * 3600)
        .ok_or_else(|| anyhow::anyhow!("prompt.utc_offset_hours is out of range"))?;
    let settings = Settings {
        rules: Rules {
            brief_hour: n.brief_hour,
            brief_days: n.brief_days,
            week_plan_hour: n.week_plan_hour,
            due_soon_hours: n.due_soon_hours,
            start_early_days: n.start_early_days,
            big_work: n.big_work.iter().map(|w| w.to_lowercase()).collect(),
            new_item_days: n.new_item_days,
            follow_up_days: n.follow_up_days,
            follow_up_max_days: n.follow_up_max_days,
            after_due_hours: n.after_due_hours,
            after_due_lines: n.after_due_lines.clone(),
            exam_words: n.exam_words.iter().map(|w| w.to_lowercase()).collect(),
            exam_week_min: n.exam_week_min,
            exam_week_lines: n.exam_week_lines.clone(),
            wins_hour: n.wins_hour,
            wins_min: n.wins_min,
            wins_lines: n.wins_lines.clone(),
            tips_days: n.tips_days,
            tip_hour: n.tip_hour,
            tips: n.tips.clone(),
        },
        active_days: i64::from(n.active_days),
        daily_cap: n.daily_cap,
        defaults: defaults(cfg),
        offset,
        event_hour: n.event_hour,
        study_hour: n.study_hour,
        calendar_weekday: n.calendar_weekday,
        calendar_hour: n.calendar_hour,
        ask_timeout: Duration::from_secs(n.ask_timeout_secs),
        keep_days: i64::from(n.keep_days),
        nothing: n.nothing.clone(),
        event_prompt: n.event_prompt.clone(),
        calendar_prompt: n.calendar_prompt.clone(),
        study_prompt: n.study_prompt.clone(),
        opportunity_weekday: n.opportunity_weekday,
        opportunity_hour: n.opportunity_hour,
        opportunity_prompt: n.opportunity_prompt.clone(),
    };
    let background: Arc<dyn Background> = Arc::new(AgentBackground {
        agent: agent.background(),
    });
    let labels: Vec<&str> = sources.iter().map(|s| s.label()).collect();
    tracing::info!(sources = %labels.join(", "), "nudges on");
    let nudger = Nudger::new(
        store,
        sources,
        Some(Arc::clone(&stores.memory)),
        Some(background),
        settings,
    );
    tokio::spawn(run_nudges(
        Arc::new(nudger),
        Duration::from_secs(n.tick_secs),
    ));
    Ok(())
}
