//! Streaming: reasoning on the thinking line, the answer released block by block, drafts withdrawn.

use std::sync::Arc;

use serde_json::json;

use crate::core::tests::support::{Echo, MemorySink, Scripted, agent, calls, ctx, text};
use crate::core::types::agent::AgentConfig;
use crate::core::types::model::{ModelDelta, ModelResponse};
use crate::core::types::tools::RiskClass;
use crate::core::types::trace::TraceEvent;
use crate::core::types::trace::progress::{Progress, ProgressStyle};
use crate::runtime::harness::agent::call::draft::{Draft, Release};
use crate::runtime::harness::agent::{Agent, AgentDeps};
use crate::runtime::harness::safety::guardrail::{RuleGuardrail, Rules};
use crate::runtime::harness::safety::policy::RiskPolicy;
use crate::runtime::harness::tools::ToolSet;

fn pushed(draft: &mut Draft, pieces: &[ModelDelta]) -> Vec<Release> {
    pieces.iter().cloned().flat_map(|p| draft.push(p)).collect()
}

fn words(text: &str) -> Vec<ModelDelta> {
    text.split_inclusive(' ')
        .map(|w| ModelDelta::Text(w.to_owned()))
        .collect()
}

/// A completion that reasons, then answers.
fn reasoned(reasoning: &str, answer: &str) -> ModelResponse {
    ModelResponse {
        reasoning: reasoning.into(),
        ..text(answer)
    }
}

/// The streamed events of a run, in order.
fn live(sink: &MemorySink) -> Vec<TraceEvent> {
    sink.records()
        .into_iter()
        .map(|r| r.event)
        .filter(|e| {
            matches!(
                e,
                TraceEvent::ModelReasoning { .. }
                    | TraceEvent::ModelThought { .. }
                    | TraceEvent::AnswerDraft { .. }
                    | TraceEvent::AnswerDraftCleared { .. }
            )
        })
        .collect()
}

#[test]
fn an_answer_is_released_at_the_end_of_each_sentence() {
    let mut draft = Draft::new(160);
    let released = pushed(
        &mut draft,
        &words("Hayden closes at 2am. Noble closes at midnight"),
    );
    assert_eq!(
        released,
        vec![Release::Answer("Hayden closes at 2am.".into())]
    );
    let more = pushed(&mut draft, &words(" tonight.\n"));
    assert_eq!(
        more,
        vec![Release::Answer(
            "Hayden closes at 2am. Noble closes at midnight tonight.".into()
        )]
    );
    assert!(draft.shown());
}

#[test]
fn a_run_with_no_sentence_end_is_released_at_a_word_break_once_it_is_long() {
    let mut draft = Draft::new(20);
    let released = pushed(&mut draft, &words("one two three four five six seven"));
    let Some(Release::Answer(first)) = released.first() else {
        unreachable!("a long run is released, got {released:?}")
    };
    assert!(first.starts_with("one two three"), "{first}");
    assert!(!first.ends_with(' '));
}

#[test]
fn reasoning_grows_on_the_thinking_line_then_is_released_whole_when_the_answer_begins() {
    let mut draft = Draft::new(160);
    let reasoning = pushed(
        &mut draft,
        &[
            ModelDelta::Reasoning("Row two has the hours. ".into()),
            ModelDelta::Reasoning("Friday reads 2am".into()),
        ],
    );
    assert_eq!(
        reasoning,
        vec![Release::Reasoning("Row two has the hours. ".into())]
    );
    let answer = pushed(&mut draft, &words("It closes at 2am. "));
    assert_eq!(
        answer,
        vec![
            Release::Thought("Row two has the hours. Friday reads 2am".into()),
            Release::Answer("It closes at 2am.".into()),
        ]
    );
}

#[test]
fn inline_thinking_never_shows_as_answer_text() {
    let mut draft = Draft::new(160);
    let released = pushed(
        &mut draft,
        &[
            ModelDelta::Text("<thi".into()),
            ModelDelta::Text("nk>Row two. Friday.</think>".into()),
            ModelDelta::Text("Open until 2am. ".into()),
        ],
    );
    assert!(
        released.iter().all(
            |r| !matches!(r, Release::Answer(a) if a.contains("<thi") || a.contains("Row two"))
        ),
        "{released:?}"
    );
    assert!(
        released.contains(&Release::Thought("Row two. Friday.".into())),
        "{released:?}"
    );
    assert!(
        released.contains(&Release::Answer("Open until 2am.".into())),
        "{released:?}"
    );
}

#[test]
fn a_withheld_draft_releases_nothing_more() {
    let mut draft = Draft::new(160);
    draft.withhold();
    assert_eq!(pushed(&mut draft, &words("Anything at all. ")).len(), 0);
    assert!(!draft.shown());
}

#[tokio::test]
async fn a_streamed_run_shows_the_reasoning_then_the_answer_block_by_block() {
    let (agent, sink) = agent(
        Scripted::new(vec![Ok(reasoned(
            "The hours are in row two. Friday is the day asked.",
            "Hayden closes at 2am tonight. Noble closes at midnight.",
        ))])
        .streaming(),
        ToolSet::new(),
        AgentConfig::default(),
    );
    let answer = agent.run(&ctx(), "when does hayden close").await.ok();
    assert_eq!(
        answer.map(|a| a.text),
        Some("Hayden closes at 2am tonight. Noble closes at midnight.".into()),
        "streaming changes what is shown, not the answer"
    );
    let events = live(&sink);
    let reasoning = events
        .iter()
        .position(|e| matches!(e, TraceEvent::ModelReasoning { .. }));
    let thought = events
        .iter()
        .position(|e| matches!(e, TraceEvent::ModelThought { .. }));
    let draft = events
        .iter()
        .position(|e| matches!(e, TraceEvent::AnswerDraft { .. }));
    assert!(reasoning < thought && thought < draft, "{events:?}");
    let drafts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            TraceEvent::AnswerDraft { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(drafts, vec!["Hayden closes at 2am tonight."], "{drafts:?}");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, TraceEvent::AnswerDraftCleared { .. }))
    );
}

#[tokio::test]
async fn text_written_on_the_way_to_a_tool_call_is_withdrawn_as_a_draft() {
    let tools = ToolSet::new().with(Arc::new(Echo(RiskClass::ReadPublic)));
    let preamble = ModelResponse {
        content: "Let me check the live hours first. ".into(),
        ..calls(vec![("c1", "echo", json!({}))])
    };
    let (agent, sink) = agent(
        Scripted::new(vec![Ok(preamble), Ok(text("Open until 2am."))]).streaming(),
        tools,
        AgentConfig::default(),
    );
    assert!(agent.run(&ctx(), "when").await.is_ok());
    let events = live(&sink);
    let cleared = events
        .iter()
        .position(|e| matches!(e, TraceEvent::AnswerDraftCleared { step: 1 }));
    let drafted = events
        .iter()
        .position(|e| matches!(e, TraceEvent::AnswerDraft { step: 1, .. }));
    assert!(drafted.is_some() && cleared > drafted, "{events:?}");
}

#[tokio::test]
async fn a_draft_the_guardrail_refuses_is_withdrawn_and_never_shown_again() {
    let sink = Arc::new(MemorySink::new());
    let deps = AgentDeps {
        model: Arc::new(
            Scripted::new(vec![Ok(text(
                "Here you go. Your SSN is on file. More text follows.",
            ))])
            .streaming(),
        ),
        tools: ToolSet::new(),
        policy: Arc::new(RiskPolicy::default()),
        trace: sink.clone(),
        conversations: None,
        memory: None,
        confirmations: None,
        sandbox: None,
        files: None,
        compactor: None,
        guardrail: Some(Arc::new(RuleGuardrail::new(Rules {
            denied_phrases: vec!["ssn".into()],
            protected_terms: Vec::new(),
            redaction: "[hidden]".into(),
            max_answer_chars: 0,
            replacement: "blocked".into(),
        }))),
    };
    let answer = Agent::new(deps, AgentConfig::default(), "sys")
        .run(&ctx(), "q")
        .await
        .ok();
    assert_eq!(answer.map(|a| a.text), Some("blocked".into()));
    let drafts: Vec<String> = live(&sink)
        .into_iter()
        .filter_map(|e| match e {
            TraceEvent::AnswerDraft { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(
        drafts,
        vec!["Here you go.".to_owned()],
        "nothing after the refusal"
    );
    assert!(
        live(&sink)
            .iter()
            .any(|e| matches!(e, TraceEvent::AnswerDraftCleared { .. }))
    );
}

#[tokio::test]
async fn a_protected_term_is_redacted_in_a_streamed_draft() {
    let sink = Arc::new(MemorySink::new());
    let mut rules = Rules {
        denied_phrases: Vec::new(),
        protected_terms: Vec::new(),
        redaction: "[hidden]".into(),
        max_answer_chars: 0,
        replacement: "blocked".into(),
    };
    rules.protect(["search_live".to_owned()]);
    let deps = AgentDeps {
        model: Arc::new(
            Scripted::new(vec![Ok(text("Use search_live to check. Then read it."))]).streaming(),
        ),
        tools: ToolSet::new(),
        policy: Arc::new(RiskPolicy::default()),
        trace: sink.clone(),
        conversations: None,
        memory: None,
        confirmations: None,
        sandbox: None,
        files: None,
        compactor: None,
        guardrail: Some(Arc::new(RuleGuardrail::new(rules))),
    };
    let answer = Agent::new(deps, AgentConfig::default(), "sys")
        .run(&ctx(), "q")
        .await
        .ok();
    assert_eq!(
        answer.map(|a| a.text),
        Some("Use [hidden] to check. Then read it.".into())
    );
    let drafts: Vec<String> = live(&sink)
        .into_iter()
        .filter_map(|e| match e {
            TraceEvent::AnswerDraft { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(
        drafts,
        vec!["Use [hidden] to check.".to_owned()],
        "the streamed draft is redacted too"
    );
}

#[tokio::test]
async fn with_streaming_off_nothing_is_shown_before_the_call_returns() {
    let (agent, sink) = agent(
        Scripted::new(vec![Ok(reasoned(
            "Row two.",
            "Open until 2am. Closed Sunday.",
        ))])
        .streaming(),
        ToolSet::new(),
        AgentConfig {
            stream: false,
            ..AgentConfig::default()
        },
    );
    assert!(agent.run(&ctx(), "when").await.is_ok());
    let events = live(&sink);
    assert!(
        events
            .iter()
            .all(|e| matches!(e, TraceEvent::ModelThought { .. })),
        "only the finished thought, {events:?}"
    );
}

#[test]
fn live_pieces_reach_the_watcher_as_draft_or_thinking_lines() {
    let style = ProgressStyle {
        detail_chars: 160,
        thought_chars: 12,
        plain: false,
    };
    let reasoning = TraceEvent::ModelReasoning {
        step: 2,
        text: "first part then the latest words".into(),
    };
    let Some(line) = Progress::of(&reasoning, style) else {
        unreachable!("reasoning is shown")
    };
    assert_eq!(
        line.slot.as_deref(),
        Some("model:2"),
        "it writes over the thinking line"
    );
    assert!(
        line.text.starts_with("\u{1f914} \u{2026}") && line.text.ends_with("words"),
        "the newest reasoning shows: {}",
        line.text
    );
    assert!(!line.draft);

    let draft = TraceEvent::AnswerDraft {
        step: 2,
        text: "Open until 2am.\n\nClosed Sunday.".into(),
    };
    let Some(body) = Progress::of(&draft, style) else {
        unreachable!("a draft is shown")
    };
    assert!(body.draft);
    assert_eq!(
        body.text, "Open until 2am.\n\nClosed Sunday.",
        "a draft keeps its lines"
    );

    let Some(gone) = Progress::of(&TraceEvent::AnswerDraftCleared { step: 2 }, style) else {
        unreachable!("a withdrawn draft is shown")
    };
    assert!(gone.draft && gone.clear);
    assert!(
        reasoning.is_transient() && draft.is_transient(),
        "live pieces stay out of the recorded trace"
    );
    assert!(
        !TraceEvent::ModelThought {
            step: 2,
            text: String::new()
        }
        .is_transient()
    );
}
