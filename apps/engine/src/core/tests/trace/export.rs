//! OTLP export reaches the Phoenix traces path, with a bearer token only when one is set.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use opentelemetry::trace::{Span as _, Tracer as _, TracerProvider as _};
use secrecy::SecretString;

use crate::core::config::Telemetry;
use crate::core::telemetry::provider;

type Seen = Arc<Mutex<Vec<(String, String)>>>;

async fn record(State(seen): State<Seen>, uri: Uri, headers: HeaderMap) -> StatusCode {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    if let Ok(mut seen) = seen.lock() {
        seen.push((uri.path().to_owned(), auth));
    }
    StatusCode::OK
}

/// Serves on a thread of its own and returns the bound address.
fn serve(seen: Seen) -> Option<std::net::SocketAddr> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        rt.block_on(async move {
            let Ok(listener) = tokio::net::TcpListener::bind("127.0.0.1:0").await else {
                return;
            };
            let _ = tx.send(listener.local_addr().ok());
            let app = Router::new().fallback(record).with_state(seen);
            let _ = axum::serve(listener, app).await;
        });
    });
    rx.recv_timeout(Duration::from_secs(5)).ok().flatten()
}

/// Exports one span through cfg and returns the paths and authorization headers it reached.
fn exported(cfg: &Telemetry, seen: &Seen) -> Vec<(String, String)> {
    let built = provider(cfg, "engine-test", "test");
    assert!(built.as_ref().is_ok_and(Option::is_some), "{built:?}");
    let Ok(Some(provider)) = built else {
        return Vec::new();
    };
    let mut span = provider.tracer("engine-test").start("probe");
    span.end();
    let _ = provider.force_flush();

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut got = Vec::new();
    while Instant::now() < deadline {
        got = seen.lock().map_or_default(|s| s.clone());
        if !got.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = provider.shutdown();
    got
}

#[test]
fn a_span_reaches_phoenix_without_a_bearer_token() {
    let seen: Seen = Arc::default();
    let addr = serve(Arc::clone(&seen));
    assert!(addr.is_some());
    let Some(addr) = addr else {
        return;
    };
    let cfg = Telemetry {
        phoenix_url: Some(format!("http://{addr}/")),
        ..Telemetry::default()
    };
    let got = exported(&cfg, &seen);
    assert_eq!(
        got.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
        ["/v1/traces"]
    );
    assert!(got.iter().all(|(_, auth)| auth.is_empty()), "{got:?}");
}

#[test]
fn an_api_key_is_sent_as_a_bearer_token() {
    let seen: Seen = Arc::default();
    let addr = serve(Arc::clone(&seen));
    assert!(addr.is_some());
    let Some(addr) = addr else {
        return;
    };
    let cfg = Telemetry {
        phoenix_url: Some(format!("http://{addr}/")),
        phoenix_api_key: SecretString::from("px_test"),
        ..Telemetry::default()
    };
    let got = exported(&cfg, &seen);
    assert!(
        got.iter().all(|(_, auth)| auth == "Bearer px_test"),
        "{got:?}"
    );
}

#[test]
fn an_empty_phoenix_url_turns_export_off() {
    let unset = Telemetry::default();
    assert!(matches!(provider(&unset, "e", "test"), Ok(None)));
    let blank = Telemetry {
        phoenix_url: Some(" ".into()),
        ..Telemetry::default()
    };
    assert!(matches!(provider(&blank, "e", "test"), Ok(None)));
}

#[test]
fn the_resource_names_the_service_environment_and_phoenix_project() {
    use opentelemetry::Key;

    use crate::core::telemetry::resource;

    let cfg = Telemetry {
        project_name: "sparky-test".into(),
        ..Telemetry::default()
    };
    let resource = resource(&cfg, "engine-test", "test");
    for (key, want) in [
        ("service.name", "engine-test"),
        ("deployment.environment", "test"),
        ("openinference.project.name", "sparky-test"),
    ] {
        assert_eq!(
            resource
                .get(&Key::from_static_str(key))
                .map(|v| v.to_string()),
            Some(want.to_owned()),
            "{key}"
        );
    }
}
