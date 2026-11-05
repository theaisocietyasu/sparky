//! The rules over fixed moments: what each one says, and when it stays quiet.

use std::collections::HashSet;

use chrono::{DateTime, Duration, FixedOffset, TimeZone, Utc};
use uuid::Uuid;

use crate::core::types::memory::{Memory, MemoryKind};
use crate::core::types::nudge::NudgeKind;
use crate::core::types::tools::upcoming::UpcomingItem;
use crate::runtime::nudge::rules::{
    self, Action, CANVAS_TOOL, Facts, Rules, day_key, item_key, when,
};

fn phoenix() -> FixedOffset {
    FixedOffset::west_opt(7 * 3600).unwrap_or_else(|| unreachable!("a valid offset"))
}

/// A local time in Phoenix on a given day of November 2026.
fn at(day: u32, hour: u32) -> DateTime<Utc> {
    match phoenix().with_ymd_and_hms(2026, 11, day, hour, 0, 0) {
        chrono::LocalResult::Single(t) => t.with_timezone(&Utc),
        _ => unreachable!("a valid time"),
    }
}

fn item(title: &str, due: DateTime<Utc>) -> UpcomingItem {
    UpcomingItem {
        at: due,
        all_day: false,
        title: title.into(),
        detail: Some("CSE 310".into()),
        url: None,
    }
}

fn rules() -> Rules {
    Rules {
        brief_hour: 8,
        brief_days: 7,
        week_plan_hour: 18,
        due_soon_hours: 24,
        start_early_days: 3,
        big_work: vec!["exam".into(), "project".into()],
        new_item_days: 21,
        follow_up_days: 3,
        follow_up_max_days: 10,
    }
}

fn facts<'a>(
    now: DateTime<Utc>,
    items: &'a [(&'static str, UpcomingItem)],
    memories: &'a [Memory],
    sent: &'a HashSet<String>,
) -> Facts<'a> {
    Facts {
        now,
        offset: phoenix(),
        items,
        memories,
        sent,
    }
}

fn sent_kinds(actions: &[Action]) -> Vec<NudgeKind> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Send(n) => Some(n.kind),
            Action::Mark(..) => None,
        })
        .collect()
}

fn texts(actions: &[Action]) -> Vec<String> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::Send(n) => Some(n.text.clone()),
            Action::Mark(..) => None,
        })
        .collect()
}

#[test]
fn due_times_read_the_way_a_student_says_them() {
    // Thursday Nov 5 2026, 9 AM in Phoenix.
    let now = at(5, 9);
    assert_eq!(
        when(
            &item("a", at(5, 23) + Duration::minutes(59)),
            now,
            phoenix()
        ),
        "today at 11:59 PM"
    );
    assert_eq!(
        when(&item("a", at(6, 17)), now, phoenix()),
        "tomorrow at 5 PM"
    );
    assert_eq!(
        when(&item("a", at(9, 12)), now, phoenix()),
        "Mon Nov 9 at 12 PM"
    );
}

#[test]
fn something_due_within_a_day_is_said_once() {
    let now = at(5, 9);
    let items = [(CANVAS_TOOL, item("Lab 4", at(5, 23)))];
    let empty = HashSet::new();
    let out = rules::due_soon(&facts(now, &items, &[], &empty), &rules());
    assert_eq!(texts(&out), ["Heads up: Lab 4 is due today at 11 PM."]);
    let sent: HashSet<String> = [item_key("due", CANVAS_TOOL, &items[0].1)].into();
    assert_eq!(
        rules::due_soon(&facts(now, &items, &[], &sent), &rules()).len(),
        0
    );
}

#[test]
fn a_reminder_gets_no_second_nudge() {
    let now = at(5, 9);
    let items = [("remind", item("Call advisor", at(5, 15)))];
    let empty = HashSet::new();
    assert_eq!(
        rules::due_soon(&facts(now, &items, &[], &empty), &rules()).len(),
        0
    );
}

#[test]
fn the_first_look_at_canvas_only_remembers_what_is_there() {
    let now = at(5, 9);
    let items = [(CANVAS_TOOL, item("HW 1", at(12, 23)))];
    let empty = HashSet::new();
    let out = rules::new_items(&facts(now, &items, &[], &empty), &rules());
    assert!(matches!(out.as_slice(), [Action::Mark(NudgeKind::NewItem, keys)] if keys.len() == 2));
    let mut seen: HashSet<String> = ["new:baseline:canvas_deadlines".to_owned()].into();
    let out = rules::new_items(&facts(now, &items, &[], &seen), &rules());
    assert_eq!(
        texts(&out),
        ["New on Canvas: HW 1 (CSE 310), Thu Nov 12 at 11 PM."]
    );
    seen.insert(item_key("new", CANVAS_TOOL, &items[0].1));
    assert_eq!(
        rules::new_items(&facts(now, &items, &[], &seen), &rules()).len(),
        0
    );
}

#[test]
fn big_work_a_few_days_out_gets_a_start_early_nudge_in_the_day() {
    let items = [
        (CANVAS_TOOL, item("Midterm exam", at(7, 22))),
        (CANVAS_TOOL, item("Reading quiz", at(7, 22))),
    ];
    let empty = HashSet::new();
    let out = rules::start_early(&facts(at(5, 9), &items, &[], &empty), &rules());
    assert_eq!(sent_kinds(&out), [NudgeKind::StartEarly]);
    assert!(texts(&out)[0].starts_with("Midterm exam is due"));
    assert_eq!(
        rules::start_early(&facts(at(5, 6), &items, &[], &empty), &rules()).len(),
        0,
        "not at dawn"
    );
}

#[test]
fn a_plan_with_no_date_gets_one_follow_up_after_a_few_days() {
    let now = at(9, 12);
    let plan = |days: i64| Memory {
        id: Uuid::new_v4(),
        kind: MemoryKind::Task,
        content: "apply to the Intel internship".into(),
        sensitive: false,
        created_at: now - Duration::days(days),
        expires_at: None,
    };
    let empty = HashSet::new();
    let fresh = [plan(1)];
    assert_eq!(
        rules::follow_up(&facts(now, &[], &fresh, &empty), &rules()).len(),
        0,
        "too soon"
    );
    let stale = [plan(20)];
    assert_eq!(
        rules::follow_up(&facts(now, &[], &stale, &empty), &rules()).len(),
        0,
        "too old"
    );
    let ready = [plan(4)];
    let out = rules::follow_up(&facts(now, &[], &ready, &empty), &rules());
    assert_eq!(
        texts(&out),
        [
            "A few days ago you mentioned: apply to the Intel internship. How did it go? I can help with the next step."
        ]
    );
}

#[test]
fn the_brief_goes_out_on_weekday_mornings_with_something_to_say() {
    let items = [
        (CANVAS_TOOL, item("Lab 4", at(5, 23))),
        (CANVAS_TOOL, item("Project 2", at(9, 23))),
    ];
    let empty = HashSet::new();
    let out = rules::brief(&facts(at(5, 8), &items, &[], &empty), &rules());
    let text = texts(&out).join("");
    assert!(
        text.starts_with("Good morning! Due today:\n- Lab 4"),
        "{text}"
    );
    assert!(text.contains("Later this week:\n- Project 2"), "{text}");
    assert_eq!(
        rules::brief(&facts(at(5, 13), &items, &[], &empty), &rules()).len(),
        0,
        "afternoon"
    );
    assert_eq!(
        rules::brief(&facts(at(7, 8), &items, &[], &empty), &rules()).len(),
        0,
        "Saturday"
    );
    assert_eq!(
        rules::brief(&facts(at(5, 8), &[], &[], &empty), &rules()).len(),
        0,
        "nothing due"
    );
    let sent: HashSet<String> = [day_key(
        "brief",
        at(5, 8).with_timezone(&phoenix()).date_naive(),
    )]
    .into();
    assert_eq!(
        rules::brief(&facts(at(5, 8), &items, &[], &sent), &rules()).len(),
        0,
        "once a day"
    );
}

#[test]
fn the_sunday_plan_lists_the_week_by_day() {
    // Sunday Nov 8 2026, 6 PM.
    let items = [
        (CANVAS_TOOL, item("Lab 5", at(9, 23))),
        (CANVAS_TOOL, item("Essay", at(11, 12))),
    ];
    let empty = HashSet::new();
    let out = rules::week_plan(&facts(at(8, 18), &items, &[], &empty), &rules());
    let text = texts(&out).join("");
    assert!(
        text.starts_with("Your week ahead:\nMonday\n- Lab 5"),
        "{text}"
    );
    assert!(text.contains("Wednesday\n- Essay"), "{text}");
    assert_eq!(
        rules::week_plan(&facts(at(8, 10), &items, &[], &empty), &rules()).len(),
        0,
        "morning"
    );
    assert_eq!(
        rules::week_plan(&facts(at(9, 18), &items, &[], &empty), &rules()).len(),
        0,
        "Monday"
    );
}
