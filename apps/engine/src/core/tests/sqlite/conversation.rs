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

#[tokio::test]
async fn list_shows_open_conversations_with_a_question_and_delete_removes_one() {
    let store = store().await.conversations();
    let first = caller();
    let second = caller();
    let empty = caller();
    for ctx in [&first, &second, &empty] {
        assert!(store.ensure(ctx, "web").await.is_ok());
    }
    let asked = [
        Message::user("first question"),
        Message::assistant("answer"),
    ];
    assert!(store.append(&first, &asked).await.is_ok());
    assert!(
        store
            .append(&second, &[Message::user("second question")])
            .await
            .is_ok()
    );

    let Ok(list) = store.list(&first, "web", 10).await else {
        unreachable!("the list loads")
    };
    let mut titles: Vec<&str> = list.iter().map(|c| c.title.as_str()).collect();
    titles.sort_unstable();
    assert_eq!(titles, ["first question", "second question"]);
    assert!(matches!(store.list(&other(), "web", 10).await, Ok(v) if v.is_empty()));
    assert!(matches!(store.list(&first, "elsewhere", 10).await, Ok(v) if v.is_empty()));
    assert!(matches!(store.list(&first, "web", 1).await, Ok(v) if v.len() == 1));

    let thief = other().with_conversation(first.conversation_id);
    assert!(matches!(store.delete(&thief).await, Ok(false)));
    assert!(matches!(store.delete(&first).await, Ok(true)));
    assert!(matches!(store.owns(&first).await, Ok(false)));
    assert!(matches!(store.load(&first, 10).await, Ok(v) if v.is_empty()));
    assert!(matches!(store.list(&second, "web", 10).await, Ok(v) if v.len() == 1));
}

#[tokio::test]
async fn chat_search_finds_the_callers_other_private_chats_only() {
    use crate::core::types::agent::context::RequestContext;
    use std::time::Duration;
    let store = store().await.conversations();
    let mine = caller().with_visibility(Visibility::Private);
    assert!(store.ensure(&mine, "dm").await.is_ok());
    let said = [
        Message::user("I want to apply to the Intel internship"),
        Message::assistant("The Intel internship closes on Friday."),
        Message::tool_result("c1", "search_live", "Intel careers page text"),
    ];
    assert!(store.append(&mine, &said).await.is_ok());
    let public = caller().with_visibility(Visibility::Public);
    assert!(store.ensure(&public, "general").await.is_ok());
    assert!(
        store
            .append(&public, &[Message::user("Intel stock price")])
            .await
            .is_ok()
    );
    let theirs = other().with_visibility(Visibility::Private);
    assert!(store.ensure(&theirs, "dm").await.is_ok());
    assert!(
        store
            .append(&theirs, &[Message::user("Intel interview tips")])
            .await
            .is_ok()
    );

    let now = RequestContext::new("guild", "111", Duration::from_secs(5))
        .with_visibility(Visibility::Private);
    let Ok(hits) = store.search(&now, "intel internships", 10).await else {
        unreachable!("search runs")
    };
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert!(
        hits.iter()
            .all(|h| h.conversation_id == mine.conversation_id.to_string())
    );
    assert!(
        hits.iter()
            .any(|h| h.role == "user" && h.snippet.contains("Intel internship"))
    );
    let Ok(same) = store.search(&mine, "intel", 10).await else {
        unreachable!("search runs")
    };
    assert_eq!(same.len(), 0, "the chat in progress is already in history");
    let Ok(none) = store.search(&now, "  ?! ", 10).await else {
        unreachable!("search runs")
    };
    assert_eq!(none.len(), 0);
}
