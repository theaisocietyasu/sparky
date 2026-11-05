//! The reminder and notification routes: web sessions see their own, the bot polls with the token.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use chrono::Utc;
use secrecy::SecretString;
use serde_json::Value;

use super::web::{SITE, auth, browser, session};
use crate::core::traits::reminder::ReminderStore;
use crate::core::types::reminder::{NewReminder, Repeat, SentRequest};
use crate::routes::notifications::{self, NotificationState};
use crate::stores::sqlite::{ReminderLimits, Seal, SqliteStore, StoreOptions};

const TOKEN: &str = "change-me";

async fn reminders() -> Arc<dyn ReminderStore> {
    let options = StoreOptions {
        path: "sqlite::memory:".into(),
        max_connections: 1,
        busy_timeout: Duration::from_secs(1),
    };
    let limits = ReminderLimits {
        max_per_user: 10,
        keep_days: 30,
    };
    match SqliteStore::open(&options).await {
        Ok(store) => Arc::new(store.reminders(Seal::new(&[7; 32]), limits)),
        Err(e) => unreachable!("an in-memory store opens: {e}"),
    }
}

fn state(store: Option<Arc<dyn ReminderStore>>) -> NotificationState {
    NotificationState {
        store,
        default_tenant: "g".into(),
        list_limit: 10,
        service_token: SecretString::from(TOKEN),
        web: Some(auth()),
    }
}

fn student(user: &str) -> HeaderMap {
    let Some(sealed) = auth().seal(&session(user, &[], 3600)) else {
        unreachable!("the session seals")
    };
    browser(SITE, &sealed)
}

fn bot() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer change-me"),
    );
    headers
}

async fn body(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    let Ok(bytes) = axum::body::to_bytes(response.into_body(), usize::MAX).await else {
        unreachable!("a readable body")
    };
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn fired(store: &Arc<dyn ReminderStore>, user: &str, text: &str) {
    let due = NewReminder {
        text: text.into(),
        due_at: Utc::now() + chrono::Duration::minutes(1),
        repeat: Repeat::Once,
    };
    let _ = store.add("g", user, &due).await;
    let _ = store.fire(Utc::now() + chrono::Duration::minutes(2)).await;
}

#[tokio::test]
async fn a_student_sees_and_reads_only_their_own_notifications() {
    let store = reminders().await;
    fired(&store, "111", "Essay due").await;
    fired(&store, "222", "Not yours").await;
    let s = state(Some(Arc::clone(&store)));
    let (status, list) = body(notifications::list(State(s.clone()), student("111")).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["unread"], 1);
    assert_eq!(list["notifications"][0]["text"], "Essay due");
    let read = notifications::read(State(s.clone()), student("111")).await;
    assert_eq!(read.status(), StatusCode::NO_CONTENT);
    let (_, list) = body(notifications::list(State(s), student("111")).await).await;
    assert_eq!(list["unread"], 0);
}

#[tokio::test]
async fn a_student_lists_and_cancels_a_reminder() {
    let store = reminders().await;
    let due = NewReminder {
        text: "Lab report".into(),
        due_at: Utc::now() + chrono::Duration::hours(3),
        repeat: Repeat::Weekly,
    };
    let Ok(Some(kept)) = store.add("g", "111", &due).await else {
        unreachable!("kept")
    };
    let s = state(Some(Arc::clone(&store)));
    let (_, list) = body(notifications::reminders(State(s.clone()), student("111")).await).await;
    assert_eq!(list["reminders"][0]["repeat"], "weekly");
    let other =
        notifications::cancel(State(s.clone()), student("222"), Path(kept.id.clone())).await;
    assert_eq!(other.status(), StatusCode::NOT_FOUND);
    let mine = notifications::cancel(State(s), student("111"), Path(kept.id)).await;
    assert_eq!(mine.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn the_bot_polls_with_the_token_and_marks_what_it_sent() {
    let store = reminders().await;
    fired(&store, "111", "Gym").await;
    let s = state(Some(Arc::clone(&store)));
    let refused = notifications::outgoing(State(s.clone()), student("111")).await;
    assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
    let (status, out) = body(notifications::outgoing(State(s.clone()), bot()).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(out["notifications"][0]["user_id"], "111");
    let id = out["notifications"][0]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let done =
        notifications::sent(State(s.clone()), bot(), Json(SentRequest { ids: vec![id] })).await;
    assert_eq!(done.status(), StatusCode::NO_CONTENT);
    let (_, out) = body(notifications::outgoing(State(s), bot()).await).await;
    assert_eq!(out["notifications"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn the_web_routes_refuse_the_bot_and_everything_is_off_without_a_store() {
    let s = state(Some(reminders().await));
    let as_bot = notifications::list(State(s), bot()).await;
    assert_eq!(as_bot.status(), StatusCode::FORBIDDEN);
    let off = notifications::list(State(state(None)), student("111")).await;
    assert_eq!(off.status(), StatusCode::SERVICE_UNAVAILABLE);
}
