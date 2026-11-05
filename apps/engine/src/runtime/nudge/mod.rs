//! The nudge loop: on each pass, looks at every active student and sends what is worth saying.

pub mod rules;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveTime, Timelike, Utc};

use crate::core::traits::memory::MemoryStore;
use crate::core::traits::nudge::{Background, NudgeStore};
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::memory::{Memory, MemoryKind};
use crate::core::types::nudge::{Nudge, NudgeKind, NudgePrefs, Person};
use crate::core::types::tools::upcoming::UpcomingItem;
use crate::core::types::tools::{REMEMBER, REMIND};
use crate::runtime::nudge::rules::{Action, Facts, Rules, day_key, week_key, when};

/// Settings of the loop beside the rules.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The rules that need no search.
    pub rules: Rules,
    /// Students active in this many days get nudges.
    pub active_days: i64,
    /// Most nudges a student gets in a local day.
    pub daily_cap: usize,
    /// Quiet time and kinds for a student who set nothing.
    pub defaults: NudgePrefs,
    /// The student's local time zone.
    pub offset: FixedOffset,
    /// Local hour the event match runs from.
    pub event_hour: u32,
    /// Local hour the late study check runs from.
    pub study_hour: u32,
    /// Local weekday of the calendar check, 1 for Monday.
    pub calendar_weekday: u32,
    /// Local hour the calendar check runs from.
    pub calendar_hour: u32,
    /// Budget for one background run.
    pub ask_timeout: StdDuration,
    /// Days a sent key is kept.
    pub keep_days: i64,
    /// What the background agent answers when nothing is worth sending.
    pub nothing: String,
    /// Instruction of the event match.
    pub event_prompt: String,
    /// Instruction of the calendar check.
    pub calendar_prompt: String,
    /// Instruction of the late study check.
    pub study_prompt: String,
    /// Local weekday of the openings check, 1 for Monday. 0 never runs it.
    pub opportunity_weekday: u32,
    /// Local hour the openings check runs from.
    pub opportunity_hour: u32,
    /// Instruction of the openings check.
    pub opportunity_prompt: String,
}

/// What the loop reads and writes.
pub struct Nudger {
    store: Arc<dyn NudgeStore>,
    sources: Vec<Arc<dyn UpcomingSource>>,
    memory: Option<Arc<dyn MemoryStore>>,
    background: Option<Arc<dyn Background>>,
    settings: Settings,
}

/// A nudge the background agent writes, before it runs.
struct Ask {
    kind: NudgeKind,
    key: String,
    instruction: String,
}

impl Nudger {
    /// A loop over the store, the upcoming sources, the memories, and the background agent.
    pub fn new(
        store: Arc<dyn NudgeStore>,
        sources: Vec<Arc<dyn UpcomingSource>>,
        memory: Option<Arc<dyn MemoryStore>>,
        background: Option<Arc<dyn Background>>,
        settings: Settings,
    ) -> Self {
        Self {
            store,
            sources,
            memory,
            background,
            settings,
        }
    }

    /// One pass over every active student. Returns how many nudges went out.
    pub async fn pass(&self, now: DateTime<Utc>) -> usize {
        let before = now - Duration::days(self.settings.keep_days);
        if let Err(error) = self.store.prune(before).await {
            tracing::warn!(%error, "could not prune sent nudges");
        }
        let since = now - Duration::days(self.settings.active_days);
        let people = match self.store.people(since).await {
            Ok(people) => people,
            Err(error) => {
                tracing::error!(%error, "could not list students for nudges");
                return 0;
            }
        };
        let mut sent = 0;
        for who in &people {
            match self.person(who, now).await {
                Ok(n) => sent += n,
                Err(error) => tracing::warn!(%error, "nudges for one student failed"),
            }
        }
        sent
    }

    /// The nudges for one student. Returns how many went out.
    pub async fn person(&self, who: &Person, now: DateTime<Utc>) -> Result<usize, String> {
        let s = &self.settings;
        let prefs = self
            .store
            .prefs(who)
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| s.defaults.clone());
        let local = now.with_timezone(&s.offset);
        let hour = u8::try_from(local.hour()).unwrap_or(0);
        if prefs.quiet_at(hour) {
            return Ok(0);
        }
        let midnight = local
            .with_time(NaiveTime::MIN)
            .single()
            .map_or(now, |m| m.with_timezone(&Utc));
        let today = self
            .store
            .count_since(who, midnight)
            .await
            .map_err(|e| e.to_string())?;
        let mut room = s.daily_cap.saturating_sub(today);
        if room == 0 {
            return Ok(0);
        }

        let ctx = self.context(who);
        let items = self.items(&ctx, now).await;
        let memories = self.memories(&ctx).await;
        let first_seen = self.store.first_seen(who).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "could not read when a student first talked");
            None
        });
        let empty = HashSet::new();
        let probe = Facts {
            now,
            offset: s.offset,
            items: &items,
            memories: &memories,
            sent: &empty,
            first_seen,
        };
        let mut keys = rules::candidate_keys(&probe, &s.rules);
        let asks = self.asks(&probe);
        keys.extend(asks.iter().map(|a| a.key.clone()));
        let sent: HashSet<String> = self
            .store
            .sent_keys(who, &keys)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
        let facts = Facts {
            sent: &sent,
            ..probe
        };

        let mut out = 0;
        for action in rules::all(&facts, &s.rules) {
            match action {
                Action::Mark(kind, keys) => {
                    self.store
                        .mark(who, kind, &keys)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                Action::Send(nudge) if room > 0 && prefs.allows(nudge.kind) => {
                    self.store
                        .send(who, &nudge)
                        .await
                        .map_err(|e| e.to_string())?;
                    room -= 1;
                    out += 1;
                }
                Action::Send(_) => {}
            }
        }
        for ask in asks {
            if room == 0 {
                break;
            }
            if !prefs.allows(ask.kind) || sent.contains(&ask.key) {
                continue;
            }
            if self.run_ask(who, &ctx, ask).await? {
                room -= 1;
                out += 1;
            }
        }
        Ok(out)
    }

    /// A private request context for one student, bounded by the ask timeout.
    fn context(&self, who: &Person) -> RequestContext {
        let mut ctx = RequestContext::new(&who.tenant, &who.user, self.settings.ask_timeout);
        ctx.visibility = Visibility::Private;
        ctx
    }

    /// Dated items from every source, from the start of the week before to the furthest any rule looks.
    async fn items(
        &self,
        ctx: &RequestContext,
        now: DateTime<Utc>,
    ) -> Vec<(&'static str, UpcomingItem)> {
        let r = &self.settings.rules;
        let days = r
            .brief_days
            .max(r.new_item_days)
            .max(r.start_early_days + 1);
        let (from, until) = (now - Duration::days(7), now + Duration::days(days));
        let mut out = Vec::new();
        for source in &self.sources {
            match source.upcoming(ctx, from, until).await {
                Ok(items) => out.extend(items.into_iter().map(|i| (source.tool(), i))),
                Err(why) => tracing::debug!(source = source.label(), %why, "no items for nudges"),
            }
        }
        out.sort_by_key(|(_, i)| i.at);
        out
    }

    /// What Sparky remembers about the student, or nothing when memory is off.
    async fn memories(&self, ctx: &RequestContext) -> Vec<Memory> {
        let Some(memory) = &self.memory else {
            return Vec::new();
        };
        memory.recall(ctx).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "could not recall memories for nudges");
            Vec::new()
        })
    }

    /// The nudges the background agent writes that are due now, most useful first.
    fn asks(&self, f: &Facts<'_>) -> Vec<Ask> {
        let s = &self.settings;
        if self.background.is_none() {
            return Vec::new();
        }
        let local = f.local();
        let (hour, today) = (local.hour(), local.date_naive());
        let mut out = Vec::new();
        if (s.study_hour..21).contains(&hour) {
            let tomorrow_end = f.now + Duration::hours(30);
            if let Some((_, item)) = f.items.iter().find(|(tool, i)| {
                *tool != REMIND && *tool != REMEMBER && i.at > f.now && i.at <= tomorrow_end
            }) {
                out.push(Ask {
                    kind: NudgeKind::Study,
                    key: day_key("study", today),
                    instruction: s
                        .study_prompt
                        .replace("{item}", &item.title)
                        .replace("{due}", &when(item, f.now, f.offset))
                        .replace("{nothing}", &s.nothing),
                });
            }
        }
        let knows_them = f
            .memories
            .iter()
            .any(|m| matches!(m.kind, MemoryKind::Fact | MemoryKind::Preference));
        let busy = rules::exam_week_on(f, &s.rules);
        if knows_them
            && !busy
            && local.weekday().number_from_monday() == s.opportunity_weekday
            && (s.opportunity_hour..20).contains(&hour)
        {
            out.push(Ask {
                kind: NudgeKind::Opportunity,
                key: week_key("opportunity", today),
                instruction: s.opportunity_prompt.replace("{nothing}", &s.nothing),
            });
        }
        if knows_them && !busy && (s.event_hour..20).contains(&hour) {
            out.push(Ask {
                kind: NudgeKind::Event,
                key: day_key("event", today),
                instruction: s.event_prompt.replace("{nothing}", &s.nothing),
            });
        }
        if local.weekday().number_from_monday() == s.calendar_weekday
            && (s.calendar_hour..20).contains(&hour)
        {
            out.push(Ask {
                kind: NudgeKind::Calendar,
                key: week_key("calendar", today),
                instruction: s.calendar_prompt.replace("{nothing}", &s.nothing),
            });
        }
        out
    }

    /// Runs one background ask and sends what it wrote. The key is marked either way.
    async fn run_ask(&self, who: &Person, ctx: &RequestContext, ask: Ask) -> Result<bool, String> {
        let Some(background) = &self.background else {
            return Ok(false);
        };
        let mut instruction = ask.instruction;
        if instruction.contains("{already}") {
            let told = self
                .store
                .recent(who, ask.kind, 4)
                .await
                .map_err(|e| e.to_string())?;
            let already = if told.is_empty() {
                "nothing yet".to_owned()
            } else {
                told.join(" | ")
            };
            instruction = instruction.replace("{already}", &already);
        }
        let mut run = ctx.clone();
        run.deadline = std::time::Instant::now() + self.settings.ask_timeout;
        let written = background.ask(&run, &instruction).await;
        let keys = vec![ask.key];
        match written {
            Ok(text) if !self.is_nothing(&text) => {
                let nudge = Nudge {
                    kind: ask.kind,
                    keys,
                    text: text.trim().to_owned(),
                };
                self.store
                    .send(who, &nudge)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(true)
            }
            Ok(_) => {
                self.store
                    .mark(who, ask.kind, &keys)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(false)
            }
            Err(why) => {
                tracing::warn!(kind = ask.kind.as_str(), %why, "a background nudge failed");
                self.store
                    .mark(who, ask.kind, &keys)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(false)
            }
        }
    }

    /// Whether the agent said there is nothing worth sending.
    fn is_nothing(&self, text: &str) -> bool {
        let bare = text
            .trim()
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_ascii_uppercase();
        let nothing = self.settings.nothing.to_ascii_uppercase();
        bare.is_empty()
            || bare == nothing
            || (bare.starts_with(&format!("{nothing} ")) && bare.len() < 80)
    }
}

/// Runs a pass every tick until the process ends.
pub async fn run_nudges(nudger: Arc<Nudger>, every: StdDuration) {
    let mut tick = tokio::time::interval(every);
    loop {
        tick.tick().await;
        let sent = nudger.pass(Utc::now()).await;
        if sent > 0 {
            tracing::info!(sent, "nudges sent");
        }
    }
}
