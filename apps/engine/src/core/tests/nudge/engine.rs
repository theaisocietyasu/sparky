//! The nudge loop over a real store: quiet time, the daily cap, switched off kinds, and background runs.

use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

use async_trait::async_trait;
use chrono::{DateTime, Duration, FixedOffset, Utc};

use crate::core::traits::nudge::{Background, NudgeStore};
use crate::core::traits::reminder::ReminderStore;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::nudge::{NudgeKind, NudgePrefs, Person};
use crate::core::types::tools::upcoming::UpcomingItem;
use crate::runtime::nudge::rules::{CANVAS_TOOL, Rules};
use crate::runtime::nudge::{Nudger, Settings};
use crate::stores::sqlite::{ReminderLimits, Seal, SqliteStore, StoreOptions};

struct Canvas(Vec<UpcomingItem>);

#[async_trait]
impl UpcomingSource for Canvas {
    fn label(&self) -> &'static str {
        "Canvas"
    }
    fn tool(&self) -> &'static str {
        CANVAS_TOOL
    }
    async fn upcoming(
        &self,
        _ctx: &RequestContext,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<UpcomingItem>, String> {
        Ok(self
            .0
            .iter()
            .filter(|i| i.at >= from && i.at <= until)
            .cloned()
            .collect())
    }
}

/// A background agent that writes a fixed answer and counts its runs.
struct Writes {
    answer: String,
    runs: Mutex<usize>,
}

#[async_trait]
impl Background for Writes {
    async fn ask(&self, _ctx: &RequestContext, _instruction: &str) -> Result<String, String> {
        if let Ok(mut runs) = self.runs.lock() {
            *runs += 1;
        }
        Ok(self.answer.clone())
    }
}

fn me() -> Person {
    Person {
        tenant: "guild".into(),
        user: "111".into(),
    }
}

async fn open() -> SqliteStore {
    let options = StoreOptions {
        path: "sqlite::memory:".into(),
        max_connections: 1,
        busy_timeout: StdDuration::from_secs(1),
    };
    match SqliteStore::open(&options).await {
        Ok(store) => store,
        Err(e) => unreachable!("an in-memory store opens: {e}"),
    }
}

fn settings(cap: usize) -> Settings {
    Settings {
        rules: Rules {
            brief_hour: 8,
            brief_days: 7,
            week_plan_hour: 18,
            due_soon_hours: 24,
            start_early_days: 3,
            big_work: vec!["exam".into()],
            new_item_days: 21,
            follow_up_days: 3,
            follow_up_max_days: 10,
            ..Rules::default()
        },
        active_days: 30,
        daily_cap: cap,
        // No quiet time, so the tests run at any hour.
        defaults: NudgePrefs {
            quiet_start: 0,
            quiet_end: 0,
            off: Vec::new(),
        },
        offset: FixedOffset::east_opt(0).unwrap_or_else(|| unreachable!("a valid offset")),
        event_hour: 0,
        study_hour: 0,
        calendar_weekday: 1,
        calendar_hour: 0,
        ask_timeout: StdDuration::from_secs(5),
        keep_days: 30,
        nothing: "NONE".into(),
        event_prompt: "events {nothing}".into(),
        calendar_prompt: "calendar {already} {nothing}".into(),
        study_prompt: "study {item} {due} {nothing}".into(),
        // 0 never runs the openings check.
        opportunity_weekday: 0,
        opportunity_hour: 0,
        opportunity_prompt: "openings {already} {nothing}".into(),
    }
}

fn due_in(hours: i64, title: &str) -> UpcomingItem {
    due_at(Utc::now() + Duration::hours(hours), title)
}

fn due_at(at: DateTime<Utc>, title: &str) -> UpcomingItem {
    UpcomingItem {
        at,
        all_day: false,
        title: title.into(),
        detail: None,
        url: None,
    }
}

/// Noon UTC today, inside the hours the study check runs.
fn noon_today() -> DateTime<Utc> {
    Utc::now()
        .date_naive()
        .and_hms_opt(12, 0, 0)
        .map_or_else(Utc::now, |t| t.and_utc())
}

async fn kinds(store: &SqliteStore) -> Vec<String> {
    let reminders = store.reminders(
        Seal::new(&[7; 32]),
        ReminderLimits {
            max_per_user: 5,
            keep_days: 30,
        },
    );
    let Ok(list) = reminders.notifications("guild", "111", 50).await else {
        unreachable!("notifications list")
    };
    list.into_iter().map(|n| n.kind).collect()
}

fn nudger(
    store: &SqliteStore,
    items: Vec<UpcomingItem>,
    bg: Option<Arc<Writes>>,
    cap: usize,
) -> Nudger {
    Nudger::new(
        Arc::new(store.nudges(Seal::new(&[7; 32]))),
        vec![Arc::new(Canvas(items))],
        None,
        bg.map(|b| b as Arc<dyn Background>),
        settings(cap),
    )
}

#[tokio::test]
async fn a_deadline_is_said_once_across_passes() {
    let store = open().await;
    let n = nudger(&store, vec![due_in(5, "Lab 4")], None, 10);
    let _ = n.person(&me(), Utc::now()).await;
    let _ = n.person(&me(), Utc::now()).await;
    let due = kinds(&store)
        .await
        .iter()
        .filter(|k| *k == "due_soon")
        .count();
    assert_eq!(due, 1);
}

#[tokio::test]
async fn the_daily_cap_holds_back_the_rest() {
    let store = open().await;
    let items = vec![due_in(2, "Lab 4"), due_in(3, "Lab 5"), due_in(4, "Lab 6")];
    let n = nudger(&store, items, None, 2);
    let _ = n.person(&me(), Utc::now()).await;
    let _ = n.person(&me(), Utc::now()).await;
    assert_eq!(kinds(&store).await.len(), 2);
}

#[tokio::test]
async fn quiet_time_and_switched_off_kinds_send_nothing() {
    let store = open().await;
    let nudges = store.nudges(Seal::new(&[7; 32]));
    let off = NudgePrefs {
        quiet_start: 0,
        quiet_end: 0,
        off: NudgeKind::ALL.to_vec(),
    };
    let _ = nudges.set_prefs(&me(), &off).await;
    let n = nudger(&store, vec![due_in(5, "Lab 4")], None, 10);
    assert_eq!(n.person(&me(), Utc::now()).await.ok(), Some(0));
    let quiet = NudgePrefs {
        quiet_start: 0,
        quiet_end: 23,
        off: Vec::new(),
    };
    let _ = nudges.set_prefs(&me(), &quiet).await;
    let late = Utc::now()
        .date_naive()
        .and_hms_opt(5, 0, 0)
        .map(|t| t.and_utc());
    let Some(late) = late else {
        unreachable!("a valid time")
    };
    assert_eq!(n.person(&me(), late).await.ok(), Some(0));
}

#[tokio::test]
async fn a_background_none_sends_nothing_and_is_not_asked_again() {
    let store = open().await;
    let bg = Arc::new(Writes {
        answer: "NONE".into(),
        runs: Mutex::new(0),
    });
    let noon = noon_today();
    let n = nudger(
        &store,
        vec![due_at(noon + Duration::hours(20), "Essay")],
        Some(Arc::clone(&bg)),
        10,
    );
    let _ = n.person(&me(), noon).await;
    let first = bg.runs.lock().map_or(0, |r| *r);
    assert!(first >= 1, "the study check runs before a deadline");
    let _ = n.person(&me(), noon).await;
    assert_eq!(
        bg.runs.lock().map_or(0, |r| *r),
        first,
        "marked, not asked again"
    );
    assert!(!kinds(&store).await.iter().any(|k| k == "study"));
}

#[tokio::test]
async fn a_background_answer_is_sent_as_its_kind() {
    let store = open().await;
    let bg = Arc::new(Writes {
        answer: "Hayden closes at 10 tonight; Noble is open until 2 AM.".into(),
        runs: Mutex::new(0),
    });
    let noon = noon_today();
    let n = nudger(
        &store,
        vec![due_at(noon + Duration::hours(20), "Essay")],
        Some(bg),
        10,
    );
    let _ = n.person(&me(), noon).await;
    assert!(kinds(&store).await.iter().any(|k| k == "study"));
}
