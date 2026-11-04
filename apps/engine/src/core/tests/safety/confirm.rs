//! Approving a held action: what is kept, who may answer, and what happens after.

use std::sync::Arc;

use serde_json::json;

use crate::core::tests::support::{
    Echo, Held, Recording, Scripted, agent_holding, calls, ctx, text,
};
use crate::core::types::tools::RiskClass;
use crate::core::types::trace::RunStatus;
use crate::runtime::harness::tools::ToolSet;

fn moderator() -> crate::core::types::agent::context::RequestContext {
    ctx().with_roles(vec!["MANAGE_GUILD".into()])
}

#[tokio::test]
async fn a_write_is_held_with_what_it_takes_to_run_it_later() {
    let held = Arc::new(Held::default());
    let agent = agent_holding(
        Scripted::new(vec![Ok(calls(vec![("c1", "echo", json!({"x": 1}))]))]),
        ToolSet::new().with(Arc::new(Echo(RiskClass::ExternalWrite))),
        Arc::new(Recording::default()),
        held.clone(),
    );

    let out = agent.run(&moderator(), "post it").await.ok();

    assert_eq!(
        out.as_ref().map(|a| a.status.clone()),
        Some(RunStatus::AwaitingConfirmation)
    );
    assert!(held.holds(), "the action is kept, not just described");
}

#[tokio::test]
async fn only_the_caller_who_was_asked_can_claim_it() {
    use crate::core::traits::safety::confirmation::ConfirmationStore;

    let held = Arc::new(Held::default());
    let agent = agent_holding(
        Scripted::new(vec![Ok(calls(vec![("c1", "echo", json!({"x": 1}))]))]),
        ToolSet::new().with(Arc::new(Echo(RiskClass::ExternalWrite))),
        Arc::new(Recording::default()),
        held.clone(),
    );
    let asker = moderator();
    let token = agent
        .run(&asker, "post it")
        .await
        .ok()
        .and_then(|a| a.confirmation)
        .map(|c| c.token)
        .unwrap_or_default();

    let someone_else = crate::core::types::agent::context::RequestContext::new(
        "g",
        "another-member",
        std::time::Duration::from_secs(5),
    );
    assert!(
        held.claim(&someone_else, token, true)
            .await
            .ok()
            .flatten()
            .is_none(),
        "a bystander cannot approve"
    );
    assert!(
        held.claim(&asker, token, true)
            .await
            .ok()
            .flatten()
            .is_some(),
        "the caller who was asked can"
    );
    assert!(
        held.claim(&asker, token, true)
            .await
            .ok()
            .flatten()
            .is_none(),
        "and only once"
    );
}

#[tokio::test]
async fn approving_runs_the_action_and_carries_on_to_an_answer() {
    use crate::core::types::safety::policy::{PendingAction, ProposedAction};

    let agent = agent_holding(
        Scripted::new(vec![Ok(text("Done — the message is posted."))]),
        ToolSet::new().with(Arc::new(Echo(RiskClass::ExternalWrite))),
        Arc::new(Recording::default()),
        Arc::new(Held::default()),
    );
    let pending = PendingAction {
        call_id: "c1".into(),
        action: ProposedAction {
            tool: "echo".into(),
            risk: RiskClass::ExternalWrite,
            arguments: json!({"x": 1}),
            private: false,
        },
    };

    let out = agent.resume(&moderator(), pending).await.ok();

    assert_eq!(
        out.as_ref().map(|a| a.text.as_str()),
        Some("Done — the message is posted."),
        "the agent speaks after the action, not the raw tool output"
    );
    let ran = out.map(|a| a.tool_runs).unwrap_or_default();
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].tool, "echo");
    assert!(ran[0].ok);
}
