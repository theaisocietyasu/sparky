//! The grounding check on answers written without tools.

use std::sync::Arc;

use serde_json::json;

use crate::core::tests::support::{Named, Scripted, agent, calls, ctx, text};
use crate::core::types::agent::AgentConfig;
use crate::core::types::trace::TraceEvent;
use crate::runtime::harness::agent::ground::{Grounding, needs_look};
use crate::runtime::harness::agent::task::{Task, TaskConfig};
use crate::runtime::harness::tools::ToolSet;

fn check(verdict: &str) -> Grounding {
    let model = Arc::new(Scripted::new(vec![Ok(text(verdict)), Ok(text(verdict))]));
    Grounding::new(
        Task::new(model, "grounding", "judge", TaskConfig::default()),
        "Search now.",
    )
}

#[test]
fn the_verdict_word_is_read_loosely() {
    assert!(needs_look("LOOK"));
    assert!(needs_look(" look."));
    assert!(needs_look("**LOOK**"));
    assert!(!needs_look("OK"));
    assert!(!needs_look(""));
}

#[tokio::test]
async fn an_answer_that_needs_a_look_goes_back_to_search_once() {
    let (sparky, sink) = agent(
        Scripted::new(vec![
            Ok(text(
                "I do not have live weather data. Check a weather app.",
            )),
            Ok(calls(vec![(
                "c1",
                "search",
                json!({"query": "Tempe weather"}),
            )])),
            Ok(text("Sunny and 78 in Tempe today.")),
        ]),
        ToolSet::new().with(Arc::new(Named("search"))),
        AgentConfig::default(),
    );
    let sparky = sparky.with_grounding(check("LOOK"));

    let out = sparky.run(&ctx(), "weather in tempe today").await.ok();

    assert_eq!(
        out.as_ref().map(|a| a.text.as_str()),
        Some("Sunny and 78 in Tempe today.")
    );
    assert!(
        out.is_some_and(|a| a.tool_runs.iter().any(|r| r.tool == "search")),
        "the second try searched"
    );
    assert!(
        sink.records()
            .iter()
            .any(|r| matches!(r.event, TraceEvent::AnswerDraftCleared { step: 1 })),
        "the guessed answer is withdrawn from the screen"
    );
}

#[tokio::test]
async fn an_answer_the_check_passes_stands() {
    let (sparky, _) = agent(
        Scripted::new(vec![Ok(text("Hey! What are you studying?"))]),
        ToolSet::new().with(Arc::new(Named("search"))),
        AgentConfig::default(),
    );
    let sparky = sparky.with_grounding(check("OK"));

    let out = sparky.run(&ctx(), "hi").await.ok();

    assert_eq!(out.map(|a| a.steps), Some(1));
}

#[tokio::test]
async fn with_no_search_offered_the_check_does_not_run() {
    let (sparky, _) = agent(
        Scripted::new(vec![Ok(text("It is probably sunny."))]),
        ToolSet::new(),
        AgentConfig::default(),
    );
    let sparky = sparky.with_grounding(check("LOOK"));

    let out = sparky.run(&ctx(), "weather?").await.ok();

    assert_eq!(
        out.map(|a| a.text),
        Some("It is probably sunny.".to_owned())
    );
}
