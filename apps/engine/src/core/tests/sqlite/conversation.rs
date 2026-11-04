//! Conversations in the file: ownership, history after a summary, and channel endings.

use super::{caller, other, store};
use crate::core::traits::conversation::ConversationStore;
use crate::core::types::conversation::Visibility;
use crate::core::types::conversation::message::Message;
use crate::core::types::store::StoreError;

#[tokio::test]
async fn ensure_then_owns_and_another_user_is_not_owned() {
    let store = store().await.conversations();
    let ctx = caller();
    assert!(store.ensure(&ctx, "chan").await.is_ok());
    assert!(store.ensure(&ctx, "chan").await.is_ok());
    assert!(matches!(store.owns(&ctx).await, Ok(true)));

    let thief = other().with_conversation(ctx.conversation_id);
    assert!(matches!(
        store.ensure(&thief, "chan").await,
        Err(StoreError::NotOwned)
    ));
    assert!(matches!(store.owns(&thief).await, Ok(false)));
    assert!(matches!(
        store.append(&thief, &[Message::user("hi")]).await,
        Err(StoreError::NotOwned)
    ));
    // Another visibility or channel is a different conversation.
    let private = ctx.clone().with_visibility(Visibility::Private);
    assert!(matches!(store.owns(&private).await, Ok(false)));
    assert!(matches!(
        store.ensure(&ctx, "elsewhere").await,
        Err(StoreError::NotOwned)
    ));
}

#[tokio::test]
async fn load_returns_the_newest_summary_and_only_the_messages_after_it() {
    let store = store().await.conversations();
    let ctx = caller();
    assert!(store.ensure(&ctx, "chan").await.is_ok());
    let turns = [
        Message::user("one"),
        Message::assistant("two"),
        Message::user("three"),
        Message::assistant("four"),
    ];
    assert!(store.append(&ctx, &turns).await.is_ok());
    let Ok(all) = store.load(&ctx, 10).await else {
        unreachable!("history loads")
    };
    let texts: Vec<&str> = all.iter().map(|s| s.message.content.as_str()).collect();
    assert_eq!(texts, ["one", "two", "three", "four"]);

    let covers = all[1].position;
    assert!(
        store
            .append_summary(&ctx, &Message::summary("one and two"), covers)
            .await
            .is_ok()
    );
    let Ok(after) = store.load(&ctx, 10).await else {
        unreachable!("history loads")
    };
    let texts: Vec<&str> = after.iter().map(|s| s.message.content.as_str()).collect();
    assert_eq!(texts, ["one and two", "three", "four"]);
    assert_eq!(after[0].position, covers);

    let Ok(last) = store.load(&ctx, 1).await else {
        unreachable!("history loads")
    };
    let texts: Vec<&str> = last.iter().map(|s| s.message.content.as_str()).collect();
    assert_eq!(texts, ["one and two", "four"]);

    assert!(
        matches!(store.load(&other().with_conversation(ctx.conversation_id), 10).await, Ok(v) if v.is_empty())
    );
}

#[tokio::test]
async fn latest_and_end_are_per_channel_and_latest_is_per_visibility() {
    let store = store().await.conversations();
    let public = caller();
    let private = caller().with_visibility(Visibility::Private);
    assert!(store.ensure(&public, "chan").await.is_ok());
    assert!(store.ensure(&private, "chan").await.is_ok());
    let elsewhere = caller();
    assert!(store.ensure(&elsewhere, "other").await.is_ok());

    assert!(
        matches!(store.latest(&public, "chan").await, Ok(Some(id)) if id == public.conversation_id)
    );
    assert!(
        matches!(store.latest(&private, "chan").await, Ok(Some(id)) if id == private.conversation_id)
    );
    assert!(matches!(store.latest(&other(), "chan").await, Ok(None)));

    assert!(matches!(store.end(&public, "chan").await, Ok(2)));
    assert!(matches!(store.latest(&public, "chan").await, Ok(None)));
    assert!(matches!(
        store.latest(&elsewhere, "other").await,
        Ok(Some(_))
    ));
}
