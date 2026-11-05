//! The Canvas feed link routes: the token, the link check, save and remove.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use secrecy::{ExposeSecret, SecretString};
use tower::ServiceExt as _;

use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::routes::canvas::CanvasState;
use crate::stores::sqlite::{Seal, SqliteStore, StoreOptions};

const TOKEN: &str = "change-me";
const LINK: &str = "https://canvas.asu.edu/feeds/calendars/user_AbC123.ics";

async fn feeds() -> Arc<dyn CanvasFeeds> {
    let options = StoreOptions {
        path: "sqlite::memory:".into(),
        max_connections: 1,
        busy_timeout: Duration::from_secs(1),
    };
    match SqliteStore::open(&options).await {
        Ok(store) => Arc::new(store.canvas_feeds(Seal::new(&[7; 32]))),
        Err(e) => unreachable!("an in-memory store opens: {e}"),
    }
}

fn router(feeds: Option<Arc<dyn CanvasFeeds>>) -> axum::Router {
    axum::Router::new()
        .route(
            "/canvas/feed",
            axum::routing::post(crate::routes::canvas::save),
        )
        .route(
            "/canvas/feed/remove",
            axum::routing::post(crate::routes::canvas::remove),
        )
        .with_state(CanvasState {
            feeds,
            hosts: vec!["canvas.asu.edu".into()],
            service_token: SecretString::from(TOKEN),
            web: None,
        })
}

async fn post(router: axum::Router, path: &str, token: &str, body: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()));
    let Ok(request) = request else {
        unreachable!("the request builds")
    };
    let response = match router.oneshot(request).await {
        Ok(response) => response,
        Err(e) => match e {},
    };
    let status = response.status();
    let Ok(bytes) = axum::body::to_bytes(response.into_body(), 1 << 16).await else {
        unreachable!("the body reads")
    };
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn save_body(link: &str) -> String {
    serde_json::json!({ "user": "111", "link": link }).to_string()
}

#[tokio::test]
async fn a_feed_link_is_saved_and_removed() {
    let store = feeds().await;
    let app = router(Some(Arc::clone(&store)));
    let (status, _) = post(app.clone(), "/canvas/feed", TOKEN, &save_body(LINK)).await;
    assert_eq!(status, StatusCode::OK);
    let held = store.load("111").await.ok().flatten();
    assert_eq!(
        held.map(|l| l.expose_secret().to_owned()).as_deref(),
        Some(LINK)
    );
    let (status, body) = post(
        app.clone(),
        "/canvas/feed/remove",
        TOKEN,
        r#"{"user": "111"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("true"), "{body}");
    let (_, body) = post(app, "/canvas/feed/remove", TOKEN, r#"{"user": "111"}"#).await;
    assert!(body.contains("false"), "{body}");
}

#[tokio::test]
async fn a_link_that_is_not_a_canvas_feed_is_refused_and_not_kept() {
    let store = feeds().await;
    let app = router(Some(Arc::clone(&store)));
    let bad = "https://evil.example.com/feeds/calendars/user_A.ics";
    let (status, body) = post(app, "/canvas/feed", TOKEN, &save_body(bad)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(!body.contains("evil"), "{body}");
    assert!(matches!(store.load("111").await, Ok(None)));
}

#[tokio::test]
async fn the_routes_need_the_token_and_canvas_on() {
    let (status, _) = post(
        router(Some(feeds().await)),
        "/canvas/feed",
        "wrong",
        &save_body(LINK),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = post(router(None), "/canvas/feed", TOKEN, &save_body(LINK)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn a_web_session_saves_and_reads_only_its_own_link() {
    use crate::core::tests::http::web::{SITE, auth, browser, session};

    let store = feeds().await;
    let web = auth();
    let Some(sealed) = web.seal(&session("222", &[], 3600)) else {
        unreachable!("the session seals")
    };
    let app = axum::Router::new()
        .route(
            "/canvas/feed",
            axum::routing::post(crate::routes::canvas::save).get(crate::routes::canvas::status),
        )
        .with_state(CanvasState {
            feeds: Some(Arc::clone(&store)),
            hosts: vec!["canvas.asu.edu".into()],
            service_token: SecretString::from(TOKEN),
            web: Some(web),
        });
    let send = |method: &str, body: String| {
        let mut request = Request::builder().method(method).uri("/canvas/feed");
        for (name, value) in &browser(SITE, &sealed) {
            request = request.header(name, value);
        }
        let Ok(request) = request
            .header("content-type", "application/json")
            .body(Body::from(body))
        else {
            unreachable!("the request builds")
        };
        let app = app.clone();
        async move {
            let response = match app.oneshot(request).await {
                Ok(response) => response,
                Err(e) => match e {},
            };
            let status = response.status();
            let Ok(bytes) = axum::body::to_bytes(response.into_body(), 1 << 16).await else {
                unreachable!("the body reads")
            };
            (status, String::from_utf8_lossy(&bytes).into_owned())
        }
    };
    let (_, body) = send("GET", String::new()).await;
    assert_eq!(body, r#"{"linked":false}"#);
    let (status, _) = send("POST", save_body(LINK)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(matches!(store.load("111").await, Ok(None)));
    assert!(matches!(store.load("222").await, Ok(Some(_))));
    let (_, body) = send("GET", String::new()).await;
    assert_eq!(body, r#"{"linked":true}"#);
}
