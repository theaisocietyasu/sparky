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
use crate::core::types::admin::{
    AdminGuardrails, AdminLimits, HeldAction, InstructionsUpdate, ToolSwitch,
};
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
        traces: None,
        traces_limit: 10,
        queries: None,
        guardrails: AdminGuardrails {
            enabled: true,
            denied_phrases: vec!["exam answers".into()],
            protected_terms: vec!["ssn".into()],
            redaction: "[hidden]".into(),
            max_answer_chars: 8000,
            replacement: "I cannot answer that.".into(),
            write_roles: vec!["MANAGE_GUILD".into()],
            allow_authenticated_reads: false,
            confirm_from: "external_write".into(),
        },
        phoenix_ui_url: None,
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

#[tokio::test]
async fn a_moderator_lists_traces_of_the_tenant_and_reads_one_in_full() {
    use crate::core::traits::trace::TraceSink;
    use crate::core::types::agent::context::RequestContext;
    use crate::core::types::model::Usage;
    use crate::core::types::safety::guardrail::Stage;
    use crate::core::types::trace::{RunStatus, TraceEvent};
    use crate::runtime::harness::trace::{JsonlSink, JsonlTraces};

    let dir = std::env::temp_dir().join(format!("sparky-admin-traces-{}", uuid::Uuid::new_v4()));
    let Ok(sink) = JsonlSink::new(&dir, 0) else {
        unreachable!("the trace dir is created")
    };
    let turn = |tenant: &str, input: &str| {
        let ctx = RequestContext::new(tenant, "222", std::time::Duration::from_secs(5));
        sink.emit(
            &ctx,
            TraceEvent::RequestStarted {
                input: input.into(),
                tenant_id: tenant.into(),
                user_id: "222".into(),
                trace_id: Some("4bf92f3577b34da6a3ce929d0e0e4736".into()),
            },
        );
        sink.emit(
            &ctx,
            TraceEvent::GuardrailBlocked {
                step: 1,
                stage: Stage::Answer,
                reason: "denied phrase".into(),
            },
        );
        sink.emit(
            &ctx,
            TraceEvent::Completed {
                status: RunStatus::Answered,
                steps: 2,
                usage: Usage {
                    prompt_tokens: 100,
                    completion_tokens: 20,
                },
                cost_usd: 0.0,
                duration_ms: 900,
            },
        );
        ctx.request_id
    };
    let mine = turn("g", "when is the library open?");
    let _ = turn("other", "not for this server");

    let mut state = state(Arc::new(Ledger::default()));
    state.traces = Some(Arc::new(JsonlTraces::new(&dir)));
    state.phoenix_ui_url = Some("https://phoenix.example/".into());
    let (status, listed) = body(admin::traces(State(state.clone()), moderator()).await).await;
    assert_eq!(status, StatusCode::OK);
    let rows = listed.as_array().cloned().unwrap_or_default();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["user_id"], "222");
    assert_eq!(rows[0]["input"], "when is the library open?");
    assert_eq!(rows[0]["status"], "answered");
    assert_eq!(rows[0]["tokens"], 120);
    assert_eq!(rows[0]["guardrail_blocks"], 1);
    assert_eq!(rows[0]["guardrail_redactions"], 0);
    assert_eq!(
        rows[0]["phoenix_url"],
        "https://phoenix.example/redirects/traces/4bf92f3577b34da6a3ce929d0e0e4736"
    );

    let (status, full) =
        body(admin::trace(State(state.clone()), moderator(), Path(mine.to_string())).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(full.as_array().map(Vec::len), Some(3));
    assert_eq!(full[0]["kind"], "request_started");

    let missing = admin::trace(
        State(state.clone()),
        moderator(),
        Path("../etc/passwd".into()),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let member = admin::traces(State(state), signed_in(&[])).await;
    assert_eq!(member.status(), StatusCode::FORBIDDEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sources_list_the_live_registry_and_traces_off_is_not_implemented() {
    use crate::core::tests::support::FakeQueries;
    use crate::core::types::knowledge::query::QuerySourceInfo;

    let mut state = state(Arc::new(Ledger::default()));
    state.queries = Some(Arc::new(FakeQueries::new(vec![QuerySourceInfo {
        key: "library_hours".into(),
        description: "Library hours by day.".into(),
        params: Vec::new(),
    }])));
    let (_, sources) = body(admin::sources(State(state.clone()), moderator()).await).await;
    assert_eq!(sources[0]["key"], "library_hours");
    let off = admin::traces(State(state), moderator()).await;
    assert_eq!(off.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn a_moderator_reads_the_guardrails_and_a_member_is_refused() {
    let state = state(Arc::new(Ledger::default()));
    let (status, shown) = body(admin::guardrails(State(state.clone()), moderator()).await).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shown["denied_phrases"], json!(["exam answers"]));
    assert_eq!(shown["confirm_from"], "external_write");
    let member = admin::guardrails(State(state), signed_in(&[])).await;
    assert_eq!(member.status(), StatusCode::FORBIDDEN);
}
