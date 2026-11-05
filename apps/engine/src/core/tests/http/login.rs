//! The account link routes over a store that runs the login itself.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use secrecy::SecretString;
use tower::ServiceExt as _;

use crate::core::tests::support::HostedLogins;
use crate::routes::oauth::OAuthState;

const TOKEN: &str = "change-me";

fn router() -> axum::Router {
    axum::Router::new()
        .route(
            "/oauth/{provider}/authorize",
            axum::routing::post(crate::routes::oauth::authorize),
        )
        .route(
            "/oauth/{provider}/logout",
            axum::routing::post(crate::routes::oauth::disconnect),
        )
        .with_state(OAuthState {
            store: Arc::new(HostedLogins),
            service_token: SecretString::from(TOKEN),
            web: None,
            providers: vec!["canvas".into(), "google".into()],
        })
}

async fn post(path: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"user": "111"}"#));
    let Ok(request) = request else {
        unreachable!("the request builds")
    };
    let response = match router().oneshot(request).await {
        Ok(response) => response,
        Err(e) => match e {},
    };
    let status = response.status();
    let Ok(body) = axum::body::to_bytes(response.into_body(), 1 << 16).await else {
        unreachable!("the body reads")
    };
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn a_hosted_login_hands_back_the_store_link_without_a_local_client() {
    let (status, body) = post("/oauth/canvas/authorize").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("https://platform.test/start/111"), "{body}");
}

#[tokio::test]
async fn a_provider_the_store_does_not_offer_is_not_enabled() {
    let (status, _) = post("/oauth/google/authorize").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn a_hosted_login_can_be_disconnected_without_a_local_client() {
    let (status, body) = post("/oauth/canvas/logout").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("true"), "{body}");
}

#[tokio::test]
async fn a_provider_whose_tool_is_off_is_not_enabled() {
    let (status, _) = post("/oauth/microsoft/authorize").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
