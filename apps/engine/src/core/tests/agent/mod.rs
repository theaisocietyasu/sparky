//! The loop: answers, tool feedback, parallelism, policy, limits, retries, cost, redaction.

mod assemble;
mod capability;
mod citation;
mod context;
mod stream;
mod thinking;
mod uploads;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::core::tests::support::{
    Boom, Echo, Named, Ordered, Scripted, Slow, agent, calls, ctx, text,
};
use crate::core::types::agent::AgentConfig;
use crate::core::types::model::ModelError;
use crate::core::types::safety::policy::Decision;
use crate::core::types::safety::redact::redact;
use crate::core::types::tools::RiskClass;
use crate::core::types::trace::{RunStatus, TraceEvent};
use crate::runtime::harness::tools::ToolSet;

#[tokio::test]
async fn text_reply_is_the_answer() {
    let (agent, sink) = agent(
        Scripted::new(vec![Ok(text("2am"))]),
        ToolSet::new(),
        AgentConfig::default(),
    );
    let out = agent.run(&ctx(), "when?").await.ok();
    let out = out.as_ref();
    assert_eq!(out.map(|answer| answer.text.as_str()), Some("2am"));
    assert_eq!(out.map(|answer| answer.steps), Some(1));
    assert!(
        sink.records()
            .iter()
            .any(|record| matches!(record.event, TraceEvent::Completed { .. }))
    );
}

#[tokio::test]
async fn tool_result_is_fed_back_and_loop_continues() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let (agent, sink) = agent(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "echo", json!({"q": 1}))])),
            Ok(text("done")),
        ]),
        tools,
        AgentConfig::default(),
    );
    let out = agent.run(&ctx(), "go").await.ok();
    assert_eq!(
        out.as_ref().map(|answer| answer.text.as_str()),
        Some("done")
    );
    assert_eq!(out.as_ref().map(|answer| answer.steps), Some(2));
    assert!(sink.records().iter().any(
        |record| matches!(&record.event, TraceEvent::ToolCall { tool, .. } if tool == "echo")
    ));
    let runs = out.map(|answer| answer.tool_runs).unwrap_or_default();
    assert_eq!(runs.len(), 1, "the answer reports what it ran");
    assert_eq!(runs[0].tool, "echo");
    assert!(runs[0].ok);
}

#[tokio::test]
async fn a_failing_tool_is_reported_as_run_and_failed() {
    let tools = ToolSet::new().with(Arc::new(Boom));
    let (agent, _) = agent(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "boom", json!({}))])),
            Ok(text("recovered")),
        ]),
        tools,
        AgentConfig::default(),
    );

    let runs = agent
        .run(&ctx(), "go")
        .await
        .map(|answer| answer.tool_runs)
        .unwrap_or_default();

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].tool, "boom");
    assert!(!runs[0].ok, "a failed call still shows, marked failed");
}

#[tokio::test]
async fn parallel_calls_all_run() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let (agent, sink) = agent(
        Scripted::new(vec![
            Ok(calls(vec![
                ("c1", "echo", json!(1)),
                ("c2", "echo", json!(2)),
                ("c3", "echo", json!(3)),
            ])),
            Ok(text("ok")),
        ]),
        tools,
        AgentConfig::default(),
    );
    let _ = agent.run(&ctx(), "go").await;
    let count = sink
        .records()
        .iter()
        .filter(|record| matches!(record.event, TraceEvent::ToolCall { .. }))
        .count();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn write_without_role_is_denied_not_run() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ExternalWrite)));
    let (agent, sink) = agent(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "echo", json!({}))])),
            Ok(text("ok")),
        ]),
        tools,
        AgentConfig::default(),
    );
    let _ = agent.run(&ctx(), "post it").await;
    let records = sink.records();
    assert!(records.iter().any(|record| matches!(
        &record.event,
        TraceEvent::PolicyDecision {
            decision: Decision::Deny { .. },
            ..
        }
    )));
    assert!(
        !records
            .iter()
            .any(|record| matches!(record.event, TraceEvent::ToolCall { .. }))
    );
}

#[tokio::test]
async fn write_with_role_stops_for_confirmation() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ExternalWrite)));
    let (agent, _) = agent(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "echo", json!({}))])),
            Ok(text("never")),
        ]),
        tools,
        AgentConfig::default(),
    );
    let context = ctx().with_roles(vec!["MANAGE_GUILD".into()]);
    let out = agent.run(&context, "post it").await.ok();
    assert_eq!(
        out.as_ref().map(|answer| answer.status.clone()),
        Some(RunStatus::AwaitingConfirmation)
    );
    assert!(
        out.as_ref()
            .is_some_and(|answer| answer.confirmation.is_some())
    );
}

#[tokio::test]
async fn the_last_step_answers_from_what_the_tools_found() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let model = Scripted::new(vec![
        Ok(calls(vec![("c0", "echo", json!(0))])),
        Ok(calls(vec![("c1", "echo", json!(1))])),
        Ok(text("from what I found")),
    ]);
    let sent = model.sent();
    let (agent, _) = agent(
        model,
        tools,
        AgentConfig {
            max_steps: 3,
            ..AgentConfig::default()
        },
    );
    let out = agent.run(&ctx(), "loop").await.ok();
    assert_eq!(
        out.as_ref().map(|answer| answer.text.as_str()),
        Some("from what I found")
    );
    assert_eq!(out.as_ref().map(|answer| answer.steps), Some(3));
    let requests = sent.lock().map(|r| r.clone()).unwrap_or_default();
    assert!(
        requests.last().is_some_and(|r| r.tools.is_empty()),
        "the last step is offered no tools"
    );
}

#[tokio::test]
async fn a_step_offered_no_tools_never_runs_one() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let script: Vec<_> = (0..10)
        .map(|i| Ok(calls(vec![(&format!("c{i}"), "echo", json!(i))])))
        .collect();
    let (agent, sink) = agent(
        Scripted::new(script),
        tools,
        AgentConfig {
            max_steps: 3,
            ..AgentConfig::default()
        },
    );
    let out = agent.run(&ctx(), "loop").await.ok();
    assert_eq!(
        out.as_ref().map(|answer| answer.status.clone()),
        Some(RunStatus::Stalled)
    );
    let executed = sink
        .records()
        .iter()
        .filter(|record| matches!(record.event, TraceEvent::ToolCall { .. }))
        .count();
    assert_eq!(executed, 2, "the answer step runs nothing");
}

#[tokio::test]
async fn the_tool_call_cap_makes_the_next_step_answer() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let model = Scripted::new(vec![
        Ok(calls(vec![
            ("a", "echo", json!(1)),
            ("b", "echo", json!(2)),
            ("c", "echo", json!(3)),
        ])),
        Ok(text("enough")),
    ]);
    let sent = model.sent();
    let (agent, _) = agent(
        model,
        tools,
        AgentConfig {
            max_tool_calls: 3,
            ..AgentConfig::default()
        },
    );
    let out = agent.run(&ctx(), "go").await.ok();
    assert_eq!(out.map(|answer| answer.text), Some("enough".into()));
    let requests = sent.lock().map(|r| r.clone()).unwrap_or_default();
    assert!(requests.last().is_some_and(|r| r.tools.is_empty()));
}

#[tokio::test]
async fn a_slow_tool_leaves_time_to_answer() {
    let tools = ToolSet::new().with(Arc::new(Slow));
    let (agent, _) = agent(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "slow", json!({}))])),
            Ok(text("what I have")),
        ]),
        tools,
        AgentConfig {
            tool_timeout: Duration::from_secs(60),
            answer_reserve: Duration::from_secs(60),
            ..AgentConfig::default()
        },
    );
    let context = crate::core::types::agent::context::RequestContext::new(
        "g",
        "u",
        Duration::from_millis(400),
    );
    let out = agent.run(&context, "go").await.ok();
    assert_eq!(
        out.as_ref().map(|answer| answer.status.clone()),
        Some(RunStatus::Answered)
    );
    assert_eq!(out.map(|answer| answer.text), Some("what I have".into()));
}

#[tokio::test]
async fn tool_timeout_becomes_an_error_result() {
    let tools = ToolSet::new().with(Arc::new(Slow));
    let (agent, sink) = agent(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "slow", json!({}))])),
            Ok(text("ok")),
        ]),
        tools,
        AgentConfig {
            tool_timeout: Duration::from_millis(50),
            ..AgentConfig::default()
        },
    );
    let _ = agent.run(&ctx(), "go").await;
    assert!(sink.records().iter().any(|record| matches!(
        &record.event,
        TraceEvent::ToolCall { result: Err(message), .. } if message.contains("timed out")
    )));
}

#[tokio::test]
async fn cancellation_ends_the_run() {
    let (agent, _) = agent(
        Scripted::new(vec![Ok(text("x"))]),
        ToolSet::new(),
        AgentConfig::default(),
    );
    let context = ctx();
    context.cancel.cancel();
    let out = agent.run(&context, "go").await.ok();
    assert_eq!(out.map(|answer| answer.status), Some(RunStatus::Cancelled));
}

#[tokio::test]
async fn retryable_model_error_is_retried() {
    let (agent, sink) = agent(
        Scripted::new(vec![
            Err(ModelError::Transport("boom".into())),
            Ok(text("recovered")),
        ]),
        ToolSet::new(),
        AgentConfig::default(),
    );
    let out = agent.run(&ctx(), "go").await.ok();
    assert_eq!(out.map(|answer| answer.text), Some("recovered".into()));
    assert!(
        sink.records()
            .iter()
            .any(|record| matches!(record.event, TraceEvent::ModelError { retried: true, .. }))
    );
}

#[tokio::test]
async fn usage_and_cost_accumulate() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let (agent, _) = agent(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "echo", json!({}))])),
            Ok(text("ok")),
        ]),
        tools,
        AgentConfig {
            usd_per_m_prompt: 1.0,
            usd_per_m_completion: 2.0,
            ..AgentConfig::default()
        },
    );
    let out = agent.run(&ctx(), "go").await.ok();
    let out = out.as_ref();
    assert_eq!(out.map(|answer| answer.usage.prompt_tokens), Some(20));
    assert_eq!(out.map(|answer| answer.usage.completion_tokens), Some(10));
    assert!(out.is_some_and(|answer| (answer.cost_usd - 0.000_04).abs() < 1e-12));
}

#[test]
fn secrets_are_redacted_from_traces() {
    let redacted = redact(&json!({"user": "a", "password": "b", "nested": {"api_key": "c"}}));
    assert_eq!(redacted["user"], "a");
    assert_eq!(redacted["password"], "[redacted]");
    assert_eq!(redacted["nested"]["api_key"], "[redacted]");
}

#[tokio::test]
async fn a_repeated_call_forces_a_tool_free_answer() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let same = || Ok(calls(vec![("c", "echo", json!({"q": 1}))]));
    // Step 1 calls, step 2 repeats, step 3 (no tools offered) answers.
    let (agent, sink) = agent(
        Scripted::new(vec![same(), same(), Ok(text("from the result"))]),
        tools,
        AgentConfig::default(),
    );
    let out = agent.run(&ctx(), "loop").await.ok();
    assert_eq!(
        out.as_ref().map(|answer| answer.text.as_str()),
        Some("from the result")
    );
    assert_eq!(out.map(|answer| answer.status), Some(RunStatus::Answered));
    let executed = sink
        .records()
        .iter()
        .filter(|record| matches!(record.event, TraceEvent::ToolCall { .. }))
        .count();
    assert_eq!(executed, 1, "the repeat must not run again");
}

#[tokio::test]
async fn the_tool_free_step_is_told_to_answer_in_plain_text() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let same = || Ok(calls(vec![("c", "echo", json!({"q": 1}))]));
    let model = Scripted::new(vec![same(), same(), Ok(text("from the result"))]);
    let sent = model.sent();
    let (agent, _) = agent(model, tools, AgentConfig::default());
    assert!(agent.run(&ctx(), "loop").await.is_ok());
    let requests = sent.lock().map(|r| r.clone()).unwrap_or_default();
    let last = requests.last();
    assert!(
        last.is_some_and(|r| r.tools.is_empty()),
        "the third call offers no tools"
    );
    let told = last.is_some_and(|r| {
        r.messages
            .iter()
            .any(|m| m.content.contains("no tools on this step"))
    });
    assert!(told, "the tool-free call carries the answer-only line");
    assert!(
        !requests[0]
            .messages
            .iter()
            .any(|m| m.content.contains("no tools on this step")),
        "a call with tools does not"
    );
}

#[tokio::test]
async fn a_tool_call_written_as_text_on_the_tool_free_step_is_not_the_answer() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let same = || Ok(calls(vec![("c", "echo", json!({"q": 1}))]));
    let (agent, _) = agent(
        Scripted::new(vec![
            same(),
            same(),
            Ok(text(r#"{"name": "echo", "arguments": {"q": 1}}"#)),
        ]),
        tools,
        AgentConfig::default(),
    );
    let out = agent.run(&ctx(), "loop").await.ok();
    assert_eq!(
        out.as_ref().map(|a| a.status.clone()),
        Some(RunStatus::Stalled)
    );
    assert!(
        out.is_some_and(|a| !a.text.contains("\"name\"")),
        "the user never sees the call as text"
    );
}

#[tokio::test]
async fn repeating_even_without_tools_stalls() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let same = || Ok(calls(vec![("c", "echo", json!({"q": 1}))]));
    // Step 3 gets no tools; a scripted model that still returns an empty answer stalls.
    let mut empty = text("");
    empty.finish_reason = crate::core::types::model::FinishReason::Stop;
    let (agent, _) = agent(
        Scripted::new(vec![same(), same(), Ok(empty)]),
        tools,
        AgentConfig::default(),
    );
    let out = agent.run(&ctx(), "loop").await.ok();
    assert_eq!(out.map(|answer| answer.status), Some(RunStatus::Stalled));
}

#[tokio::test]
async fn stateful_tools_run_in_order() {
    let tools = ToolSet::new().with(Arc::new(Ordered(RiskClass::ReadPublic)));
    let (agent, sink) = agent(
        Scripted::new(vec![
            Ok(calls(vec![
                ("c1", "ordered", json!({"n": 1})),
                ("c2", "ordered", json!({"n": 2})),
                ("c3", "ordered", json!({"n": 3})),
            ])),
            Ok(text("ok")),
        ]),
        tools,
        AgentConfig::default(),
    );
    let _ = agent.run(&ctx(), "go").await;
    let order: Vec<String> = sink
        .records()
        .iter()
        .filter_map(|record| match &record.event {
            TraceEvent::ToolCall {
                result: Ok(text), ..
            } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(order, vec!["1", "2", "3"]);
}

#[test]
fn backoff_grows_and_spreads_retries_across_requests() {
    use uuid::Uuid;

    use crate::runtime::harness::agent::call::backoff;

    let id = Uuid::from_u128(0);
    let plenty = Duration::from_mins(1);
    let first = backoff(1, id, plenty, 250, 8_000);
    let second = backoff(2, id, plenty, 250, 8_000);
    let third = backoff(3, id, plenty, 250, 8_000);
    assert!(first < second && second < third, "each wait is longer");
    assert!(third <= Duration::from_secs(8), "capped");

    // Two requests retrying at the same moment do not wake together.
    assert_ne!(
        backoff(1, Uuid::from_u128(1), plenty, 250, 8_000),
        backoff(1, Uuid::from_u128(2), plenty, 250, 8_000)
    );

    // Never outlives the request.
    assert_eq!(
        backoff(3, id, Duration::from_millis(5), 250, 8_000),
        Duration::from_millis(5)
    );

    // The cap comes from the arguments.
    assert!(backoff(6, id, plenty, 250, 1_000) <= Duration::from_secs(1));
    assert!(backoff(6, id, plenty, 1_000, 30_000) > Duration::from_secs(8));
}

#[tokio::test]
async fn a_run_that_hits_the_step_limit_still_answers_and_keeps_its_turns() {
    use crate::core::tests::support::{Recording, agent_with_store};

    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let script: Vec<_> = (0..10)
        .map(|i| Ok(calls(vec![(&format!("c{i}"), "echo", json!(i))])))
        .collect();
    let store = Arc::new(Recording::default());
    let agent = agent_with_store(
        Scripted::new(script),
        tools,
        AgentConfig {
            max_steps: 2,
            ..AgentConfig::default()
        },
        store.clone(),
    );

    let out = agent.run(&ctx(), "loop").await.ok();

    assert_eq!(
        out.as_ref().map(|a| a.status.clone()),
        Some(RunStatus::Stalled)
    );
    assert!(
        out.is_some_and(|a| !a.text.trim().is_empty()),
        "every terminal status says something"
    );
    assert!(
        !store.appended().is_empty(),
        "the turns are kept even though the loop gave up"
    );
}

#[tokio::test]
async fn a_failed_tool_sends_the_run_to_the_sandbox_before_it_gives_up() {
    let tools = ToolSet::new()
        .with(Arc::new(Boom))
        .with(Arc::new(Named("run_sandbox")));
    let (agent, _) = agent(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "boom", json!({}))])),
            Ok(text("I could not find that. Try the ASU website.")),
            Ok(calls(vec![("c2", "run_sandbox", json!({}))])),
            Ok(text("Hayden closes at midnight.")),
        ]),
        tools,
        AgentConfig::default(),
    );

    let out = agent.run(&ctx(), "when does hayden close").await.ok();

    let runs = out
        .as_ref()
        .map(|a| a.tool_runs.clone())
        .unwrap_or_default();
    assert!(
        runs.iter().any(|r| r.tool == "run_sandbox"),
        "the last route is taken before the user is told nothing was found, got {runs:?}"
    );
    assert_eq!(
        out.map(|a| a.text),
        Some("Hayden closes at midnight.".to_owned())
    );
}

#[tokio::test]
async fn the_run_is_sent_to_the_sandbox_once_and_not_when_there_is_none() {
    let answer = "I could not find that.";
    let script = || {
        Scripted::new(vec![
            Ok(calls(vec![("c1", "boom", json!({}))])),
            Ok(text(answer)),
            Ok(text(answer)),
            Ok(text(answer)),
        ])
    };

    let (no_sandbox, _) = agent(
        script(),
        ToolSet::new().with(Arc::new(Boom)),
        AgentConfig::default(),
    );
    let out = no_sandbox.run(&ctx(), "go").await.ok();
    assert_eq!(
        out.map(|a| a.steps),
        Some(2),
        "with no sandbox registered the answer stands"
    );

    let (with_sandbox, _) = agent(
        script(),
        ToolSet::new()
            .with(Arc::new(Boom))
            .with(Arc::new(Named("run_sandbox"))),
        AgentConfig::default(),
    );
    let out = with_sandbox.run(&ctx(), "go").await.ok();
    assert_eq!(
        out.map(|a| a.steps),
        Some(3),
        "the hand-back is offered once, not every time the model repeats itself"
    );
}

#[tokio::test]
async fn what_is_kept_is_what_was_said_not_what_the_tools_returned() {
    use crate::core::tests::support::{Recording, agent_with_store};
    use crate::core::types::conversation::message::Role;

    let store = Arc::new(Recording::default());
    let agent = agent_with_store(
        Scripted::new(vec![
            Ok(calls(vec![("c1", "echo", json!({"page": "a long page"}))])),
            Ok(text("Hayden closes at midnight.")),
        ]),
        ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic))),
        AgentConfig::default(),
        store.clone(),
    );

    let _ = agent.run(&ctx(), "when does hayden close").await;

    let kept = store.appended();
    let roles: Vec<Role> = kept.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        vec![Role::User, Role::Assistant],
        "a tool call and its result answer this turn, so they are not carried into the next"
    );
    assert!(kept.iter().all(|m| m.tool_calls.is_empty()));
    assert_eq!(kept[1].content, "Hayden closes at midnight.");
}

#[tokio::test]
async fn a_model_call_is_announced_once_per_step_whatever_the_retries() {
    let (agent, sink) = agent(
        Scripted::new(vec![
            Err(ModelError::Transport("boom".into())),
            Ok(text("recovered")),
        ]),
        ToolSet::new(),
        AgentConfig::default(),
    );
    assert!(agent.run(&ctx(), "go").await.is_ok());
    let started: Vec<u32> = sink
        .records()
        .iter()
        .filter_map(|record| match record.event {
            TraceEvent::ModelStarted { step } => Some(step),
            _ => None,
        })
        .collect();
    assert_eq!(started, vec![1]);
}
