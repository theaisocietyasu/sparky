//! Memories in the file: scoped to the caller, unexpired, sealed when sensitive.

use chrono::{Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

use super::{caller, other, seal, store};
use crate::core::traits::memory::MemoryStore;
use crate::core::types::memory::{MemoryKind, MemoryQuery};
use crate::stores::sqlite::{NewMemory, Seal};

fn memory(content: &str, kind: MemoryKind) -> NewMemory {
    NewMemory {
        id: Uuid::new_v4(),
        kind,
        content: content.into(),
        sensitive: false,
        confidence: 0.9,
        source_conversation: None,
        source_seq: None,
        created_at: Utc::now(),
        expires_at: None,
    }
}

fn all() -> MemoryQuery {
    MemoryQuery {
        kinds: Vec::new(),
        limit: 10,
    }
}

#[tokio::test]
async fn recall_leaves_out_expired_rows_other_kinds_and_other_users() {
    let store = store().await;
    let memories = store.memory(seal());
    let ctx = caller();
    let mut expired = memory("old", MemoryKind::Semantic);
    expired.expires_at = Some(Utc::now() - Duration::hours(1));
    for m in [
        memory("likes rust", MemoryKind::Semantic),
        memory("asked about parking", MemoryKind::Episodic),
        expired,
    ] {
        assert!(memories.write(&ctx, &m).await.is_ok());
    }
    assert!(
        memories
            .write(&other(), &memory("not yours", MemoryKind::Semantic))
            .await
            .is_ok()
    );

    let Ok(found) = memories.recall(&ctx, &all()).await else {
        unreachable!("recall runs")
    };
    let mut texts: Vec<&str> = found.iter().map(|m| m.content.as_str()).collect();
    texts.sort_unstable();
    assert_eq!(texts, ["asked about parking", "likes rust"]);

    let semantic = MemoryQuery {
        kinds: vec![MemoryKind::Semantic],
        limit: 10,
    };
    let Ok(found) = memories.recall(&ctx, &semantic).await else {
        unreachable!("recall runs")
    };
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].content, "likes rust");
}

#[tokio::test]
async fn a_sensitive_memory_is_stored_sealed_and_recalled_in_clear() {
    let store = store().await;
    let memories = store.memory(seal());
    let mut secret = memory("my phone is 555-0100", MemoryKind::Profile);
    secret.sensitive = true;
    assert!(memories.write(&caller(), &secret).await.is_ok());

    let Ok(row) = sqlx::query("SELECT content, nonce FROM memories")
        .fetch_one(store.pool())
        .await
    else {
        unreachable!("the row exists")
    };
    let raw: Vec<u8> = row.try_get("content").unwrap_or_default();
    let nonce: Option<Vec<u8>> = row.try_get("nonce").unwrap_or_default();
    assert!(nonce.is_some());
    assert_ne!(raw, b"my phone is 555-0100".to_vec());

    let Ok(found) = memories.recall(&caller(), &all()).await else {
        unreachable!("recall runs")
    };
    assert_eq!(found[0].content, "my phone is 555-0100");
}

#[tokio::test]
async fn a_row_that_does_not_open_is_left_out_of_recall() {
    let store = store().await;
    let mut secret = memory("sealed under another key", MemoryKind::Profile);
    secret.sensitive = true;
    assert!(
        store
            .memory(Seal::new(&[9; 32]))
            .write(&caller(), &secret)
            .await
            .is_ok()
    );
    assert!(
        store
            .memory(seal())
            .write(&caller(), &memory("plain", MemoryKind::Semantic))
            .await
            .is_ok()
    );
    let Ok(found) = store.memory(seal()).recall(&caller(), &all()).await else {
        unreachable!("recall runs")
    };
    let texts: Vec<&str> = found.iter().map(|m| m.content.as_str()).collect();
    assert_eq!(texts, ["plain"]);
}
