//! Liveness and readiness.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use secrecy::{ExposeSecret, SecretString};

use crate::core::traits::health::Probe;
use crate::core::types::http::health::Readiness;

/// What readiness checks.
#[derive(Clone)]
pub struct HealthState {
    /// The stores every request needs.
    pub probes: Vec<Arc<dyn Probe>>,
    /// Chat model base URL, ending in /v1.
    pub model_base_url: String,
    /// Bearer key the chat model endpoint takes.
    pub model_api_key: SecretString,
}

/// Process is up.
pub async fn live() -> StatusCode {
    StatusCode::OK
}

/// Returns 200 when every store probe and the model endpoint answer, 503 with the report otherwise.
pub async fn ready(State(state): State<HealthState>) -> Response {
    let mut stores = BTreeMap::new();
    for probe in &state.probes {
        let up = probe.ready().await;
        if !up {
            tracing::warn!(store = probe.name(), "readiness: store did not answer");
        }
        stores.insert(probe.name(), up);
    }
    let model = match reqwest::Client::new()
        .get(format!(
            "{}/models",
            state.model_base_url.trim_end_matches('/')
        ))
        .bearer_auth(state.model_api_key.expose_secret())
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => true,
        Ok(r) => {
            tracing::warn!(status = %r.status(), "readiness: model endpoint refused");
            false
        }
        Err(error) => {
            tracing::warn!(%error, "readiness: model endpoint did not answer");
            false
        }
    };
    let healthy = model && stores.values().all(|up| *up);
    let report = Readiness { stores, model };
    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report)).into_response()
}
