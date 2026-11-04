//! The prune deletes idle conversations with their messages, expired tasks and settled actions.

use chrono::{Duration, SecondsFormat, Utc};

use super::store;
use crate::stores::sqlite::{Pruned, prune_once};

fn ago(days: i64) -> String {
    (Utc::now() - Duration::days(days)).to_rfc3339_opts(SecondsFormat::Micros, true)
}

async fn count(pool: &sqlx::SqlitePool, table: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    sqlx::query_as::<_, (i64,)>(&sql)
        .fetch_one(pool)
        .await
        .map_or(-1, |(n,)| n)
}

async fn seed(pool: &sqlx::SqlitePool) {
    for (id, idle) in [("old", 31), ("recent", 29)] {
        let updated = ago(idle);
        let done = sqlx::query(
            "INSERT INTO conversations (id, tenant_id, user_id, channel_id, visibility, \
             created_at, updated_at) VALUES (?, 'guild', '111', 'chan', 'private', ?, ?)",
        )
        .bind(id)
        .bind(&updated)
        .bind(&updated)
        .execute(pool)
        .await;
        assert!(done.is_ok());
        let done = sqlx::query(
            "INSERT INTO messages (conversation_id, role, content, created_at) \
             VALUES (?, 'user', '{}', ?)",
        )
        .bind(id)
        .bind(&updated)
        .execute(pool)
        .await;
        assert!(done.is_ok());
    }
    for (id, kind, expires) in [("m1", "task", Some(ago(1))), ("m2", "fact", None)] {
        let done = sqlx::query(
            "INSERT INTO memories (id, tenant_id, user_id, kind, content, created_at, expires_at) \
             VALUES (?, 'guild', '111', ?, x'00', ?, ?)",
        )
        .bind(id)
        .bind(kind)
        .bind(ago(3))
        .bind(expires)
        .execute(pool)
        .await;
        assert!(done.is_ok());
    }
    for (token, status, expires) in [
        ("t1", "confirmed", ago(-1)),
        ("t2", "pending", ago(1)),
        ("t3", "pending", ago(-1)),
    ] {
        let done = sqlx::query(
            "INSERT INTO pending_actions (token, tenant_id, user_id, action, payload_hash, status, \
             created_at, expires_at) VALUES (?, 'guild', '111', '{}', 'h', ?, ?, ?)",
        )
        .bind(token)
        .bind(status)
        .bind(ago(2))
        .bind(expires)
        .execute(pool)
        .await;
        assert!(done.is_ok());
    }
}

#[tokio::test]
async fn a_conversation_idle_past_the_limit_goes_with_its_messages() {
    let store = store().await;
    seed(store.pool()).await;
    let done = prune_once(store.pool(), Utc::now(), Some(30)).await;
    assert_eq!(
        done.ok(),
        Some(Pruned {
            conversations: 1,
            memories: 1,
            pending_actions: 2,
        })
    );
    assert_eq!(count(store.pool(), "conversations").await, 1);
    assert_eq!(count(store.pool(), "messages").await, 1);
    assert_eq!(count(store.pool(), "memories").await, 1);
    assert_eq!(count(store.pool(), "pending_actions").await, 1);
}

#[tokio::test]
async fn with_no_idle_limit_every_conversation_stays() {
    let store = store().await;
    seed(store.pool()).await;
    let done = prune_once(store.pool(), Utc::now(), None).await;
    assert_eq!(done.ok().map(|p| p.conversations), Some(0));
    assert_eq!(count(store.pool(), "messages").await, 2);
}
