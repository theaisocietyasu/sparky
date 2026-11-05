//! Readiness: every store probe and the model endpoint, each under its own name.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;

use crate::core::tests::support::Fixed;
use crate::core::traits::health::Probe;
use crate::routes::health::{HealthState, ready};

/// A model endpoint that lists its models to a caller with the key k.
async fn model() -> String {
    let app = axum::Router::new().route(
        "/v1/models",
        get(|headers: axum::http::HeaderMap| async move {
            let key = headers.get(axum::http::header::AUTHORIZATION);
            if key.and_then(|v| v.to_str().ok()) == Some("Bearer k") {
                (StatusCode::OK, "{}")
            } else {
                (StatusCode::UNAUTHORIZED, "no key")
            }
        }),
    );
    let Ok(listener) = tokio::net::TcpListener::bind("127.0.0.1:0").await else {
        unreachable!("a local port is free")
    };
    let Ok(address) = listener.local_addr() else {
        unreachable!("the listener has an address")
    };
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{address}/v1")
}

async fn report(probes: Vec<Arc<dyn Probe>>) -> (StatusCode, serde_json::Value) {
    report_with(probes, "k").await
}

async fn report_with(probes: Vec<Arc<dyn Probe>>, key: &str) -> (StatusCode, serde_json::Value) {
    let state = HealthState {
        probes,
        model_base_url: model().await,
        model_api_key: secrecy::SecretString::from(key),
    };
    let response = ready(axum::extract::State(state)).await.into_response();
    let status = response.status();
    let Ok(body) = axum::body::to_bytes(response.into_body(), 1 << 16).await else {
        unreachable!("the body reads")
    };
    (status, serde_json::from_slice(&body).unwrap_or_default())
}

#[tokio::test]
async fn every_probe_is_reported_by_name_beside_the_model() {
    let (status, body) = report(vec![Arc::new(Fixed {
        name: "platform",
        up: true,
    })])
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({"platform": true, "model": true}));
}

#[tokio::test]
async fn one_probe_down_makes_the_engine_unready() {
    let (status, body) = report(vec![Arc::new(Fixed {
        name: "platform",
        up: false,
    })])
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["platform"], false);
}

#[tokio::test]
async fn the_model_probe_sends_the_model_key() {
    let (status, body) = report_with(vec![], "wrong").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["model"], false);
}
