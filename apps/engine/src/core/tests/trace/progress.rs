//! Live progress: which trace events reach the caller mid-run, and how they read on the wire.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;

use crate::core::tests::support::{MemorySink, ctx};
use crate::core::traits::trace::TraceSink;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::model::{FinishReason, Usage};
use crate::core::types::safety::policy::Decision;
use crate::core::types::trace::progress::{Progress, ProgressStyle};
use crate::core::types::trace::{RunStatus, TraceEvent};
use crate::runtime::harness::trace::Fanout;

/// A tool start, as the loop emits it.
fn started(tool: &str, arguments: serde_json::Value) -> TraceEvent {
    TraceEvent::ToolStarted {
        step: 1,
        call_id: "c1".into(),
        tool: tool.into(),
        arguments,
    }
}

/// A finished tool call, as the loop emits it.
fn finished(tool: &str, result: Result<String, String>) -> TraceEvent {
    TraceEvent::ToolCall {
        step: 1,
        call_id: "c1".into(),
        tool: tool.into(),
        arguments: json!({"query": "hayden hours"}),
        result,
        duration_ms: 3,
    }
}

/// The line an event writes, at the default amount of detail.
fn line(event: &TraceEvent) -> Option<String> {
    event.progress(ProgressStyle::default())
}

#[test]
fn events_the_caller_should_see_carry_their_own_wording() {
    let search = line(&started("search", json!({"query": "hayden hours"}))).unwrap_or_default();
    assert!(
        search.contains("`search`"),
        "the line names the tool: {search}"
    );
    assert!(
        search.contains("query: hayden hours"),
        "and what it was asked: {search}"
    );
    assert!(
        search.contains("searching"),
        "in wording a student understands: {search}"
    );

    let unknown = line(&started("some_new_mcp_tool", json!({}))).unwrap_or_default();
    assert!(
        unknown.contains("`some_new_mcp_tool`") && unknown.contains("running"),
        "a tool nobody wrote a phrase for still reports something: {unknown}"
    );

    let denied = TraceEvent::PolicyDecision {
        step: 1,
        tool: "announce".into(),
        decision: Decision::Deny {
            reason: "no".into(),
        },
    };
    assert!(
        line(&denied)
            .unwrap_or_default()
            .contains("`announce` was not allowed"),
        "{denied:?}"
    );
}

#[test]
fn a_finished_tool_call_carries_its_result_and_a_failed_one_its_error() {
    let ok = line(&finished(
        "search",
        Ok("three passages about Hayden".into()),
    ))
    .unwrap_or_default();
    assert!(ok.contains("three passages about Hayden"), "{ok}");
    assert!(ok.contains("`search`"), "{ok}");

    let failed = line(&finished("search", Err("upstream said 500".into()))).unwrap_or_default();
    assert!(failed.contains("upstream said 500"), "{failed}");

    let empty = line(&finished("search", Ok(String::new()))).unwrap_or_default();
    assert!(empty.contains("nothing came back"), "{empty}");
}

#[test]
fn detail_is_held_to_what_the_style_allows() {
    let long = finished("search", Ok("x".repeat(4_000)));
    let short = long
        .progress(ProgressStyle {
            detail_chars: 40,
            thought_chars: 40,
            plain: false,
        })
        .unwrap_or_default();
    assert!(short.chars().count() < 140, "{short}");
    assert!(short.contains('\u{2026}'), "the cut is visible: {short}");
}

#[test]
fn a_model_call_a_thought_and_recalled_memory_each_get_a_line() {
    assert!(
        line(&TraceEvent::ModelStarted { step: 1 })
            .unwrap_or_default()
            .contains("thinking")
    );

    let thought = TraceEvent::ModelThought {
        step: 2,
        text: "The hours are not in what I was given,\nso I will search.".into(),
    };
    let shown = line(&thought).unwrap_or_default();
    assert!(shown.contains("so I will search."), "{shown}");
    assert!(!shown.contains('\n'), "a step is one line: {shown}");

    let quiet = TraceEvent::ModelThought {
        step: 2,
        text: "   ".into(),
    };
    assert!(line(&quiet).is_none(), "an empty thought says nothing");

    assert!(
        line(&TraceEvent::MemoryRecalled { count: 3 })
            .unwrap_or_default()
            .contains("remembering 3 things about you")
    );
    assert!(
        line(&TraceEvent::MemoryRecalled { count: 1 })
            .unwrap_or_default()
            .contains("remembering 1 thing about you")
    );
    assert!(line(&TraceEvent::MemoryRecalled { count: 0 }).is_none());
    let written = TraceEvent::MemoryWritten {
        memory: crate::core::types::memory::MemoryKind::Fact,
    };
    assert!(
        line(&written)
            .unwrap_or_default()
            .contains("remembering that")
    );
    assert_eq!(written.kind(), "memory_written");
    assert!(
        line(&TraceEvent::MemoryForgotten)
            .unwrap_or_default()
            .contains("forgetting that")
    );
}

#[test]
fn the_thinking_line_resolves_into_the_thought_or_the_answer() {
    let thinking = TraceEvent::ModelStarted { step: 1 };
    let thought = TraceEvent::ModelThought {
        step: 1,
        text: "the hours are not in what I was given".into(),
    };
    let answered = TraceEvent::ModelAnswered { step: 1 };
    assert_eq!(thinking.slot(), thought.slot());
    assert_eq!(thinking.slot(), answered.slot(), "one line, rewritten");
    assert!(
        answered.clears_slot() && line(&answered).is_none(),
        "a step that thought nothing worth showing takes its thinking line back"
    );
    let Some(taken) = Progress::of(&answered, ProgressStyle::default()) else {
        unreachable!("an event that clears a slot still reaches the client")
    };
    assert!(taken.clear && taken.text.is_empty(), "{taken:?}");
    assert!(
        Progress::of(&thought, ProgressStyle::default()).is_some_and(|p| !p.clear),
        "a thought writes its line instead"
    );
}

#[test]
fn a_tool_result_replaces_the_line_its_own_start_wrote() {
    let start = started("search", json!({}));
    let end = finished("search", Ok("three passages".into()));
    assert_eq!(start.slot(), end.slot(), "one line, twice written");
    assert_eq!(
        TraceEvent::ModelStarted { step: 2 }.slot(),
        TraceEvent::ModelThought {
            step: 2,
            text: "t".into()
        }
        .slot(),
        "a thought lands where the thinking line was"
    );
    assert!(
        TraceEvent::MemoryRecalled { count: 1 }.slot().is_none(),
        "an event with no slot appends"
    );
}

#[test]
fn bookkeeping_events_stay_out_of_the_callers_way() {
    let assembled = TraceEvent::ContextAssembled {
        step: 1,
        message_count: 4,
        estimated_tokens: 900,
    };
    assert!(line(&assembled).is_none());

    let completed = TraceEvent::Completed {
        status: RunStatus::Answered,
        steps: 2,
        usage: Usage::default(),
        cost_usd: 0.0,
        duration_ms: 10,
    };
    assert!(line(&completed).is_none(), "the answer says this");
}

#[test]
fn the_wire_form_reads_without_knowing_the_variant() {
    let event = started("search", json!({"query": "CSE 310 open seats"}));
    let Some(progress) = Progress::of(&event, ProgressStyle::default()) else {
        unreachable!("ToolStarted is user-visible")
    };
    let wire = serde_json::to_value(&progress).unwrap_or(json!(null));

    // The wire form carries event, slot, and text.
    assert_eq!(wire["event"], json!("tool_started"));
    assert_eq!(wire["slot"], json!("tool:c1"));
    assert!(
        wire["text"]
            .as_str()
            .unwrap_or_default()
            .contains("searching"),
        "{wire}"
    );
}

#[tokio::test]
async fn a_run_with_a_listener_records_and_reports_at_once() {
    let sink = Arc::new(MemorySink::new());
    let (tx, mut rx) = mpsc::unbounded_channel();
    let fanout = Fanout::new(sink.clone());
    let listening = ctx().listening_to(tx);

    fanout.emit(&listening, started("search", json!({})));
    fanout.emit(
        &listening,
        TraceEvent::ContextAssembled {
            step: 1,
            message_count: 1,
            estimated_tokens: 1,
        },
    );

    assert_eq!(sink.records().len(), 2, "every event is still traced");
    let seen = rx
        .try_recv()
        .ok()
        .map(|p: Progress| p.text)
        .unwrap_or_default();
    assert!(seen.contains("searching"), "{seen}");
    assert!(rx.try_recv().is_err(), "only user-visible events are sent");
}

#[tokio::test]
async fn a_run_nobody_is_watching_still_traces() {
    let sink = Arc::new(MemorySink::new());
    let fanout = Fanout::new(sink.clone());
    let quiet = RequestContext::new("g", "u", Duration::from_secs(5));

    fanout.emit(
        &quiet,
        TraceEvent::ModelCall {
            step: 1,
            model: "test".into(),
            finish_reason: FinishReason::Stop,
            usage: Usage::default(),
            duration_ms: 1,
            attempt: 0,
        },
    );

    assert_eq!(sink.records().len(), 1);
}

#[test]
fn plain_lines_hide_tool_names_arguments_and_results() {
    let plain = ProgressStyle {
        plain: true,
        ..ProgressStyle::default()
    };
    let started = TraceEvent::ToolStarted {
        step: 1,
        call_id: "c1".into(),
        tool: "search".into(),
        arguments: serde_json::json!({"query": "MAT 343 tutoring", "source": "web"}),
    };
    assert_eq!(
        started.progress(plain).as_deref(),
        Some("\u{1f527} Searching the web for \u{201c}MAT 343 tutoring\u{201d}")
    );
    let failed = finished("search", Err("platform did not answer within 10s".into()));
    let line = failed.progress(plain).unwrap_or_default();
    assert_eq!(line, "\u{274c} Could not reach ASU pages");
    let found = finished("search", Ok("{\"results\": []}".into()));
    assert_eq!(
        found.progress(plain).as_deref(),
        Some("\u{2705} Checked ASU pages")
    );
    assert_eq!(
        TraceEvent::MemoryRecalled { count: 10 }.progress(plain),
        None
    );
}
