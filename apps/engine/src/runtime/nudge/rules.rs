//! The nudges that need no search: worked out from deadlines and memories alone.

use std::collections::HashSet;
use std::fmt::Write as _;

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, Timelike, Utc, Weekday};

use crate::core::types::memory::{Memory, MemoryKind};
use crate::core::types::nudge::{Nudge, NudgeKind};
use crate::core::types::tools::upcoming::UpcomingItem;
use crate::core::types::tools::{REMEMBER, REMIND};

/// Tool of the Canvas upcoming source; only its items count as new assignments.
pub const CANVAS_TOOL: &str = "canvas_deadlines";

/// Most items one brief or plan lists.
const MAX_LISTED: usize = 8;

/// What the rules read about one student at one moment.
pub struct Facts<'a> {
    /// Now.
    pub now: DateTime<Utc>,
    /// The student's local time zone.
    pub offset: FixedOffset,
    /// Dated items from every upcoming source, with the tool of the source.
    pub items: &'a [(&'static str, UpcomingItem)],
    /// What Sparky remembers about the student.
    pub memories: &'a [Memory],
    /// Keys already sent to the student.
    pub sent: &'a HashSet<String>,
}

impl Facts<'_> {
    /// Now in local time.
    pub fn local(&self) -> DateTime<FixedOffset> {
        self.now.with_timezone(&self.offset)
    }

    /// Items due after now and within hours.
    fn within(&self, hours: i64) -> impl Iterator<Item = &(&'static str, UpcomingItem)> {
        let until = self.now + Duration::hours(hours);
        self.items
            .iter()
            .filter(move |(_, i)| i.at > self.now && i.at <= until)
    }

    /// Whether key was sent.
    fn was_sent(&self, key: &str) -> bool {
        self.sent.contains(key)
    }
}

/// The settings the rules use.
#[derive(Debug, Clone)]
pub struct Rules {
    /// Local hour the weekday brief goes out from.
    pub brief_hour: u32,
    /// Days ahead the brief and the plan cover.
    pub brief_days: i64,
    /// Local hour the Sunday plan goes out from.
    pub week_plan_hour: u32,
    /// Hours before a deadline the due soon nudge goes out.
    pub due_soon_hours: i64,
    /// Days before a big assignment the start early nudge goes out.
    pub start_early_days: i64,
    /// Lowercase words that make an assignment big.
    pub big_work: Vec<String>,
    /// Days ahead a new assignment is looked for.
    pub new_item_days: i64,
    /// Days after a plan is saved that the follow up goes out.
    pub follow_up_days: i64,
    /// A plan older than this gets no follow up.
    pub follow_up_max_days: i64,
}

/// What a rule wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send this nudge.
    Send(Nudge),
    /// Mark these keys sent without saying anything.
    Mark(NudgeKind, Vec<String>),
}

/// A short stable key for an item: FNV-1a over its source, title and time.
pub fn item_key(prefix: &str, tool: &str, item: &UpcomingItem) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in tool
        .bytes()
        .chain(item.title.bytes())
        .chain(item.at.timestamp().to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{prefix}:{hash:016x}")
}

/// Every key the item rules may ask about, so the store is asked once.
pub fn candidate_keys(f: &Facts<'_>, r: &Rules) -> Vec<String> {
    let mut keys = Vec::new();
    for (tool, item) in f.items {
        keys.push(item_key("due", tool, item));
        keys.push(item_key("early", tool, item));
        if *tool == CANVAS_TOOL {
            keys.push(item_key("new", tool, item));
        }
    }
    keys.push(baseline_key(CANVAS_TOOL));
    let local = f.local();
    keys.push(day_key("brief", local.date_naive()));
    keys.push(week_key("week", local.date_naive()));
    for m in follow_ups(f, r) {
        keys.push(follow_key(m));
    }
    keys
}

/// The key that says a source's items were seen once.
fn baseline_key(tool: &str) -> String {
    format!("new:baseline:{tool}")
}

/// A key for one local day.
pub fn day_key(prefix: &str, day: NaiveDate) -> String {
    format!("{prefix}:{day}")
}

/// A key for the local week that day is in.
pub fn week_key(prefix: &str, day: NaiveDate) -> String {
    let week = day.iso_week();
    format!("{prefix}:{}-W{:02}", week.year(), week.week())
}

/// The key of a follow up on one memory.
fn follow_key(m: &Memory) -> String {
    format!("follow:{}", m.short_id())
}

/// When an item is due, as a student says it: today at 11:59 PM, tomorrow, Fri Nov 7 at 5 PM.
pub fn when(item: &UpcomingItem, now: DateTime<Utc>, offset: FixedOffset) -> String {
    let at = item.at.with_timezone(&offset);
    let today = now.with_timezone(&offset).date_naive();
    let day = at.date_naive();
    let name = if day == today {
        "today".to_owned()
    } else if Some(day) == today.succ_opt() {
        "tomorrow".to_owned()
    } else {
        at.format("%a %b %-d").to_string()
    };
    if item.all_day {
        return name;
    }
    let time = if at.minute() == 0 {
        at.format("%-I %p").to_string()
    } else {
        at.format("%-I:%M %p").to_string()
    };
    format!("{name} at {time}")
}

/// An item as one line: title, course, and when.
fn line(item: &UpcomingItem, f: &Facts<'_>) -> String {
    let course = item
        .detail
        .as_deref()
        .map(|d| format!(" ({d})"))
        .unwrap_or_default();
    format!("{}{course}, {}", item.title, when(item, f.now, f.offset))
}

/// A nudge for each item due within due_soon_hours.
pub fn due_soon(f: &Facts<'_>, r: &Rules) -> Vec<Action> {
    // A reminder fires on its own, so it gets no second nudge.
    f.within(r.due_soon_hours)
        .filter(|(tool, i)| *tool != REMIND && !f.was_sent(&item_key("due", tool, i)))
        .map(|(tool, i)| {
            Action::Send(Nudge {
                kind: NudgeKind::DueSoon,
                keys: vec![item_key("due", tool, i)],
                text: format!("Heads up: {} is due {}.", i.title, when(i, f.now, f.offset)),
            })
        })
        .collect()
}

/// One nudge listing Canvas items that were not there before. The first look only remembers them.
pub fn new_items(f: &Facts<'_>, r: &Rules) -> Vec<Action> {
    let canvas: Vec<&UpcomingItem> = f
        .within(r.new_item_days * 24)
        .filter(|(tool, _)| *tool == CANVAS_TOOL)
        .map(|(_, i)| i)
        .collect();
    let keys = |items: &[&UpcomingItem]| -> Vec<String> {
        items
            .iter()
            .map(|i| item_key("new", CANVAS_TOOL, i))
            .collect()
    };
    let baseline = baseline_key(CANVAS_TOOL);
    if !f.was_sent(&baseline) {
        let mut all = keys(&canvas);
        all.push(baseline);
        return vec![Action::Mark(NudgeKind::NewItem, all)];
    }
    let fresh: Vec<&UpcomingItem> = canvas
        .into_iter()
        .filter(|i| !f.was_sent(&item_key("new", CANVAS_TOOL, i)))
        .collect();
    if fresh.is_empty() {
        return Vec::new();
    }
    let listed: Vec<String> = fresh.iter().take(MAX_LISTED).map(|i| line(i, f)).collect();
    let text = match listed.as_slice() {
        [one] => format!("New on Canvas: {one}."),
        many => format!("New on Canvas:\n- {}", many.join("\n- ")),
    };
    vec![Action::Send(Nudge {
        kind: NudgeKind::NewItem,
        keys: keys(&fresh),
        text,
    })]
}

/// A nudge for a big assignment or exam start_early_days out, in the daytime.
pub fn start_early(f: &Facts<'_>, r: &Rules) -> Vec<Action> {
    let hour = f.local().hour();
    if !(9..21).contains(&hour) {
        return Vec::new();
    }
    let from = f.now + Duration::days(r.start_early_days - 1);
    let until = f.now + Duration::days(r.start_early_days);
    f.items
        .iter()
        .filter(|(tool, i)| {
            *tool != REMEMBER
                && *tool != REMIND
                && i.at > from
                && i.at <= until
                && is_big(&i.title, &r.big_work)
                && !f.was_sent(&item_key("early", tool, i))
        })
        .take(1)
        .map(|(tool, i)| {
            Action::Send(Nudge {
                kind: NudgeKind::StartEarly,
                keys: vec![item_key("early", tool, i)],
                text: format!(
                    "{} is due {}. Starting it today makes the week a lot easier. Want me to set a reminder to work on it?",
                    i.title,
                    when(i, f.now, f.offset)
                ),
            })
        })
        .collect()
}

/// Whether a title names big work.
fn is_big(title: &str, words: &[String]) -> bool {
    let lower = title.to_lowercase();
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| words.iter().any(|big| w == big || w == format!("{big}s")))
}

/// Plans with no date that are old enough for a follow up and not too old.
fn follow_ups<'a>(f: &'a Facts<'_>, r: &Rules) -> impl Iterator<Item = &'a Memory> {
    let newest = f.now - Duration::days(r.follow_up_days);
    let oldest = f.now - Duration::days(r.follow_up_max_days);
    f.memories.iter().filter(move |m| {
        m.kind == MemoryKind::Task
            && m.expires_at.is_none()
            && m.created_at <= newest
            && m.created_at >= oldest
    })
}

/// A check in on one plan the student mentioned, in the daytime.
pub fn follow_up(f: &Facts<'_>, r: &Rules) -> Vec<Action> {
    let hour = f.local().hour();
    if !(10..20).contains(&hour) {
        return Vec::new();
    }
    follow_ups(f, r)
        .filter(|m| !f.was_sent(&follow_key(m)))
        .take(1)
        .map(|m| {
            Action::Send(Nudge {
                kind: NudgeKind::FollowUp,
                keys: vec![follow_key(m)],
                text: format!(
                    "A few days ago you mentioned: {}. How did it go? I can help with the next step.",
                    m.content.trim_end_matches('.')
                ),
            })
        })
        .collect()
}

/// The weekday morning brief: what is due today and this week.
pub fn brief(f: &Facts<'_>, r: &Rules) -> Vec<Action> {
    let local = f.local();
    let weekday = !matches!(local.weekday(), Weekday::Sat | Weekday::Sun);
    let key = day_key("brief", local.date_naive());
    if !weekday || !(r.brief_hour..r.brief_hour + 3).contains(&local.hour()) || f.was_sent(&key) {
        return Vec::new();
    }
    let today = local.date_naive();
    let items: Vec<&UpcomingItem> = f.within(r.brief_days * 24).map(|(_, i)| i).collect();
    if items.is_empty() {
        return Vec::new();
    }
    let (now, later): (Vec<&UpcomingItem>, Vec<&UpcomingItem>) = items
        .into_iter()
        .partition(|i| i.at.with_timezone(&f.offset).date_naive() == today);
    let mut text = String::from("Good morning!");
    if now.is_empty() {
        text.push_str(" Nothing is due today.");
    } else {
        text.push_str(" Due today:");
        for i in now.iter().take(MAX_LISTED) {
            let _ = write!(text, "\n- {}", line(i, f));
        }
    }
    if !later.is_empty() {
        text.push_str("\nLater this week:");
        for i in later.iter().take(MAX_LISTED.saturating_sub(now.len())) {
            let _ = write!(text, "\n- {}", line(i, f));
        }
    }
    vec![Action::Send(Nudge {
        kind: NudgeKind::Brief,
        keys: vec![key],
        text,
    })]
}

/// The Sunday evening plan: the week ahead, day by day.
pub fn week_plan(f: &Facts<'_>, r: &Rules) -> Vec<Action> {
    let local = f.local();
    let next = local.date_naive() + Duration::days(1);
    let key = week_key("week", next);
    if local.weekday() != Weekday::Sun || local.hour() < r.week_plan_hour || f.was_sent(&key) {
        return Vec::new();
    }
    let mut items: Vec<&UpcomingItem> = f.within(r.brief_days * 24).map(|(_, i)| i).collect();
    if items.is_empty() {
        return Vec::new();
    }
    items.sort_by_key(|i| i.at);
    let mut text = String::from("Your week ahead:");
    let mut day: Option<NaiveDate> = None;
    for i in items.iter().take(MAX_LISTED) {
        let on = i.at.with_timezone(&f.offset).date_naive();
        if day != Some(on) {
            let _ = write!(text, "\n{}", on.format("%A"));
            day = Some(on);
        }
        let _ = write!(text, "\n- {}", line(i, f));
    }
    if items.len() > MAX_LISTED {
        let _ = write!(text, "\nand {} more.", items.len() - MAX_LISTED);
    }
    vec![Action::Send(Nudge {
        kind: NudgeKind::WeekPlan,
        keys: vec![key],
        text,
    })]
}

/// Every rule in the order nudges are worth sending: the most urgent first.
pub fn all(f: &Facts<'_>, r: &Rules) -> Vec<Action> {
    let mut out = Vec::new();
    out.extend(due_soon(f, r));
    out.extend(new_items(f, r));
    out.extend(start_early(f, r));
    out.extend(follow_up(f, r));
    out.extend(brief(f, r));
    out.extend(week_plan(f, r));
    out
}
