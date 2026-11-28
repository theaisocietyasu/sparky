//! Fixtures shared across the suite: engine responses, sources, progress, and a local HTTP recorder.

use uuid::Uuid;

use crate::core::types::ChatResponse;
use crate::core::types::chat::Citation;
use crate::render::components::ButtonSpec;

/// An engine response with text, sources, and the status the run ended with.
pub(super) fn response(text: &str, citations: Vec<Citation>, status: &str) -> ChatResponse {
    ChatResponse {
        request_id: Uuid::new_v4(),
        conversation_id: Uuid::new_v4(),
        text: text.into(),
        citations,
        confirmation: None,
        status: status.into(),
        memories: Vec::new(),
    }
}

/// A source that carries a link. It renders as a button.
pub(super) fn linked(title: &str) -> Citation {
    Citation {
        title: title.into(),
        url: Some(format!("https://asu.edu/{title}")),
    }
}

/// A source with no page of its own. It renders as text.
pub(super) fn unlinked(title: &str) -> Citation {
    Citation {
        title: title.into(),
        url: None,
    }
}

/// The label of every button in one row.
pub(super) fn labels_of(row: &[crate::render::components::ButtonSpec]) -> Vec<String> {
    row.iter()
        .map(|b| match b {
            ButtonSpec::Press { label, .. } => (*label).to_owned(),
            ButtonSpec::Link { label, .. } => label.clone(),
        })
        .collect()
}

/// The final render of resp with no head and no steps, as one string.
pub(super) fn render(resp: &ChatResponse, limit: usize) -> Vec<String> {
    crate::render::card::answer(&[], resp, limit)
}

/// A progress update as the engine sends it.
pub(super) fn progress(
    text: &str,
    slot: Option<&str>,
    clear: bool,
    draft: bool,
) -> crate::core::types::Progress {
    crate::core::types::Progress {
        text: text.to_owned(),
        slot: slot.map(str::to_owned),
        clear,
        draft,
    }
}

/// Requests a local server received: path, authorization header, body.
pub(super) type Seen = std::sync::Arc<std::sync::Mutex<Vec<(String, String, String)>>>;

pub(super) async fn record_request(
    axum::extract::State(seen): axum::extract::State<Seen>,
    uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::http::StatusCode {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body = String::from_utf8_lossy(&body).into_owned();
    if let Ok(mut seen) = seen.lock() {
        seen.push((uri.path().to_owned(), auth, body));
    }
    axum::http::StatusCode::OK
}

/// Serves on a thread of its own and returns the bound address.
pub(super) fn serve(seen: Seen) -> Option<std::net::SocketAddr> {
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
            let app = axum::Router::new()
                .fallback(record_request)
                .with_state(seen);
            let _ = axum::serve(listener, app).await;
        });
    });
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .ok()
        .flatten()
}

/// What the server received once n requests arrived, or at the deadline.
pub(super) fn wait_for(seen: &Seen, n: usize) -> Vec<(String, String, String)> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut got = Vec::new();
    while std::time::Instant::now() < deadline {
        got = seen.lock().map_or_default(|s| s.clone());
        if got.len() >= n {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    got
}
