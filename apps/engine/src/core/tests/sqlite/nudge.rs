//! Nudges in the file: sent keys, settings, the daily count, feedback, and who is active.

use chrono::{Duration, Utc};

use super::{caller, seal, store};
use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::nudge::NudgeStore;
use crate::core::traits::reminder::ReminderStore;
use crate::core::types::conversation::Visibility;
use crate::core::types::nudge::{Nudge, NudgeKind, NudgePrefs, Person};
use crate::stores::sqlite::ReminderLimits;

fn me() -> Person {
    Person {
        tenant: "guild".into(),
        user: "111".into(),
    }
}

fn nudge(kind: NudgeKind, key: &str, text: &str) -> Nudge {
    Nudge {
        kind,
        keys: vec![key.into()],
        text: text.into(),
    }
}

#[tokio::test]
async fn a_sent_nudge_is_a_notification_and_its_key_is_kept() {
    let store = store().await;
    let nudges = store.nudges(seal());
    let reminders = store.reminders(
        seal(),
        ReminderLimits {
            max_per_user: 5,
            keep_days: 30,
        },
    );
    let sent = nudges
        .send(
            &me(),
            &nudge(NudgeKind::DueSoon, "due:1", "Essay is due today."),
        )
        .await;
    assert!(sent.is_ok(), "{sent:?}");
    let Ok(list) = reminders.notifications("guild", "111", 10).await else {
        unreachable!("notifications list")
    };
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].text, "Essay is due today.");
    assert_eq!(list[0].kind, "due_soon");
    let keys = vec!["due:1".to_owned(), "due:2".to_owned()];
    assert_eq!(
        nudges.sent_keys(&me(), &keys).await.ok(),
        Some(vec!["due:1".to_owned()])
    );
    let marked = nudges
        .mark(&me(), NudgeKind::NewItem, &["due:2".to_owned()])
        .await;
    assert!(marked.is_ok());
    assert_eq!(nudges.sent_keys(&me(), &keys).await.ok(), Some(keys));
}

#[tokio::test]
async fn the_daily_count_leaves_out_reminders_and_older_nudges() {
    let store = store().await;
    let nudges = store.nudges(seal());
    let now = Utc::now();
    let _ = nudges
        .send(&me(), &nudge(NudgeKind::Brief, "b", "Good morning"))
        .await;
    assert_eq!(
        nudges
            .count_since(&me(), now - Duration::hours(1))
            .await
            .ok(),
        Some(1)
    );
    assert_eq!(
        nudges
            .count_since(&me(), now + Duration::hours(1))
            .await
            .ok(),
        Some(0)
    );
    let other = Person {
        tenant: "guild".into(),
        user: "222".into(),
    };
    assert_eq!(
        nudges
            .count_since(&other, now - Duration::hours(1))
            .await
            .ok(),
        Some(0)
    );
}

#[tokio::test]
async fn settings_round_trip_and_start_empty() {
    let store = store().await;
    let nudges = store.nudges(seal());
    assert_eq!(nudges.prefs(&me()).await.ok(), Some(None));
    let prefs = NudgePrefs {
        quiet_start: 23,
        quiet_end: 7,
        off: vec![NudgeKind::Event, NudgeKind::Brief],
    };
    assert!(nudges.set_prefs(&me(), &prefs).await.is_ok());
    assert_eq!(nudges.prefs(&me()).await.ok(), Some(Some(prefs)));
}

#[tokio::test]
async fn feedback_names_the_kind_of_the_nudge_it_was_on() {
    let store = store().await;
    let nudges = store.nudges(seal());
    let reminders = store.reminders(
        seal(),
        ReminderLimits {
            max_per_user: 5,
            keep_days: 30,
        },
    );
    let _ = nudges
        .send(
            &me(),
            &nudge(NudgeKind::Event, "event:x", "Hackathon Friday"),
        )
        .await;
    let Ok(list) = reminders.notifications("guild", "111", 10).await else {
        unreachable!("notifications list")
    };
    let id = list[0].id.clone();
    let other = Person {
        tenant: "guild".into(),
        user: "222".into(),
    };
    assert_eq!(
        nudges.feedback(&other, &id, false).await.ok(),
        Some(None),
        "only the owner"
    );
    assert_eq!(
        nudges.feedback(&me(), &id, false).await.ok(),
        Some(Some(NudgeKind::Event))
    );
    let Ok(list) = reminders.notifications("guild", "111", 10).await else {
        unreachable!("notifications list")
    };
    assert_eq!(list[0].helpful, Some(false));
}

#[tokio::test]
async fn people_are_students_with_a_recent_private_chat() {
    let store = store().await;
    let conversations = store.conversations();
    let mut private = caller();
    private.visibility = Visibility::Private;
    assert!(conversations.ensure(&private, "dm").await.is_ok());
    let mut public = crate::core::types::agent::context::RequestContext::new(
        "guild",
        "333",
        std::time::Duration::from_secs(5),
    );
    public.visibility = Visibility::Public;
    assert!(conversations.ensure(&public, "general").await.is_ok());
    let nudges = store.nudges(seal());
    let Ok(people) = nudges.people(Utc::now() - Duration::days(1)).await else {
        unreachable!("people list")
    };
    assert_eq!(people, vec![me()]);
    let Ok(later) = nudges.people(Utc::now() + Duration::days(1)).await else {
        unreachable!("people list")
    };
    assert_eq!(later.len(), 0);
}

#[tokio::test]
async fn old_keys_are_pruned() {
    let store = store().await;
    let nudges = store.nudges(seal());
    let _ = nudges
        .mark(&me(), NudgeKind::Brief, &["brief:old".to_owned()])
        .await;
    assert_eq!(
        nudges.prune(Utc::now() + Duration::seconds(5)).await.ok(),
        Some(1)
    );
    assert_eq!(
        nudges
            .sent_keys(&me(), &["brief:old".to_owned()])
            .await
            .ok(),
        Some(Vec::new())
    );
}
