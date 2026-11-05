//! The remind rules: local times, relative times, and what they refuse.

use chrono::{Duration, FixedOffset, TimeZone, Utc};

use crate::core::types::reminder::Repeat;
use crate::runtime::tools::remind::{RemindArgs, RemindRules};

fn rules() -> RemindRules {
    RemindRules {
        max_chars: 40,
        max_days_ahead: 30,
        offset: FixedOffset::west_opt(7 * 3600).unwrap_or_else(|| unreachable!("in range")),
    }
}

fn args(text: &str, at: Option<&str>, in_minutes: Option<i64>) -> RemindArgs {
    RemindArgs {
        text: text.into(),
        at: at.map(str::to_owned),
        in_minutes,
        repeat: None,
    }
}

#[test]
fn a_local_time_is_read_at_the_offset() {
    let now = Utc
        .with_ymd_and_hms(2026, 11, 5, 12, 0, 0)
        .single()
        .unwrap_or_else(|| unreachable!("valid"));
    let Ok(kept) = rules().check(
        &args("Study for MAT 343", Some("2026-11-06 09:00"), None),
        now,
    ) else {
        unreachable!("a future time is kept")
    };
    assert_eq!(
        kept.due_at,
        Utc.with_ymd_and_hms(2026, 11, 6, 16, 0, 0)
            .single()
            .unwrap_or_else(|| unreachable!("valid"))
    );
    assert_eq!(kept.repeat, Repeat::Once);
    assert_eq!(rules().shown(kept.due_at), "Friday 6 November, 9:00 AM");
}

#[test]
fn minutes_from_now_count_from_now() {
    let now = Utc::now();
    let Ok(kept) = rules().check(&args("Stretch", None, Some(20)), now) else {
        unreachable!("kept")
    };
    assert_eq!(kept.due_at, now + Duration::minutes(20));
}

#[test]
fn it_refuses_the_past_too_far_too_long_and_both_or_neither_time() {
    let now = Utc::now();
    let r = rules();
    assert!(
        r.check(&args("x", Some("2020-01-01 09:00"), None), now)
            .is_err()
    );
    assert!(r.check(&args("x", None, Some(60 * 24 * 40)), now).is_err());
    assert!(r.check(&args(&"x".repeat(41), None, Some(5)), now).is_err());
    assert!(r.check(&args("x", None, None), now).is_err());
    assert!(
        r.check(&args("x", Some("2030-01-01 09:00"), Some(5)), now)
            .is_err()
    );
    assert!(r.check(&args("x", Some("tomorrow"), None), now).is_err());
    assert!(r.check(&args("  ", None, Some(5)), now).is_err());
}
