//! Memories in the file: scoped to the caller, capped, ordered, expiring, sealed when sensitive.

use chrono::{Duration, Utc};
use sqlx::Row;

use super::{caller, limits, other, seal, store};
use crate::core::traits::memory::MemoryStore;
use crate::core::types::memory::{MemoryKind, NewMemory};
use crate::stores::sqlite::{MemoryLimits, Seal};

fn fact(text: &str) -> NewMemory {
    NewMemory {
        kind: MemoryKind::Fact,
        text: text.into(),
        due: None,
        sensitive: false,
    }
}

fn task(text: &str, in_days: i64) -> NewMemory {
    NewMemory {
        kind: MemoryKind::Task,
        text: text.into(),
        due: Some((Utc::now() + Duration::days(in_days)).date_naive()),
        sensitive: false,
    }
}

fn wide() -> MemoryLimits {
    MemoryLimits {
        max_per_user: 50,
        ..limits()
    }
}

async fn texts(
    memories: &impl MemoryStore,
    ctx: &crate::core::types::agent::context::RequestContext,
) -> Vec<String> {
    let Ok(found) = memories.recall(ctx).await else {
        unreachable!("recall runs")
    };
    found.into_iter().map(|m| m.content).collect()
}

#[tokio::test]
async fn recall_leaves_out_past_tasks_and_other_users() {
    let store = store().await;
    let memories = store.memory(seal(), wide());
    let ctx = caller();
    for m in [fact("likes rust"), task("old essay", -2), fact("cs junior")] {
        assert!(memories.remember(&ctx, &m).await.is_ok());
    }
    assert!(
        memories
            .remember(&other(), &fact("not yours"))
            .await
            .is_ok()
    );
    let mut found = texts(&memories, &ctx).await;
    found.sort_unstable();
    assert_eq!(found, ["cs junior", "likes rust"]);
}

#[tokio::test]
async fn a_task_with_no_date_is_a_plan_that_never_expires() {
    let store = store().await;
    let memories = store.memory(seal(), wide());
    let mut undated = task("email my advisor", 3);
    undated.due = None;
    let kept = memories.remember(&caller(), &undated).await;
    let Ok(kept) = kept else {
        unreachable!("an undated task is a plan, got {kept:?}")
    };
    assert_eq!(kept.expires_at, None);
}

#[tokio::test]
async fn a_task_due_soon_comes_first_then_newest_first() {
    let store = store().await;
    let memories = store.memory(seal(), wide());
    let ctx = caller();
    for m in [
        fact("first"),
        task("far task", 30),
        task("later task", 5),
        task("next task", 1),
        fact("newest"),
    ] {
        assert!(memories.remember(&ctx, &m).await.is_ok());
    }
    assert_eq!(
        texts(&memories, &ctx).await,
        ["next task", "later task", "newest", "far task", "first"]
    );
}

#[tokio::test]
async fn at_the_cap_the_oldest_fact_goes_first() {
    let store = store().await;
    let memories = store.memory(seal(), limits());
    let ctx = caller();
    for m in [
        fact("oldest fact"),
        task("task", 3),
        fact("newer fact"),
        fact("newest fact"),
    ] {
        assert!(memories.remember(&ctx, &m).await.is_ok());
    }
    let mut found = texts(&memories, &ctx).await;
    found.sort_unstable();
    assert_eq!(found, ["newer fact", "newest fact", "task"]);
}

#[tokio::test]
async fn forget_touches_only_the_callers_rows() {
    let store = store().await;
    let memories = store.memory(seal(), wide());
    let Ok(mine) = memories.remember(&caller(), &fact("mine")).await else {
        unreachable!("remember runs")
    };
    let Ok(theirs) = memories.remember(&other(), &fact("theirs")).await else {
        unreachable!("remember runs")
    };
    assert!(matches!(
        memories.forget(&caller(), theirs.id).await,
        Ok(false)
    ));
    assert!(matches!(
        memories.forget(&caller(), mine.id).await,
        Ok(true)
    ));
    assert!(matches!(
        memories.forget(&caller(), mine.id).await,
        Ok(false)
    ));
    assert!(memories.remember(&caller(), &fact("again")).await.is_ok());
    assert!(matches!(memories.forget_all(&caller()).await, Ok(1)));
    assert_eq!(texts(&memories, &other()).await, ["theirs"]);
}

#[tokio::test]
async fn a_sensitive_memory_is_stored_sealed_and_recalled_in_clear() {
    let store = store().await;
    let memories = store.memory(seal(), wide());
    let mut secret = fact("my phone is 555-0100");
    secret.sensitive = true;
    assert!(memories.remember(&caller(), &secret).await.is_ok());

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
    assert_eq!(texts(&memories, &caller()).await, ["my phone is 555-0100"]);
}

#[tokio::test]
async fn a_row_that_does_not_open_is_left_out_of_recall() {
    let store = store().await;
    let mut secret = fact("sealed under another key");
    secret.sensitive = true;
    let other_key = store.memory(Seal::new(&[9; 32]), wide());
    assert!(other_key.remember(&caller(), &secret).await.is_ok());
    let memories = store.memory(seal(), wide());
    assert!(memories.remember(&caller(), &fact("plain")).await.is_ok());
    assert_eq!(texts(&memories, &caller()).await, ["plain"]);
}

#[tokio::test]
async fn migration_two_maps_the_old_kinds() {
    let store = store().await;
    let Ok(row) =
        sqlx::query("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'memories'")
            .fetch_one(store.pool())
            .await
    else {
        unreachable!("the table exists")
    };
    let sql: String = row.try_get("sql").unwrap_or_default();
    assert!(sql.contains("'fact'") && sql.contains("'preference'") && sql.contains("'task'"));
    assert!(!sql.contains("confidence"));
    assert_eq!(
        MemoryKind::from_platform("episodic"),
        Some(MemoryKind::Fact)
    );
    assert_eq!(
        MemoryKind::from_platform("semantic"),
        Some(MemoryKind::Fact)
    );
    assert_eq!(
        MemoryKind::from_platform("profile"),
        Some(MemoryKind::Preference)
    );
    assert_eq!(MemoryKind::from_platform("task"), Some(MemoryKind::Task));
}
