//! remember and forget_memory: the rule check, sealing, and removal by short id.

use std::sync::Arc;

use chrono::{Duration, Utc};
use serde_json::json;

use crate::core::config::MemorySettings;
use crate::core::tests::support::{MemorySink, Recalling, ctx};
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::trace::TraceSink;
use crate::core::types::memory::MemoryKind;
use crate::core::types::trace::TraceEvent;
use crate::runtime::tools::memory::{ForgetMemoryTool, MemoryRules, RememberTool};

fn rules() -> MemoryRules {
    MemoryRules::new(&MemorySettings::default(), 0)
}

fn tools() -> (
    Arc<Recalling>,
    Arc<MemorySink>,
    RememberTool,
    ForgetMemoryTool,
) {
    let store = Arc::new(Recalling::default());
    let sink = Arc::new(MemorySink::new());
    let memory: Arc<dyn MemoryStore> = store.clone();
    let trace: Arc<dyn TraceSink> = sink.clone();
    let remember = RememberTool::new(memory.clone(), trace.clone(), rules());
    let forget = ForgetMemoryTool::new(memory, trace);
    (store, sink, remember, forget)
}

async fn said(tool: &dyn Tool, args: serde_json::Value) -> String {
    match tool.call(&ctx(), args).await {
        Ok(out) => out.content,
        Err(e) => format!("error: {e}"),
    }
}

fn day(offset: i64) -> String {
    (Utc::now() + Duration::days(offset))
        .format("%Y-%m-%d")
        .to_string()
}

#[tokio::test]
async fn remember_refuses_what_it_must_not_keep() {
    let (store, sink, remember, _) = tools();
    let long = "x".repeat(201);
    for args in [
        json!({"kind": "fact", "text": "  "}),
        json!({"kind": "fact", "text": long}),
        json!({"kind": "task", "text": "essay"}),
        json!({"kind": "task", "text": "essay", "due": day(-1)}),
        json!({"kind": "task", "text": "essay", "due": "next friday"}),
    ] {
        assert!(said(&remember, args).await.starts_with("Not remembered"));
    }
    for term in &MemorySettings::default().refuse_terms {
        let args = json!({"kind": "fact", "text": format!("my {term} is here")});
        assert!(
            said(&remember, args).await.starts_with("Not remembered"),
            "{term}"
        );
    }
    assert_eq!(store.held().len(), 1, "only the starting memory");
    assert!(sink.records().is_empty());
}

#[tokio::test]
async fn remember_keeps_a_fact_and_seals_a_sensitive_term() {
    let (store, sink, remember, _) = tools();
    let reply = said(
        &remember,
        json!({"kind": "fact", "text": "CS junior at Poly"}),
    )
    .await;
    assert!(reply.starts_with("Remembered as "), "{reply}");
    let reply = said(
        &remember,
        json!({"kind": "fact", "text": "my Phone is 555-0100"}),
    )
    .await;
    assert!(reply.starts_with("Remembered as "), "{reply}");
    let reply = said(
        &remember,
        json!({"kind": "task", "text": "CSE 310 project", "due": day(3)}),
    )
    .await;
    assert!(reply.starts_with("Remembered as "), "{reply}");
    let held = store.held();
    assert_eq!(held.len(), 4);
    assert!(!held[1].sensitive);
    assert!(held[2].sensitive);
    assert_eq!(held[3].kind, MemoryKind::Task);
    let written = sink
        .records()
        .iter()
        .filter(|r| matches!(r.event, TraceEvent::MemoryWritten { .. }))
        .count();
    assert_eq!(written, 3);
}

#[tokio::test]
async fn forget_memory_removes_by_short_id_and_says_when_none_matches() {
    let (store, sink, remember, forget) = tools();
    let reply = said(
        &remember,
        json!({"kind": "preference", "text": "short answers"}),
    )
    .await;
    let id = reply
        .trim_start_matches("Remembered as ")
        .trim_end_matches('.')
        .to_owned();
    assert_eq!(id.len(), 8);
    assert!(
        said(&forget, json!({"id": "deadbeef"}))
            .await
            .starts_with("No memory")
    );
    assert!(
        said(&forget, json!({"id": format!("[{id}]")}))
            .await
            .starts_with("Forgot")
    );
    assert_eq!(store.held().len(), 1);
    assert!(
        sink.records()
            .iter()
            .any(|r| matches!(r.event, TraceEvent::MemoryForgotten))
    );
}

#[test]
fn both_tools_are_private_sequential_prepare_writes() {
    let (_, _, remember, forget) = tools();
    for d in [remember.definition(), forget.definition()] {
        assert!(d.private && d.sequential, "{}", d.name);
        assert_eq!(d.risk, crate::core::types::tools::RiskClass::PrepareWrite);
    }
}
