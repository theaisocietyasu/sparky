//! Which conversation a turn continues, who may continue it, and how a channel resets.

mod compact;

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use secrecy::SecretString;
use serde_json::{Value, json};

use crate::core::tests::support::{Held, Rooms, Scripted, agent_with_store, ctx, text};
use crate::core::traits::conversation::ConversationStore;
use crate::core::types::agent::AgentConfig;
use crate::core::types::conversation::{ResetRequest, Visibility};
use crate::core::types::http::chat::{ChatRequest, ConfirmRequest};
use crate::core::types::store::StoreError;
use crate::routes::chat::{ChatState, chat, confirm};
use crate::routes::conversation::reset;
use crate::routes::rate_limit::RateLimiter;
use crate::runtime::harness::tools::ToolSet;

fn state(rooms: Arc<Rooms>) -> ChatState {
    state_with_web(rooms, None)
}

/// The chat state with the web sign-in on when web is set.
pub(crate) fn state_with_web(
    rooms: Arc<Rooms>,
    web: Option<crate::routes::session::WebAuth>,
) -> ChatState {
    let replies = (0..16).map(|_| Ok(text("ok"))).collect();
    ChatState {
        agent: agent_with_store(
            Scripted::new(replies),
            ToolSet::new(),
            AgentConfig::default(),
            rooms.clone(),
        ),
        conversations: Some(rooms),
        confirmations: Some(Arc::new(Held::default())),
        request_budget: Duration::from_secs(5),
        default_tenant: "g".into(),
        max_images: 4,
        max_files: 4,
        max_file_bytes: 2_000_000,
        turns: Arc::new(tokio::sync::Semaphore::new(4)),
        turn_wait: Duration::from_millis(50),
        attachments_only_input: "what is this".into(),
        service_token: SecretString::from("t"),
        rate_limit: RateLimiter::new(0),
        web,
        web_lists: crate::routes::chat::WebLists {
            conversations: 50,
            turns: 200,
            title_chars: 20,
        },
    }
}

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let Ok(value) = "Bearer t".parse() else {
        unreachable!("a header value")
    };
    headers.insert("authorization", value);
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

async fn send(state: &ChatState, request: Value) -> (StatusCode, Value) {
    let Ok(req) = serde_json::from_value::<ChatRequest>(request) else {
        unreachable!("a chat request")
    };
    body(chat(State(state.clone()), headers(), Json(req)).await).await
}

fn conversation(answer: &Value) -> String {
    answer["conversation_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_chat_request_defaults_to_public_and_a_new_conversation() {
    let Ok(req) = serde_json::from_value::<ChatRequest>(json!({"user_id": "u", "message": "hi"}))
    else {
        unreachable!("a chat request")
    };
    assert_eq!(req.visibility, Visibility::Public);
    assert!(!req.continue_channel);
    assert!(req.conversation_id.is_none());

    let Ok(req) = serde_json::from_value::<ChatRequest>(json!({
        "user_id": "u", "message": "hi", "visibility": "private", "continue_channel": true
    })) else {
        unreachable!("a chat request")
    };
    assert_eq!(req.visibility, Visibility::Private);
    assert!(req.continue_channel);
}

#[tokio::test]
async fn ensuring_another_callers_conversation_is_refused() {
    let rooms = Rooms::default();
    let mine = ctx();
    assert!(rooms.ensure(&mine, "c").await.is_ok());

    let theirs =
        crate::core::types::agent::context::RequestContext::new("g", "other", mine.remaining())
            .with_conversation(mine.conversation_id);
    assert!(matches!(
        rooms.ensure(&theirs, "c").await,
        Err(StoreError::NotOwned)
    ));
    assert!(matches!(rooms.owns(&theirs).await, Ok(false)));
    assert!(matches!(rooms.owns(&mine).await, Ok(true)));
}

#[tokio::test]
async fn continuing_another_users_conversation_reads_as_missing() {
    let state = state(Arc::new(Rooms::default()));
    let (status, first) = send(&state, json!({"user_id": "a", "message": "hi"})).await;
    assert_eq!(status, StatusCode::OK);

    let (status, refused) = send(
        &state,
        json!({"user_id": "b", "message": "hi", "conversation_id": conversation(&first)}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(refused["error"], "no such conversation");

    let (status, again) = send(
        &state,
        json!({"user_id": "a", "message": "more", "conversation_id": conversation(&first)}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(conversation(&again), conversation(&first));
}

#[tokio::test]
async fn continue_channel_picks_the_open_conversation_of_that_channel_and_visibility() {
    let state = state(Arc::new(Rooms::default()));
    let (_, first) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "hi"}),
    )
    .await;
    let (_, continued) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "more", "continue_channel": true}),
    )
    .await;
    assert_eq!(conversation(&continued), conversation(&first));

    let (_, private) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "psst",
               "visibility": "private", "continue_channel": true}),
    )
    .await;
    assert_ne!(conversation(&private), conversation(&first));

    let (_, elsewhere) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c2", "message": "hi", "continue_channel": true}),
    )
    .await;
    assert_ne!(conversation(&elsewhere), conversation(&first));

    let (_, someone_else) = send(
        &state,
        json!({"user_id": "b", "channel_id": "c1", "message": "hi", "continue_channel": true}),
    )
    .await;
    assert_ne!(conversation(&someone_else), conversation(&first));

    let (_, fresh) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "new topic"}),
    )
    .await;
    assert_ne!(conversation(&fresh), conversation(&first));
}

#[tokio::test]
async fn a_reset_ends_the_channel_and_the_next_turn_starts_over() {
    let state = state(Arc::new(Rooms::default()));
    let (_, first) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "hi"}),
    )
    .await;
    send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "psst", "visibility": "private"}),
    )
    .await;

    let (status, ended) = body(
        reset(
            State(state.clone()),
            headers(),
            Json(ResetRequest {
                user: "a".into(),
                tenant: None,
                channel: "c1".into(),
            }),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ended, json!({"ended": 2}));

    let (_, next) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "again", "continue_channel": true}),
    )
    .await;
    assert_ne!(conversation(&next), conversation(&first));
}

#[tokio::test]
async fn a_reset_without_a_channel_or_token_is_refused() {
    let state = state(Arc::new(Rooms::default()));
    let request = || ResetRequest {
        user: "a".into(),
        tenant: None,
        channel: " ".into(),
    };
    let response = reset(State(state.clone()), headers(), Json(request())).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = reset(State(state), HeaderMap::new(), Json(request())).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn confirming_in_another_users_conversation_reads_as_missing() {
    let state = state(Arc::new(Rooms::default()));
    let (_, first) = send(&state, json!({"user_id": "a", "message": "hi"})).await;
    let Ok(id) = conversation(&first).parse() else {
        unreachable!("a conversation id")
    };
    let response = confirm(
        State(state),
        headers(),
        Json(ConfirmRequest {
            token: uuid::Uuid::new_v4(),
            approve: true,
            user_id: "b".into(),
            tenant_id: None,
            conversation_id: id,
            visibility: Visibility::Public,
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn a_reset_request_reads_the_wire_names() {
    let Ok(req) = serde_json::from_value::<ResetRequest>(json!({
        "user_id": "u", "channel_id": "c", "tenant_id": "t"
    })) else {
        unreachable!("a reset request")
    };
    assert_eq!(
        (
            req.user.as_str(),
            req.channel.as_str(),
            req.tenant.as_deref()
        ),
        ("u", "c", Some("t"))
    );
}

fn caller(user: &str) -> crate::core::types::agent::context::RequestContext {
    crate::core::types::agent::context::RequestContext::new("g", user, Duration::from_secs(5))
}

#[tokio::test]
async fn one_user_cannot_load_or_append_to_anothers_turns() {
    use crate::core::types::conversation::message::Message;

    let rooms = Rooms::default();
    let a = caller("a");
    let b = caller("b").with_conversation(a.conversation_id);
    assert!(rooms.ensure(&a, "c").await.is_ok());
    assert!(rooms.append(&a, &[Message::user("secret")]).await.is_ok());

    assert_eq!(rooms.load(&a, 10).await.map(|t| t.len()).ok(), Some(1));
    assert_eq!(rooms.load(&b, 10).await.map(|t| t.len()).ok(), Some(0));
    assert!(matches!(
        rooms.append(&b, &[Message::user("mine now")]).await,
        Err(StoreError::NotOwned)
    ));
}

#[tokio::test]
async fn a_private_conversation_continued_as_public_reads_as_missing() {
    let state = state(Arc::new(Rooms::default()));
    let (_, first) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "psst", "visibility": "private"}),
    )
    .await;
    let (status, refused) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "out loud",
               "conversation_id": conversation(&first)}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(refused["error"], "no such conversation");
}

#[tokio::test]
async fn a_conversation_continued_from_another_channel_reads_as_missing() {
    let state = state(Arc::new(Rooms::default()));
    let (_, first) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c1", "message": "hi"}),
    )
    .await;
    let (status, _) = send(
        &state,
        json!({"user_id": "a", "channel_id": "c2", "message": "hi",
               "conversation_id": conversation(&first)}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn confirming_at_another_visibility_reads_as_missing() {
    let state = state(Arc::new(Rooms::default()));
    let (_, first) = send(
        &state,
        json!({"user_id": "a", "message": "psst", "visibility": "private"}),
    )
    .await;
    let Ok(id) = conversation(&first).parse() else {
        unreachable!("a conversation id")
    };
    let answer = |visibility| ConfirmRequest {
        token: uuid::Uuid::new_v4(),
        approve: true,
        user_id: "a".into(),
        tenant_id: None,
        conversation_id: id,
        visibility,
    };
    let response = confirm(
        State(state.clone()),
        headers(),
        Json(answer(Visibility::Public)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    // Owned at the right visibility, the unknown token is what refuses it.
    let response = confirm(State(state), headers(), Json(answer(Visibility::Private))).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn resetting_past_the_rate_limit_is_refused() {
    let state = ChatState {
        rate_limit: RateLimiter::new(1),
        ..state(Arc::new(Rooms::default()))
    };
    let request = || ResetRequest {
        user: "a".into(),
        tenant: None,
        channel: "c1".into(),
    };
    let first = reset(State(state.clone()), headers(), Json(request())).await;
    assert_eq!(first.status(), StatusCode::OK);
    let second = reset(State(state), headers(), Json(request())).await;
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_turn_past_the_turn_cap_is_refused_as_busy_rather_than_queued_forever() {
    let mut full = state(Arc::new(Rooms::default()));
    full.turns = Arc::new(tokio::sync::Semaphore::new(0));
    let (status, reply) = send(&full, json!({"user_id": "u", "message": "hi"})).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{reply}");
    assert!(reply["error"].as_str().unwrap_or_default().contains("busy"));

    let free = state(Arc::new(Rooms::default()));
    let (status, _) = send(&free, json!({"user_id": "u", "message": "hi"})).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a permit is given back when a turn ends"
    );
    let (status, _) = send(&free, json!({"user_id": "u", "message": "hi again"})).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_message_that_is_only_an_attachment_is_answered() {
    let state = state(Arc::new(Rooms::default()));
    let (status, _) = send(
        &state,
        json!({"user_id": "u", "message": " ", "files": [{
            "url": "https://cdn.discordapp.com/a.pdf", "name": "a.pdf", "size": 10
        }]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(&state, json!({"user_id": "u", "message": " "})).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "nothing at all is still refused"
    );
}

#[tokio::test]
async fn a_signed_in_browser_turn_runs_as_the_cookie_user_in_the_web_channel() {
    use crate::core::tests::http::web::{SITE, auth, browser, session};

    let web = auth();
    let Some(sealed) = web.seal(&session("111", &[], 3600)) else {
        unreachable!("the session seals")
    };
    let state = state_with_web(Arc::new(Rooms::default()), Some(web));
    let request = || {
        let Ok(req) = serde_json::from_value::<ChatRequest>(json!({
            "user_id": "someone-else", "channel_id": "c1", "tenant_id": "other", "message": "hi"
        })) else {
            unreachable!("a chat request")
        };
        req
    };
    let (status, _) = body(
        chat(
            State(state.clone()),
            browser(SITE, &sealed),
            Json(request()),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let forged = chat(
        State(state.clone()),
        browser("https://evil.example.com", &sealed),
        Json(request()),
    )
    .await;
    assert_eq!(forged.status(), StatusCode::UNAUTHORIZED);

    let ended = |user: &str, channel: &str| ResetRequest {
        user: user.into(),
        tenant: None,
        channel: channel.into(),
    };
    let (_, theirs) = body(
        reset(
            State(state.clone()),
            headers(),
            Json(ended("someone-else", "c1")),
        )
        .await,
    )
    .await;
    assert_eq!(theirs, json!({"ended": 0}));
    let (_, mine) = body(reset(State(state), headers(), Json(ended("111", "web"))).await).await;
    assert_eq!(mine, json!({"ended": 1}));
}

#[tokio::test]
async fn a_signed_in_student_lists_reads_and_deletes_only_their_web_chats() {
    use crate::core::tests::http::web::{SITE, auth, browser, session};
    use crate::routes::conversation::{delete, list, read};
    use crate::stores::sqlite::{SqliteStore, StoreOptions};
    use axum::extract::Path;

    let options = StoreOptions {
        path: "sqlite::memory:".into(),
        max_connections: 1,
        busy_timeout: Duration::from_secs(1),
    };
    let Ok(sqlite) = SqliteStore::open(&options).await else {
        unreachable!("an in-memory store opens")
    };
    let store: Arc<dyn ConversationStore> = Arc::new(sqlite.conversations());
    let mine =
        crate::core::types::agent::context::RequestContext::new("g", "111", Duration::from_secs(5))
            .with_visibility(Visibility::Private);
    assert!(store.ensure(&mine, "web").await.is_ok());
    let turns = [
        crate::core::types::conversation::message::Message::user("when   is my next exam and more"),
        crate::core::types::conversation::message::Message::assistant("Friday"),
    ];
    assert!(store.append(&mine, &turns).await.is_ok());

    let web = auth();
    let Some(sealed) = web.seal(&session("111", &[], 3600)) else {
        unreachable!("the session seals")
    };
    let Some(theirs) = web.seal(&session("999", &[], 3600)) else {
        unreachable!("the session seals")
    };
    let mut state = state_with_web(Arc::new(Rooms::default()), Some(web));
    state.conversations = Some(store);
    let id = mine.conversation_id.to_string();

    let (status, listed) = body(list(State(state.clone()), browser(SITE, &sealed)).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed[0]["id"], json!(id));
    assert_eq!(listed[0]["title"], json!("when is my next exam"));
    let (_, none) = body(list(State(state.clone()), browser(SITE, &theirs)).await).await;
    assert_eq!(none, json!([]));

    let (_, chat) = body(
        read(
            State(state.clone()),
            browser(SITE, &sealed),
            Path(id.clone()),
        )
        .await,
    )
    .await;
    assert_eq!(
        chat,
        json!([
            {"role": "user", "text": "when   is my next exam and more"},
            {"role": "assistant", "text": "Friday"}
        ])
    );
    for refused in [
        read(
            State(state.clone()),
            browser(SITE, &theirs),
            Path(id.clone()),
        )
        .await,
        delete(
            State(state.clone()),
            browser(SITE, &theirs),
            Path(id.clone()),
        )
        .await,
        read(
            State(state.clone()),
            browser(SITE, &sealed),
            Path("nope".into()),
        )
        .await,
    ] {
        assert_eq!(refused.status(), StatusCode::NOT_FOUND);
    }
    let anonymous = list(State(state.clone()), HeaderMap::new()).await;
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let gone = delete(State(state.clone()), browser(SITE, &sealed), Path(id)).await;
    assert_eq!(gone.status(), StatusCode::OK);
    let (_, empty) = body(list(State(state), browser(SITE, &sealed)).await).await;
    assert_eq!(empty, json!([]));
}
