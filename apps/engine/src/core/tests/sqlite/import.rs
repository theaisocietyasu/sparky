//! The platform export imports once: conversations, messages with rewritten covers, sealed memories.

use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::Row;

use super::{caller, seal, store};
use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::memory::MemoryStore;
use crate::core::types::export::{ConversationPage, MemoryPage};
use crate::core::types::memory::MemoryQuery;
use crate::stores::sqlite::{ImportError, Importer};

const CONVERSATION: &str = "6f1c1d2e-0000-4000-8000-000000000001";

fn message(text: &str, role: &str) -> serde_json::Value {
    json!({"role": role, "content": text})
}

fn conversations(updated_at: &str) -> ConversationPage {
    let page = json!({
        "conversations": [{
            "id": CONVERSATION,
            "discord_id": "111",
            "channel_id": "chan",
            "visibility": "public",
            "created_at": "2026-10-01T10:00:00",
            "updated_at": updated_at,
            "ended_at": null,
            "messages": [
                {"seq": 40, "role": "user", "content": message("one", "user"), "covers_seq": null, "created_at": "2026-10-01T10:00:00"},
                {"seq": 41, "role": "assistant", "content": message("two", "assistant"), "covers_seq": null, "created_at": "2026-10-01T10:00:01"},
                {"seq": 42, "role": "user", "content": message("three", "user"), "covers_seq": null, "created_at": "2026-10-01T10:00:02"},
                {"seq": 50, "role": "summary", "content": message("one and two", "summary"), "covers_seq": 41, "created_at": "2026-10-01T10:00:03"}
            ]
        }],
        "next": null
    });
    match serde_json::from_value(page) {
        Ok(p) => p,
        Err(e) => unreachable!("the fixture parses: {e}"),
    }
}

fn memories() -> MemoryPage {
    let page = json!({
        "memories": [
            {"id": "6f1c1d2e-0000-4000-8000-0000000000a1", "discord_id": "111", "kind": "semantic",
             "content": "studies CS", "sensitivity": "normal", "confidence": 0.8,
             "created_at": "2026-10-01T10:00:00Z", "expires_at": null},
            {"id": "6f1c1d2e-0000-4000-8000-0000000000a2", "discord_id": "111", "kind": "profile",
             "content": "lives in Tempe", "sensitivity": "sensitive", "confidence": 0.9,
             "created_at": "2026-10-01T10:00:00Z", "expires_at": null}
        ],
        "next": null
    });
    match serde_json::from_value(page) {
        Ok(p) => p,
        Err(e) => unreachable!("the fixture parses: {e}"),
    }
}

fn recent() -> String {
    (Utc::now() - Duration::days(1))
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string()
}

#[tokio::test]
async fn an_export_imports_with_rewritten_covers_and_sealed_memories_and_only_once() {
    let store = store().await;
    let cutoff = Utc::now() - Duration::days(30);
    let mut importer = Importer::new(store.pool().clone(), seal(), "guild", cutoff);
    assert!(importer.begin("platform", false).await.is_ok());
    assert!(
        importer
            .conversations(&conversations(&recent()))
            .await
            .is_ok()
    );
    assert!(importer.memories(&memories()).await.is_ok());
    let Ok(counts) = importer.finish("platform").await else {
        unreachable!("the import finishes")
    };
    assert_eq!(
        (counts.conversations, counts.messages, counts.memories),
        (1, 4, 2)
    );

    let ctx = match uuid::Uuid::parse_str(CONVERSATION) {
        Ok(id) => caller().with_conversation(id),
        Err(e) => unreachable!("{e}"),
    };
    let Ok(history) = store.conversations().load(&ctx, 10).await else {
        unreachable!("history loads")
    };
    let texts: Vec<&str> = history.iter().map(|s| s.message.content.as_str()).collect();
    assert_eq!(texts, ["one and two", "three"]);

    let Ok(row) = sqlx::query("SELECT content FROM memories WHERE sensitive = 1")
        .fetch_one(store.pool())
        .await
    else {
        unreachable!("the sensitive row exists")
    };
    let raw: Vec<u8> = row.try_get("content").unwrap_or_default();
    assert_ne!(raw, b"lives in Tempe".to_vec());
    let query = MemoryQuery {
        kinds: Vec::new(),
        limit: 10,
    };
    let Ok(found) = store.memory(seal()).recall(&caller(), &query).await else {
        unreachable!("recall runs")
    };
    let mut texts: Vec<&str> = found.iter().map(|m| m.content.as_str()).collect();
    texts.sort_unstable();
    assert_eq!(texts, ["lives in Tempe", "studies CS"]);

    let again = Importer::new(store.pool().clone(), seal(), "guild", cutoff);
    assert!(matches!(
        again.begin("platform", false).await,
        Err(ImportError::Done(..))
    ));
    assert!(again.begin("platform", true).await.is_ok());
}

#[tokio::test]
async fn a_conversation_older_than_the_cutoff_is_skipped() {
    let store = store().await;
    let cutoff = Utc::now() - Duration::days(30);
    let mut importer = Importer::new(store.pool().clone(), seal(), "guild", cutoff);
    assert!(
        importer
            .conversations(&conversations("2020-01-01T00:00:00"))
            .await
            .is_ok()
    );
    let Ok(counts) = importer.finish("platform").await else {
        unreachable!("the import finishes")
    };
    assert_eq!((counts.conversations, counts.skipped), (0, 1));
}
