//! The home conversation: reminders and nudges land there, it stays open, and threads hang off it.

use std::time::Duration as StdDuration;

use chrono::{Duration, Utc};

use super::{seal, store};
use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::nudge::NudgeStore;
use crate::core::traits::reminder::ReminderStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::conversation::{Parent, Visibility, WEB_CHANNEL, home_id};
use crate::core::types::nudge::{Nudge, NudgeKind, Person};
use crate::core::types::reminder::{NewReminder, Repeat};
use crate::stores::sqlite::ReminderLimits;

const LIMITS: ReminderLimits = ReminderLimits {
    max_per_user: 5,
    keep_days: 30,
};

fn web(user: &str) -> RequestContext {
    RequestContext::new("g", user, StdDuration::from_secs(5)).with_visibility(Visibility::Private)
}

fn home(user: &str) -> RequestContext {
    web(user).with_conversation(home_id("g", user))
}

#[test]
fn the_home_id_is_one_per_student() {
    assert_eq!(home_id("g", "1"), home_id("g", "1"));
    assert_ne!(home_id("g", "1"), home_id("g", "2"));
    assert_ne!(home_id("g", "1"), home_id("h", "1"));
}

#[tokio::test]
async fn a_nudge_and_a_reminder_land_in_home_as_messages_of_sparky() {
    let store = store().await;
    let nudges = store.nudges(seal());
    let reminders = store.reminders(seal(), LIMITS);
    let conversations = store.conversations();
    let who = Person {
        tenant: "g".into(),
        user: "111".into(),
    };
    let nudge = Nudge {
        kind: NudgeKind::DueSoon,
        keys: vec!["due:1".into()],
        text: "Essay is due today.".into(),
    };
    assert!(nudges.send(&who, &nudge).await.is_ok());
    let soon = NewReminder {
        text: "Drink water".into(),
        due_at: Utc::now() + Duration::minutes(1),
        repeat: Repeat::Once,
    };
    let _ = reminders.add("g", "111", &soon).await;
    assert!(matches!(
        reminders.fire(Utc::now() + Duration::minutes(5)).await,
        Ok(1)
    ));

    let ctx = home("111");
    assert!(matches!(conversations.owns(&ctx).await, Ok(true)));
    let stored = conversations.load(&ctx, 10).await.unwrap_or_default();
    let said: Vec<(&str, &str)> = stored
        .iter()
        .filter_map(|s| {
            let notice = s.message.notice.as_ref()?;
            (s.message.role == Role::Assistant)
                .then_some((s.message.content.as_str(), notice.kind.as_str()))
        })
        .collect();
    assert_eq!(
        said,
        vec![
            ("Essay is due today.", "due_soon"),
            ("Drink water", "reminder")
        ]
    );
    let ids: Vec<String> = reminders
        .notifications("g", "111", 10)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert!(
        stored
            .iter()
            .filter_map(|s| s.message.notice.as_ref())
            .all(|n| ids.contains(&n.id))
    );
    // The student chats in home like any web conversation.
    assert!(conversations.ensure(&ctx, WEB_CHANNEL).await.is_ok());
    assert!(
        conversations
            .append(&ctx, &[Message::user("thanks")])
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn home_stays_open_and_out_of_the_list() {
    let store = store().await;
    let conversations = store.conversations();
    let ctx = home("111");
    assert!(conversations.ensure(&ctx, WEB_CHANNEL).await.is_ok());
    let _ = conversations.append(&ctx, &[Message::user("hi")]).await;
    let chat = web("111");
    assert!(conversations.ensure(&chat, WEB_CHANNEL).await.is_ok());
    let _ = conversations.append(&chat, &[Message::user("other")]).await;

    let listed = conversations
        .list(&chat, WEB_CHANNEL, 10)
        .await
        .unwrap_or_default();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "other");
    assert!(matches!(conversations.end(&chat, WEB_CHANNEL).await, Ok(1)));
    assert_eq!(
        conversations
            .latest(&chat, WEB_CHANNEL)
            .await
            .ok()
            .flatten(),
        Some(ctx.conversation_id)
    );
}

#[tokio::test]
async fn a_thread_names_a_message_of_the_students_own() {
    let store = store().await;
    let conversations = store.conversations();
    let ctx = home("111");
    assert!(conversations.ensure(&ctx, WEB_CHANNEL).await.is_ok());
    let _ = conversations
        .append(&ctx, &[Message::assistant("Essay is due today.")])
        .await;
    let position = conversations.load(&ctx, 10).await.unwrap_or_default()[0].position;

    let thread = web("111");
    assert!(conversations.ensure(&thread, WEB_CHANNEL).await.is_ok());
    let _ = conversations
        .append(&thread, &[Message::user("help me start")])
        .await;
    let parent = Parent {
        conversation_id: ctx.conversation_id,
        position,
    };
    // Another student cannot hang a thread off this home, and no thread names a missing message.
    let theirs = web("222");
    assert!(conversations.ensure(&theirs, WEB_CHANNEL).await.is_ok());
    let _ = conversations
        .append(&theirs, &[Message::user("mine")])
        .await;
    assert!(conversations.link(&theirs, &parent).await.is_ok());
    let missing = Parent {
        position: position + 100,
        ..parent
    };
    assert!(conversations.link(&thread, &missing).await.is_ok());
    assert!(conversations.link(&thread, &parent).await.is_ok());

    let mine = conversations
        .list(&thread, WEB_CHANNEL, 10)
        .await
        .unwrap_or_default();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].parent, Some(parent));
    let other = conversations
        .list(&theirs, WEB_CHANNEL, 10)
        .await
        .unwrap_or_default();
    assert_eq!(other[0].parent, None);
}
