//! The guardrail: what it refuses, on which branch, and what the loop does with a block.

use std::sync::Arc;
use std::time::Duration;

use crate::core::config;
use crate::core::tests::support::{Scripted, calls, text};
use crate::core::traits::safety::guardrail::Guardrail;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::safety::guardrail::{Stage, Verdict};
use crate::runtime::harness::safety::guardrail::{RuleGuardrail, Rules};

fn ctx() -> RequestContext {
    RequestContext::new("g", "u", Duration::from_secs(5))
}

fn rules(denied: &[&str], max_answer_chars: usize) -> Rules {
    Rules {
        denied_phrases: denied.iter().map(|p| p.to_lowercase()).collect(),
        protected_terms: Vec::new(),
        redaction: "[hidden]".into(),
        max_answer_chars,
        replacement: "blocked".into(),
    }
}

/// Rules that redact the given whole-word terms from an answer.
fn protecting(terms: &[&str]) -> Rules {
    let mut rules = rules(&[], 0);
    rules.protect(terms.iter().map(|t| (*t).to_owned()));
    rules
}

#[tokio::test]
async fn a_denied_phrase_is_refused_on_either_branch() {
    let g = RuleGuardrail::new(rules(&["ssn"], 0));
    for stage in [Stage::Answer, Stage::Capability] {
        let v = g.check(&ctx(), stage, "here is your SSN").await;
        let Verdict::Block { replacement, .. } = v else {
            unreachable!("a denied phrase blocks on {stage:?}")
        };
        assert_eq!(replacement, "blocked");
    }
}

#[tokio::test]
async fn matching_ignores_case() {
    let g = RuleGuardrail::new(rules(&["Secret Token"], 0));
    let v = g.check(&ctx(), Stage::Answer, "the secret TOKEN is").await;
    assert!(matches!(v, Verdict::Block { .. }));
}

#[tokio::test]
async fn length_and_emptiness_are_checked_only_on_the_answer() {
    let g = RuleGuardrail::new(rules(&[], 10));
    assert!(matches!(
        g.check(&ctx(), Stage::Answer, "far too long to fit").await,
        Verdict::Block { .. }
    ));
    // A capability branch carries tool calls and often no text. Neither rule applies there.
    assert_eq!(
        g.check(&ctx(), Stage::Capability, "far too long to fit")
            .await,
        Verdict::Pass
    );
    assert!(matches!(
        g.check(&ctx(), Stage::Answer, "   ").await,
        Verdict::Block { .. }
    ));
    assert_eq!(
        g.check(&ctx(), Stage::Capability, "   ").await,
        Verdict::Pass
    );
}

#[tokio::test]
async fn a_zero_ceiling_removes_the_length_rule() {
    let g = RuleGuardrail::new(rules(&[], 0));
    let long = "x".repeat(100_000);
    assert_eq!(g.check(&ctx(), Stage::Answer, &long).await, Verdict::Pass);
}

#[tokio::test]
async fn an_empty_denied_phrase_matches_nothing() {
    // An empty phrase blocks nothing.
    let g = RuleGuardrail::new(rules(&[""], 0));
    assert_eq!(
        g.check(&ctx(), Stage::Answer, "a perfectly ordinary answer")
            .await,
        Verdict::Pass
    );
}

#[tokio::test]
async fn a_protected_term_is_redacted_from_an_answer() {
    let g = RuleGuardrail::new(protecting(&["search_live", "run_sandbox"]));
    let v = g
        .check(
            &ctx(),
            Stage::Answer,
            "I will use search_live and run_sandbox now",
        )
        .await;
    let Verdict::Redact { text, .. } = v else {
        unreachable!("a protected term is redacted")
    };
    assert_eq!(text, "I will use [hidden] and [hidden] now");
}

#[tokio::test]
async fn redaction_matches_whole_words_only() {
    let g = RuleGuardrail::new(protecting(&["search"]));
    // search_live embeds search but is not the whole word search.
    let v = g
        .check(&ctx(), Stage::Answer, "try search_live for hours")
        .await;
    assert_eq!(
        v,
        Verdict::Pass,
        "a term inside a longer identifier is left alone"
    );
}

#[tokio::test]
async fn redaction_ignores_case() {
    let g = RuleGuardrail::new(protecting(&["valley_metro"]));
    let v = g.check(&ctx(), Stage::Answer, "call Valley_Metro").await;
    assert!(matches!(v, Verdict::Redact { text, .. } if text == "call [hidden]"));
}

#[tokio::test]
async fn a_protected_term_in_a_capability_branch_is_left() {
    let g = RuleGuardrail::new(protecting(&["search_live"]));
    let v = g
        .check(&ctx(), Stage::Capability, "planning to call search_live")
        .await;
    assert_eq!(
        v,
        Verdict::Pass,
        "the capability branch is not shown to the user"
    );
}

#[tokio::test]
async fn a_denied_phrase_beats_a_protected_term() {
    let mut r = protecting(&["search_live"]);
    r.denied_phrases = vec!["ssn".into()];
    let g = RuleGuardrail::new(r);
    let v = g
        .check(&ctx(), Stage::Answer, "your ssn via search_live")
        .await;
    assert!(
        matches!(v, Verdict::Block { .. }),
        "a block wins over a redaction"
    );
}

#[test]
fn the_default_rules_come_from_configuration() {
    let cfg = config::Guardrail::default();
    let r = Rules::default();
    assert_eq!(r.max_answer_chars, cfg.max_answer_chars);
    assert_eq!(r.replacement, cfg.replacement);
    assert_eq!(r.denied_phrases.len(), 0);
}

#[tokio::test]
async fn a_blocked_answer_replaces_the_text_and_ends_the_run() {
    use crate::core::tests::support::MemorySink;
    use crate::core::types::agent::AgentConfig;
    use crate::core::types::trace::{RunStatus, TraceEvent};
    use crate::runtime::harness::agent::{Agent, AgentDeps};
    use crate::runtime::harness::safety::policy::RiskPolicy;
    use crate::runtime::harness::tools::ToolSet;

    let sink = Arc::new(MemorySink::new());
    let deps = AgentDeps {
        model: Arc::new(Scripted::new(vec![Ok(text("here is your SSN 123"))])),
        tools: ToolSet::new(),
        policy: Arc::new(RiskPolicy::default()),
        trace: sink.clone(),
        conversations: None,
        memory: None,
        confirmations: None,
        compactor: None,
        guardrail: Some(Arc::new(RuleGuardrail::new(rules(&["ssn"], 0)))),
        sandbox: None,
        files: None,
    };
    let agent = Agent::new(deps, AgentConfig::default(), "sys");
    let Ok(answer) = agent.run(&ctx(), "what is my ssn").await else {
        unreachable!("a block ends the run without failing it")
    };
    assert_eq!(answer.status, RunStatus::Blocked);
    assert_eq!(
        answer.text, "blocked",
        "the model text never reaches the user"
    );
    assert!(
        sink.records()
            .iter()
            .any(|r| matches!(r.event, TraceEvent::GuardrailBlocked { .. })),
        "the block is traced"
    );
}

#[tokio::test]
async fn a_leaked_tool_name_is_redacted_before_the_user_sees_it() {
    use crate::core::tests::support::MemorySink;
    use crate::core::types::agent::AgentConfig;
    use crate::core::types::trace::{RunStatus, TraceEvent};
    use crate::runtime::harness::agent::{Agent, AgentDeps};
    use crate::runtime::harness::safety::policy::RiskPolicy;
    use crate::runtime::harness::tools::ToolSet;

    let sink = Arc::new(MemorySink::new());
    let deps = AgentDeps {
        model: Arc::new(Scripted::new(vec![Ok(text(
            "Check the catalog with search_live for open seats.",
        ))])),
        tools: ToolSet::new(),
        policy: Arc::new(RiskPolicy::default()),
        trace: sink.clone(),
        conversations: None,
        memory: None,
        confirmations: None,
        compactor: None,
        guardrail: Some(Arc::new(RuleGuardrail::new(protecting(&["search_live"])))),
        sandbox: None,
        files: None,
    };
    let agent = Agent::new(deps, AgentConfig::default(), "sys");
    let Ok(answer) = agent.run(&ctx(), "any open seats").await else {
        unreachable!("a redaction does not end the run")
    };
    assert_eq!(answer.status, RunStatus::Answered);
    assert_eq!(
        answer.text,
        "Check the catalog with [hidden] for open seats."
    );
    assert!(
        sink.records()
            .iter()
            .any(|r| matches!(r.event, TraceEvent::GuardrailRedacted { .. })),
        "the redaction is traced"
    );
}

#[tokio::test]
async fn a_blocked_capability_branch_stops_before_the_tool_runs() {
    use crate::core::tests::support::{Echo, MemorySink};
    use crate::core::types::agent::AgentConfig;
    use crate::core::types::tools::RiskClass;
    use crate::core::types::trace::{RunStatus, TraceEvent};
    use crate::runtime::harness::agent::{Agent, AgentDeps};
    use crate::runtime::harness::safety::policy::RiskPolicy;
    use crate::runtime::harness::tools::ToolSet;

    let sink = Arc::new(MemorySink::new());
    let deps = AgentDeps {
        model: Arc::new(Scripted::new(vec![
            Ok(calls(vec![("1", "echo", serde_json::json!({"a": 1}))])).map(|mut r| {
                r.content = "running this against the SSN lookup".into();
                r
            }),
        ])),
        tools: ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic))),
        policy: Arc::new(RiskPolicy::default()),
        trace: sink.clone(),
        conversations: None,
        memory: None,
        confirmations: None,
        compactor: None,
        guardrail: Some(Arc::new(RuleGuardrail::new(rules(&["ssn"], 0)))),
        sandbox: None,
        files: None,
    };
    let agent = Agent::new(deps, AgentConfig::default(), "sys");
    let Ok(answer) = agent.run(&ctx(), "go").await else {
        unreachable!("a block ends the run without failing it")
    };
    assert_eq!(answer.status, RunStatus::Blocked);
    assert!(answer.tool_runs.is_empty(), "nothing ran");
    assert!(
        !sink
            .records()
            .iter()
            .any(|r| matches!(r.event, TraceEvent::ToolStarted { .. })),
        "the tool was never started"
    );
}
