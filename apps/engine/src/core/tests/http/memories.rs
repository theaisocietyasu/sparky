//! The memory routes over the SQLite store: list, forget by number and all, token, rate limit.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use secrecy::SecretString;
use serde_json::{Value, json};
use tower::ServiceExt as _;

use crate::core::traits::memory::MemoryStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::{MemoryKind, NewMemory};
use crate::routes::memories::MemoryState;
use crate::routes::rate_limit::RateLimiter;
use crate::stores::sqlite::{MemoryLimits, Seal, SqliteStore, StoreOptions};

const TOKEN: &str = "change-me";

async fn memories() -> Arc<dyn MemoryStore> {
    let options = StoreOptions {
        path: "sqlite::memory:".into(),
        max_connections: 1,
        busy_timeout: Duration::from_secs(1),
    };
    let Ok(store) = SqliteStore::open(&options).await else {
        unreachable!("an in-memory store opens")
    };
    let limits = MemoryLimits {
        max_per_user: 50,
        task_soon_days: 7,
        utc_offset_hours: 0,
    };
    let memory: Arc<dyn MemoryStore> = Arc::new(store.memory(Seal::new(&[7; 32]), limits));
    for (user, text) in [("111", "first"), ("111", "second"), ("222", "theirs")] {
        let ctx = RequestContext::new("guild", user, Duration::from_secs(5));
        let new = NewMemory {
            kind: MemoryKind::Fact,
            text: text.into(),
            due: None,
            sensitive: false,
        };
        assert!(memory.remember(&ctx, &new).await.is_ok());
    }
    memory
}

fn router(store: Arc<dyn MemoryStore>, per_min: u32) -> axum::Router {
    axum::Router::new()
        .route(
            "/memories/list",
            axum::routing::post(crate::routes::memories::list),
        )
        .route(
            "/memories/forget",
            axum::routing::post(crate::routes::memories::forget),
        )
        .with_state(MemoryState {
            store,
            request_budget: Duration::from_secs(5),
            utc_offset_hours: -7,
            rate_limit: RateLimiter::new(per_min),
            default_tenant: "guild".into(),
            service_token: SecretString::from(TOKEN),
            web: None,
        })
}

async fn post(router: &axum::Router, path: &str, token: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()));
    let Ok(request) = request else {
        unreachable!("the request builds")
    };
    let response = match router.clone().oneshot(request).await {
        Ok(response) => response,
        Err(e) => match e {},
    };
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap_or_default();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn texts(list: &Value) -> Vec<String> {
    list["memories"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m["text"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn list_numbers_the_callers_memories_newest_first() {
    let router = router(memories().await, 60);
    let (status, list) = post(&router, "/memories/list", TOKEN, json!({"user_id": "111"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(texts(&list), ["second", "first"]);
    assert_eq!(list["memories"][0]["number"], 1);
    assert_eq!(list["memories"][0]["kind"], "fact");
    assert_eq!(list["memories"][0]["id"].as_str().map(str::len), Some(8));
}

#[tokio::test]
async fn forget_by_number_then_all_touches_only_the_caller() {
    let store = memories().await;
    let router = router(store, 60);
    let body = json!({"user_id": "111", "number": 1});
    let (status, done) = post(&router, "/memories/forget", TOKEN, body).await;
    assert_eq!(
        (status, done["removed"].as_u64()),
        (StatusCode::OK, Some(1))
    );
    let (_, list) = post(&router, "/memories/list", TOKEN, json!({"user_id": "111"})).await;
    assert_eq!(texts(&list), ["first"]);
    let body = json!({"user_id": "111", "number": 9});
    let (status, _) = post(&router, "/memories/forget", TOKEN, body).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, done) = post(
        &router,
        "/memories/forget",
        TOKEN,
        json!({"user_id": "111"}),
    )
    .await;
    assert_eq!(done["removed"].as_u64(), Some(1));
    let (_, list) = post(&router, "/memories/list", TOKEN, json!({"user_id": "222"})).await;
    assert_eq!(texts(&list), ["theirs"]);
}

#[tokio::test]
async fn a_wrong_token_is_refused_and_the_rate_limit_applies() {
    let router = router(memories().await, 1);
    let (status, _) = post(
        &router,
        "/memories/list",
        "wrong",
        json!({"user_id": "111"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = post(&router, "/memories/list", TOKEN, json!({"user_id": "111"})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = post(&router, "/memories/list", TOKEN, json!({"user_id": "111"})).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}
