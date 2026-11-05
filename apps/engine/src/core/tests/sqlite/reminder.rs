//! Reminders in the file: sealed at rest, fired into notifications, repeats moved on, kept per user.

use chrono::{Duration, Utc};
use sqlx::Row;

use super::{seal, store};
use crate::core::traits::reminder::ReminderStore;
use crate::core::types::reminder::{NewReminder, Repeat};
use crate::stores::sqlite::ReminderLimits;

const LIMITS: ReminderLimits = ReminderLimits {
    max_per_user: 2,
    keep_days: 30,
};

fn soon(text: &str, minutes: i64, repeat: Repeat) -> NewReminder {
    NewReminder {
        text: text.into(),
        due_at: Utc::now() + Duration::minutes(minutes),
        repeat,
    }
}

#[tokio::test]
async fn a_reminder_is_sealed_and_listed_only_for_its_user() {
    let store = store().await;
    let reminders = store.reminders(seal(), LIMITS);
    let kept = reminders
        .add("g", "111", &soon("Call the advisor", 5, Repeat::Once))
        .await;
    assert!(matches!(kept, Ok(Some(_))));
    let listed = reminders.list("g", "111").await.unwrap_or_default();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].text, "Call the advisor");
    assert_eq!(reminders.list("g", "222").await.unwrap_or_default(), vec![]);
    let Ok(row) = sqlx::query("SELECT sealed FROM reminders")
        .fetch_one(store.pool())
        .await
    else {
        unreachable!("the row is there")
    };
    let sealed: Vec<u8> = row.try_get("sealed").unwrap_or_default();
    assert!(!String::from_utf8_lossy(&sealed).contains("advisor"));
}

#[tokio::test]
async fn a_user_keeps_at_most_the_limit() {
    let store = store().await;
    let reminders = store.reminders(seal(), LIMITS);
    for n in 0..2 {
        assert!(matches!(
            reminders
                .add("g", "111", &soon(&format!("r{n}"), 5, Repeat::Once))
                .await,
            Ok(Some(_))
        ));
    }
    assert!(matches!(
        reminders
            .add("g", "111", &soon("one more", 5, Repeat::Once))
            .await,
        Ok(None)
    ));
}

#[tokio::test]
async fn a_due_reminder_becomes_a_notification_and_a_repeat_moves_on() {
    let store = store().await;
    let reminders = store.reminders(seal(), LIMITS);
    let _ = reminders
        .add("g", "111", &soon("Drink water", 1, Repeat::Once))
        .await;
    let _ = reminders
        .add("g", "111", &soon("Gym", 2, Repeat::Daily))
        .await;
    let later = Utc::now() + Duration::minutes(10);
    assert!(matches!(reminders.fire(later).await, Ok(2)));
    let left = reminders.list("g", "111").await.unwrap_or_default();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].text, "Gym");
    assert!(left[0].due_at > later);
    let shown = reminders
        .notifications("g", "111", 10)
        .await
        .unwrap_or_default();
    assert_eq!(shown.len(), 2);
    assert!(shown.iter().all(|n| !n.read));
    assert!(matches!(reminders.fire(later).await, Ok(0)));
}

#[tokio::test]
async fn the_bot_sees_each_notification_until_it_says_sent_and_reading_marks_the_web_list() {
    let store = store().await;
    let reminders = store.reminders(seal(), LIMITS);
    let _ = reminders
        .add("g", "111", &soon("Essay", 1, Repeat::Once))
        .await;
    let _ = reminders.fire(Utc::now() + Duration::minutes(5)).await;
    let out = reminders.outgoing(10).await.unwrap_or_default();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].user_id, "111");
    assert_eq!(out[0].text, "Essay");
    assert!(reminders.sent(&[out[0].id.clone()]).await.is_ok());
    assert_eq!(reminders.outgoing(10).await.unwrap_or_default(), vec![]);
    assert!(reminders.read_all("g", "111").await.is_ok());
    let shown = reminders
        .notifications("g", "111", 10)
        .await
        .unwrap_or_default();
    assert!(shown.iter().all(|n| n.read));
}

#[tokio::test]
async fn cancel_removes_only_the_users_own_reminder() {
    let store = store().await;
    let reminders = store.reminders(seal(), LIMITS);
    let Ok(Some(kept)) = reminders
        .add("g", "111", &soon("Lab", 5, Repeat::Once))
        .await
    else {
        unreachable!("kept")
    };
    assert!(matches!(
        reminders.cancel("g", "222", &kept.id).await,
        Ok(false)
    ));
    assert!(matches!(
        reminders.cancel("g", "111", &kept.id).await,
        Ok(true)
    ));
    assert_eq!(reminders.list("g", "111").await.unwrap_or_default(), vec![]);
}

#[test]
fn a_repeat_lands_on_the_first_time_after_now() {
    let at = Utc::now() - Duration::hours(50);
    let now = Utc::now();
    let Some(next) = Repeat::Daily.next_after(at, now) else {
        unreachable!("daily repeats")
    };
    assert!(next > now && next <= now + Duration::days(1));
    assert_eq!(Repeat::Once.next_after(at, now), None);
}
