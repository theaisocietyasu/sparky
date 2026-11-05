//! Trace sinks: JSONL round trip.

mod export;
mod progress;
mod spans;

use std::time::Duration;

use uuid::Uuid;

use crate::core::traits::trace::TraceSink;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::model::Usage;
use crate::core::types::trace::{RunStatus, TraceEvent, TraceRecord};
use crate::runtime::harness::trace::JsonlSink;

#[test]
fn live_pieces_of_a_streaming_call_stay_out_of_the_recorded_trace() {
    let dir = std::env::temp_dir().join(format!("sparky-trace-{}", Uuid::new_v4()));
    let Ok(sink) = JsonlSink::new(&dir, 0) else {
        unreachable!("the trace directory could be created")
    };
    let ctx = RequestContext::new("g", "u", Duration::from_secs(1));
    sink.emit(
        &ctx,
        TraceEvent::ModelReasoning {
            step: 1,
            text: "row two".into(),
        },
    );
    sink.emit(
        &ctx,
        TraceEvent::AnswerDraft {
            step: 1,
            text: "Open until".into(),
        },
    );
    sink.emit(&ctx, TraceEvent::AnswerDraftCleared { step: 1 });
    sink.emit(
        &ctx,
        TraceEvent::ModelThought {
            step: 1,
            text: "row two".into(),
        },
    );
    let text = std::fs::read_to_string(sink.path_for(ctx.request_id)).unwrap_or_default();
    let kinds: Vec<String> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<TraceRecord>(l).ok())
        .map(|r| r.event.kind().to_owned())
        .collect();
    assert_eq!(
        kinds,
        vec!["model_thought"],
        "only the finished thought is recorded"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn jsonl_round_trips() {
    let dir = std::env::temp_dir().join(format!("sparky-trace-{}", Uuid::new_v4()));
    let created = JsonlSink::new(&dir, 0);
    assert!(created.is_ok(), "{created:?}");
    let Ok(sink) = created else {
        return;
    };
    let ctx = RequestContext::new("g", "u", Duration::from_secs(1));
    sink.emit(
        &ctx,
        TraceEvent::RequestStarted {
            input: "hi".into(),
            tenant_id: "g".into(),
            user_id: "u".into(),
            trace_id: None,
        },
    );
    sink.emit(
        &ctx,
        TraceEvent::Completed {
            status: RunStatus::Answered,
            steps: 1,
            usage: Usage::default(),
            cost_usd: 0.0,
            duration_ms: 3,
        },
    );
    let text = std::fs::read_to_string(sink.path_for(ctx.request_id)).unwrap_or_default();
    let records: Vec<TraceRecord> = text
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    assert_eq!(records.len(), 2);
    assert!(matches!(
        records[1].event,
        TraceEvent::Completed { steps: 1, .. }
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn traceparent_parses_into_a_remote_parent() {
    use opentelemetry::trace::TraceContextExt;

    use crate::routes::chat::parse_traceparent;

    let cx = parse_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");
    let sc = cx.as_ref().map(|c| c.span().span_context().clone());
    assert!(sc.as_ref().is_some_and(|s| s.is_remote() && s.is_sampled()));
    assert_eq!(
        sc.map(|s| s.trace_id().to_string()),
        Some("4bf92f3577b34da6a3ce929d0e0e4736".into())
    );
    assert!(parse_traceparent("garbage").is_none());
    assert!(parse_traceparent("00-00000000000000000000000000000000-0000000000000000-01").is_none());
}

#[test]
fn kind_matches_the_name_each_event_serialises_under() {
    use crate::core::types::model::{FinishReason, Usage};
    use crate::core::types::safety::policy::Decision;

    let events = [
        TraceEvent::RequestStarted {
            input: String::new(),
            tenant_id: String::new(),
            user_id: String::new(),
            trace_id: None,
        },
        TraceEvent::ContextAssembled {
            step: 1,
            message_count: 0,
            estimated_tokens: 0,
        },
        TraceEvent::ModelCall {
            step: 1,
            model: String::new(),
            finish_reason: FinishReason::Stop,
            usage: Usage::default(),
            duration_ms: 0,
            attempt: 0,
        },
        TraceEvent::ModelError {
            step: 1,
            attempt: 0,
            error: String::new(),
            retried: false,
        },
        TraceEvent::PolicyDecision {
            step: 1,
            tool: String::new(),
            decision: Decision::Allow,
        },
        TraceEvent::ToolStarted {
            step: 1,
            call_id: String::new(),
            tool: String::new(),
            arguments: serde_json::Value::Null,
        },
        TraceEvent::ToolCall {
            step: 1,
            call_id: String::new(),
            tool: String::new(),
            arguments: serde_json::Value::Null,
            result: Ok(String::new()),
            duration_ms: 0,
        },
        TraceEvent::Completed {
            status: RunStatus::Answered,
            steps: 1,
            usage: Usage::default(),
            cost_usd: 0.0,
            duration_ms: 0,
        },
    ];

    for event in &events {
        let tag = serde_json::to_value(event)
            .ok()
            .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_owned));
        assert_eq!(
            tag.as_deref(),
            Some(event.kind()),
            "kind() drifted from the serde tag"
        );
    }
}

#[test]
fn a_capped_trace_keeps_the_start_of_the_run_and_drops_the_tail() {
    use crate::core::traits::trace::TraceSink;
    use crate::core::types::trace::TraceEvent;
    use crate::runtime::harness::trace::JsonlSink;

    let dir = std::env::temp_dir().join(format!("sparky-trace-cap-{}", uuid::Uuid::new_v4()));
    let Ok(sink) = JsonlSink::new(&dir, 200) else {
        unreachable!("could not create the trace directory")
    };
    let ctx = crate::core::tests::support::ctx();
    for step in 0..50 {
        sink.emit(
            &ctx,
            TraceEvent::ToolStarted {
                step,
                call_id: format!("c{step}"),
                tool: "search_library_hours".into(),
                arguments: serde_json::json!({}),
            },
        );
    }
    let Ok(written) = std::fs::read_to_string(sink.path_for(ctx.request_id)) else {
        unreachable!("no trace was written")
    };
    assert!(
        written.len() < 500,
        "the cap bounds the file: {}",
        written.len()
    );
    assert!(
        written.contains("\"step\":0"),
        "the start of the run survives"
    );
    assert!(!written.contains("\"step\":49"), "the tail is dropped");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pruning_removes_traces_older_than_the_retention_window() {
    use crate::runtime::harness::trace::JsonlSink;

    let dir = std::env::temp_dir().join(format!("sparky-trace-prune-{}", uuid::Uuid::new_v4()));
    let Ok(sink) = JsonlSink::new(&dir, 0) else {
        unreachable!("could not create the trace directory")
    };
    let Ok(()) = std::fs::write(dir.join("fresh.jsonl"), "{}\n") else {
        unreachable!("could not write a trace")
    };
    let Ok(()) = std::fs::write(dir.join("keep.txt"), "not a trace\n") else {
        unreachable!("could not write the decoy")
    };
    // Nothing here is old enough yet; a long window removes nothing.
    assert_eq!(sink.prune(std::time::Duration::from_hours(1)).ok(), Some(0));
    // A zero window makes everything stale, and only .jsonl files go.
    assert_eq!(sink.prune(std::time::Duration::ZERO).ok(), Some(1));
    assert!(dir.join("keep.txt").exists(), "other files are left alone");
    let _ = std::fs::remove_dir_all(&dir);
}
