//! Admin routes: who may call them, tool switches, instructions with rollback, and held actions.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use secrecy::SecretString;
use serde_json::{Value, json};

use super::web::{SITE, auth, browser, session};
use crate::core::tests::support::{Ledger, Named, Scripted, agent};
use crate::core::types::admin::{AdminLimits, HeldAction, InstructionsUpdate, ToolSwitch};
use crate::core::types::agent::AgentConfig;
use crate::routes::admin::{self, AdminState};
use crate::runtime::harness::tools::ToolSet;

fn state(store: Arc<Ledger>) -> AdminState {
    let tools = ToolSet::new()
        .with(Arc::new(Named("search_live")))
        .with(Arc::new(Named("run_sandbox")));
    let (agent, _) = agent(Scripted::new(Vec::new()), tools, AgentConfig::default());
    AdminState {
        store: Some(store),
        agent,
        tenant: "g".into(),
        service_token: SecretString::from("t"),
        web: Some(auth()),
        capability: "MANAGE_GUILD".into(),
        limits: AdminLimits {
            max_turns: 4,
            model_slots: 0,
            rate_limit_per_min: 20,
            memory_max_per_user: 50,
            conversation_idle_days: 0,
        },
        max_instructions_chars: 40,
        history_limit: 10,
    }
}

/// Headers of a signed-in browser with the given roles.
fn signed_in(roles: &[&str]) -> HeaderMap {
    let Some(sealed) = auth().seal(&session("111", roles, 3600)) else {
        unreachable!("the session seals")
    };
    browser(SITE, &sealed)
}

fn moderator() -> HeaderMap {
    signed_in(&["MANAGE_GUILD"])
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

#[tokio::test]
async fn a_session_without_the_write_role_or_any_credential_is_refused() {
    let state = state(Arc::new(Ledger::default()));
    let member = admin::tools(State(state.clone()), signed_in(&[])).await;
    assert_eq!(member.status(), StatusCode::FORBIDDEN);
    let nobody = admin::tools(State(state.clone()), HeaderMap::new()).await;
    assert_eq!(nobody.status(), StatusCode::UNAUTHORIZED);
    let wrong_site = {
        let Some(sealed) = auth().seal(&session("111", &["MANAGE_GUILD"], 3600)) else {
            unreachable!("the session seals")
        };
        browser("https://evil.example.com", &sealed)
    };
    let forged = admin::tools(State(state), wrong_site).await;
    assert_eq!(forged.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_tool_switched_off_leaves_the_offer_and_is_recorded_with_its_actor() {
    let store = Arc::new(Ledger::default());
    let state = state(Arc::clone(&store));
    let (status, change) = body(
        admin::switch_tool(
            State(state.clone()),
            moderator(),
            Path("run_sandbox".into()),
            Json(ToolSwitch { on: false }),
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(change["actor"], "111");

    let offered: Vec<String> = state
        .agent
        .tools()
        .definitions()
        .into_iter()
        .map(|d| d.name)
        .collect();
    assert_eq!(offered, vec!["search_live".to_owned()]);
    assert!(state.agent.tools().get("run_sandbox").is_none());

    let (_, listed) = body(admin::tools(State(state.clone()), moderator()).await).await;
    assert_eq!(listed[0]["name"], "run_sandbox");
    assert_eq!(listed[0]["on"], false);

    let unknown = admin::switch_tool(
        State(state),
        moderator(),
        Path("nope".into()),
        Json(ToolSwitch { on: false }),
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn instructions_change_within_the_limit_and_roll_back_to_an_earlier_value() {
    let state = state(Arc::new(Ledger::default()));
    let put = |text: Option<&str>| {
        admin::set_instructions(
            State(state.clone()),
            moderator(),
            Json(InstructionsUpdate {
                text: text.map(str::to_owned),
            }),
        )
    };
    let (_, first) = body(put(Some("Be brief.")).await).await;
    assert_eq!(&*state.agent.instructions(), "Be brief.");
    assert!(body(put(Some("Be thorough.")).await).await.0.is_success());
    assert_eq!(
        put(Some(&"x".repeat(41))).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(put(Some("  ")).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(&*state.agent.instructions(), "Be thorough.");

    let id = first["id"].as_i64().unwrap_or_default();
    let (status, _) =
        body(admin::rollback(State(state.clone()), moderator(), Path(id)).await).await;
    assert_eq!(status, StatusCode::OK);
    let (_, shown) = body(admin::instructions(State(state.clone()), moderator()).await).await;
    assert_eq!(shown["text"], "Be brief.");
    assert_eq!(shown["changed"], true);
    assert_eq!(shown["default_text"], "sys");

    assert!(body(put(None).await).await.0.is_success());
    assert_eq!(&*state.agent.instructions(), "sys");
    let (_, history) = body(admin::history(State(state.clone()), moderator()).await).await;
    assert_eq!(history.as_array().map(Vec::len), Some(4));
    let missing = admin::rollback(State(state), moderator(), Path(99)).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn changes_in_force_are_applied_again_at_boot() {
    let store = Arc::new(Ledger::default());
    let before = state(Arc::clone(&store));
    let _ = admin::switch_tool(
        State(before.clone()),
        moderator(),
        Path("search_live".into()),
        Json(ToolSwitch { on: false }),
    )
    .await;
    let _ = admin::set_instructions(
        State(before),
        moderator(),
        Json(InstructionsUpdate {
            text: Some("Kept.".into()),
        }),
    )
    .await;

    let after = state(Arc::clone(&store));
    assert!(after.agent.tools().get("search_live").is_some());
    let restored = admin::restore(store.as_ref(), "g", &after.agent).await;
    assert!(matches!(restored, Ok(2)));
    assert!(after.agent.tools().get("search_live").is_none());
    assert_eq!(&*after.agent.instructions(), "Kept.");
}

#[tokio::test]
async fn a_moderator_sees_held_actions_and_denies_one() {
    let store = Arc::new(Ledger::default());
    if let Ok(mut held) = store.held.lock() {
        held.push(HeldAction {
            token: "t1".into(),
            user_id: "222".into(),
            tool: "gcal_create".into(),
            created_at: "2026-11-04T00:00:00Z".into(),
            expires_at: "2026-11-04T00:10:00Z".into(),
        });
    }
    let state = state(store);
    let (_, listed) = body(admin::approvals(State(state.clone()), moderator()).await).await;
    assert_eq!(
        listed,
        json!([{
            "token": "t1", "user_id": "222", "tool": "gcal_create",
            "created_at": "2026-11-04T00:00:00Z", "expires_at": "2026-11-04T00:10:00Z"
        }])
    );
    let (status, _) =
        body(admin::deny(State(state.clone()), moderator(), Path("t1".into())).await).await;
    assert_eq!(status, StatusCode::OK);
    let again = admin::deny(State(state.clone()), moderator(), Path("t1".into())).await;
    assert_eq!(again.status(), StatusCode::NOT_FOUND);
    let (_, limits) = body(admin::limits(State(state), moderator()).await).await;
    assert_eq!(limits["rate_limit_per_min"], 20);
}
