//! Thinking per model call: the rules that decide it and the retry without it.

use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::core::tests::support::{Echo, Scripted, agent, calls, ctx, only_thought, text};
use crate::core::types::agent::AgentConfig;
use crate::core::types::agent::thinking::{ThinkingMode, ThinkingReason, ThinkingRules};
use crate::core::types::model::ModelRequest;
use crate::core::types::tools::RiskClass;
use crate::core::types::trace::{RunStatus, TraceEvent};
use crate::runtime::harness::agent::call::thinking::{StepSignals, decide};
use crate::runtime::harness::tools::ToolSet;

fn rules() -> ThinkingRules {
    ThinkingRules {
        after_tools: true,
        ..AgentConfig::default().thinking
    }
}

/// A first step on input, before any tool has run.
fn first(input: &str) -> StepSignals<'_> {
    StepSignals {
        answer_only: false,
        tool_results: false,
        tool_failed: false,
        input,
    }
}

/// Whether each request the model was sent asked it to think.
fn thinking_sent(sent: &Arc<Mutex<Vec<ModelRequest>>>) -> Vec<bool> {
    sent.lock()
        .map_or_default(|requests| requests.iter().map(|r| r.thinking).collect())
}

#[test]
fn a_short_question_answers_without_thinking() {
    let choice = decide(&rules(), &first("when does hayden close tonight"));
    assert!(!choice.on);
    assert_eq!(choice.reason, ThinkingReason::Quick);
}

#[test]
fn the_step_that_plans_the_searches_thinks() {
    let planning = ThinkingRules {
        plan_searches: true,
        ..rules()
    };
    let plan = decide(&planning, &first("clubs"));
    assert_eq!((plan.on, plan.reason), (true, ThinkingReason::Plan));
    let after = decide(
        &planning,
        &StepSignals {
            tool_results: true,
            ..first("clubs")
        },
    );
    assert_eq!(after.reason, ThinkingReason::AfterTools);
}

#[test]
fn by_default_a_simple_lookup_never_thinks() {
    let defaults = AgentConfig::default().thinking;
    let question = "how late is the library open";
    assert!(!decide(&defaults, &first(question)).on);
    let after = decide(
        &defaults,
        &StepSignals {
            tool_results: true,
            ..first(question)
        },
    );
    assert_eq!(
        (after.on, after.reason),
        (false, ThinkingReason::AfterTools)
    );
}

#[test]
fn a_failed_or_empty_tool_call_thinks_on_the_next_step() {
    let failed = StepSignals {
        tool_results: true,
        tool_failed: true,
        ..first("when")
    };
    let defaults = AgentConfig::default().thinking;
    let choice = decide(&defaults, &failed);
    assert_eq!((choice.on, choice.reason), (true, ThinkingReason::Recover));
    let off = ThinkingRules {
        recover: false,
        ..defaults
    };
    assert!(!decide(&off, &failed).on);
    let forced = decide(
        &AgentConfig::default().thinking,
        &StepSignals {
            answer_only: true,
            ..failed
        },
    );
    assert_eq!(forced.reason, ThinkingReason::AnswerOnly);
}

#[test]
fn a_cue_matches_whole_words_in_any_case() {
    let asked = decide(&rules(), &first("Why is the shuttle late"));
    assert_eq!((asked.on, asked.reason), (true, ThinkingReason::Cue));
    let inside = decide(&rules(), &first("showhow tickets"));
    assert_eq!(
        inside.reason,
        ThinkingReason::Quick,
        "a cue inside a word is not a cue"
    );
    let phrase = ThinkingRules {
        cues: vec!["Pros and cons".into()],
        ..rules()
    };
    let matched = decide(&phrase, &first("pros and cons of the meal plan"));
    assert_eq!(matched.reason, ThinkingReason::Cue);
}

#[test]
fn a_long_question_thinks() {
    let long = "tell me about ".repeat(15);
    let choice = decide(&rules(), &first(&long));
    assert_eq!((choice.on, choice.reason), (true, ThinkingReason::Long));
}

#[test]
fn the_question_rules_come_before_tool_results_and_answer_only_before_both() {
    let after = decide(
        &rules(),
        &StepSignals {
            tool_results: true,
            ..first("when")
        },
    );
    assert_eq!((after.on, after.reason), (true, ThinkingReason::AfterTools));
    let quiet = ThinkingRules {
        after_tools: false,
        ..rules()
    };
    let after = decide(
        &quiet,
        &StepSignals {
            tool_results: true,
            ..first("why")
        },
    );
    assert_eq!(
        (after.on, after.reason),
        (true, ThinkingReason::Cue),
        "a cue question thinks while it reads results"
    );
    let after = decide(
        &quiet,
        &StepSignals {
            tool_results: true,
            ..first("when")
        },
    );
    assert!(!after.on);
    let forced = decide(
        &rules(),
        &StepSignals {
            answer_only: true,
            tool_results: true,
            ..first("why")
        },
    );
    assert_eq!(
        (forced.on, forced.reason),
        (false, ThinkingReason::AnswerOnly)
    );
}

#[test]
fn a_fixed_mode_ignores_every_rule() {
    for (mode, on) in [(ThinkingMode::On, true), (ThinkingMode::Off, false)] {
        let fixed = ThinkingRules { mode, ..rules() };
        for step in [first("hi"), first("why")] {
            let choice = decide(&fixed, &step);
            assert_eq!((choice.on, choice.reason), (on, ThinkingReason::Mode));
        }
    }
}

#[tokio::test]
async fn the_loop_sends_the_decision_with_each_call() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let model = Scripted::new(vec![
        Ok(calls(vec![("c1", "echo", json!({}))])),
        Ok(text("done")),
    ]);
    let sent = model.sent();
    let cfg = AgentConfig {
        thinking: ThinkingRules {
            after_tools: false,
            plan_searches: true,
            ..rules()
        },
        max_tokens: 4096,
        max_tokens_without_thinking: 512,
        ..AgentConfig::default()
    };
    let (agent, _) = agent(model, tools, cfg);
    assert!(agent.run(&ctx(), "when").await.is_ok());
    assert_eq!(
        thinking_sent(&sent),
        vec![true, false],
        "planning the searches thinks, then tool results with after_tools off do not"
    );
    let budgets: Vec<u32> = sent
        .lock()
        .map_or_default(|r| r.iter().map(|q| q.max_tokens).collect());
    assert_eq!(
        budgets,
        vec![4096, 512],
        "a call that does not think gets the smaller completion budget"
    );
}

#[tokio::test]
async fn thinking_that_leaves_no_answer_is_asked_again_without_it() {
    let model = Scripted::new(vec![
        Ok(only_thought("the hours are in row two, and")),
        Ok(text("Hayden closes at 2am.")),
    ]);
    let sent = model.sent();
    let cfg = AgentConfig {
        thinking: ThinkingRules {
            plan_searches: true,
            ..rules()
        },
        ..AgentConfig::default()
    };
    let (agent, sink) = agent(model, ToolSet::new(), cfg);
    let out = agent.run(&ctx(), "when does hayden close").await.ok();
    assert_eq!(
        out.as_ref().map(|answer| answer.text.as_str()),
        Some("Hayden closes at 2am.")
    );
    assert_eq!(out.as_ref().map(|answer| answer.steps), Some(1), "one step");
    assert_eq!(
        out.map(|answer| answer.usage.completion_tokens),
        Some(10),
        "both calls are counted"
    );
    assert_eq!(thinking_sent(&sent), vec![true, false]);
    let started = sink
        .records()
        .iter()
        .filter(|record| matches!(record.event, TraceEvent::ModelStarted { .. }))
        .count();
    assert_eq!(started, 1, "the retry does not open a second thinking line");
}

#[tokio::test]
async fn with_the_retry_off_a_spent_step_says_it_ran_out_of_room() {
    let model = Scripted::new(vec![Ok(only_thought("still reasoning"))]);
    let sent = model.sent();
    let cfg = AgentConfig {
        thinking: ThinkingRules {
            retry_without: false,
            plan_searches: true,
            ..rules()
        },
        ..AgentConfig::default()
    };
    let (agent, _) = agent(model, ToolSet::new(), cfg);
    let out = agent.run(&ctx(), "when does hayden close").await.ok();
    assert_eq!(thinking_sent(&sent), vec![true]);
    assert!(
        out.as_ref()
            .is_some_and(|answer| answer.text.contains("ran out of room"))
    );
    assert_eq!(out.map(|answer| answer.status), Some(RunStatus::Answered));
}
