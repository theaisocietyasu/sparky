//! Platform-backed adapters against a scripted fake platform that records every request.

mod config;
mod confirmation;
mod conversation;
mod discovery;
mod memory;
mod oauth;
mod query;
mod retrieval;
mod tools;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use secrecy::SecretString;
use serde_json::{Value, json};

use crate::core::types::agent::context::RequestContext;
use crate::stores::platform::PlatformClient;

/// The machine token the fake accepts.
const TOKEN: &str = "plat_test";

/// One request the fake received.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    query: HashMap<String, String>,
    body: Value,
}

/// One scripted answer: status, JSON body, and how long to wait before it.
#[derive(Clone)]
struct Reply {
    status: u16,
    body: Value,
    delay: Duration,
}

/// Scripted answers by method and path.
type Replies = HashMap<(String, String), VecDeque<Reply>>;

/// Routes by method and path; the last answer of a route repeats once the others are used.
#[derive(Clone, Default)]
struct Fake {
    replies: Arc<Mutex<Replies>>,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl Fake {
    /// Scripts an answer for method and path.
    fn on(&self, method: &str, path: &str, status: u16, body: Value) -> &Self {
        self.after(method, path, status, body, Duration::ZERO)
    }

    /// Scripts an answer that arrives after delay.
    fn after(&self, method: &str, path: &str, status: u16, body: Value, delay: Duration) -> &Self {
        if let Ok(mut replies) = self.replies.lock() {
            replies
                .entry((method.to_owned(), path.to_owned()))
                .or_default()
                .push_back(Reply {
                    status,
                    body,
                    delay,
                });
        }
        self
    }

    /// Every request received, in order.
    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// The last request received.
    fn last(&self) -> Seen {
        match self.seen().pop() {
            Some(seen) => seen,
            None => unreachable!("the fake received a request"),
        }
    }

    /// The next answer for a route, keeping the last one in place.
    fn reply(&self, method: &str, path: &str) -> Option<Reply> {
        let mut replies = self.replies.lock().ok()?;
        let queue = replies.get_mut(&(method.to_owned(), path.to_owned()))?;
        if queue.len() > 1 {
            queue.pop_front()
        } else {
            queue.front().cloned()
        }
    }
}

/// Answers every request from the script, after checking the bearer token.
async fn answer(
    axum::extract::State(fake): axum::extract::State<Fake>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let query = uri
        .query()
        .map(|q| {
            url::form_urlencoded::parse(q.as_bytes())
                .into_owned()
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let body = serde_json::from_slice(&body).unwrap_or(Value::Null);
    if let Ok(mut seen) = fake.seen.lock() {
        seen.push(Seen {
            method: method.to_string(),
            path: uri.path().to_owned(),
            query,
            body,
        });
    }
    let bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if uri.path() != "/health" && bearer != format!("Bearer {TOKEN}") {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"error": "Invalid machine token"})),
        )
            .into_response();
    }
    let Some(reply) = fake.reply(method.as_str(), uri.path()) else {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(json!({"error": "no such route"})),
        )
            .into_response();
    };
    tokio::time::sleep(reply.delay).await;
    let status = StatusCode::from_u16(reply.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if reply.body.is_null() {
        return status.into_response();
    }
    (status, axum::Json(reply.body)).into_response()
}

/// Serves fake on a free local port and returns the base URL.
async fn serve(fake: &Fake) -> String {
    let app = Router::new().fallback(answer).with_state(fake.clone());
    let Ok(listener) = tokio::net::TcpListener::bind("127.0.0.1:0").await else {
        unreachable!("a local port is free")
    };
    let Ok(address) = listener.local_addr() else {
        unreachable!("the listener has an address")
    };
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{address}")
}

/// A client of fake holding token.
async fn client_with(fake: &Fake, token: &str) -> PlatformClient {
    match PlatformClient::new(
        &serve(fake).await,
        SecretString::from(token),
        Duration::from_secs(5),
    ) {
        Ok(client) => client,
        Err(e) => unreachable!("the client builds: {e}"),
    }
}

/// A client of fake holding the token it accepts.
async fn client(fake: &Fake) -> PlatformClient {
    client_with(fake, TOKEN).await
}

/// A request from Discord user 111.
fn caller() -> RequestContext {
    RequestContext::new("guild", "111", Duration::from_secs(30))
}

/// The member path prefix of user 111.
const MEMBER: &str = "/api/agents/members/111";

#[tokio::test]
async fn the_client_rejects_a_base_that_is_not_a_url() {
    assert!(
        PlatformClient::new(
            "not a url",
            SecretString::from(TOKEN),
            Duration::from_secs(1)
        )
        .is_err()
    );
}

#[test]
fn a_member_id_is_one_escaped_path_segment() {
    assert_eq!(
        PlatformClient::member("a/b c", "/memories"),
        "/api/agents/members/a%2Fb%20c/memories"
    );
}

#[tokio::test]
async fn readiness_reads_the_platform_health_route() {
    let fake = Fake::default();
    fake.on("GET", "/health", 200, json!({"status": "healthy"}));
    assert!(client(&fake).await.ready().await);
    let down = Fake::default();
    down.on("GET", "/health", 503, json!({}));
    assert!(!client(&down).await.ready().await);
}
