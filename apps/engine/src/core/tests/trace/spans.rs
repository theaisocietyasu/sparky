//! The attributes the trace UI reads from the model and agent spans.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SpanData};
use tracing_subscriber::layer::SubscriberExt;

use crate::core::tests::support::{Scripted, agent, ctx, text};
use crate::core::types::agent::AgentConfig;
use crate::runtime::harness::tools::ToolSet;

fn attr(span: &SpanData, key: &str) -> Option<String> {
    span.attributes
        .iter()
        .find(|kv| kv.key.as_str() == key)
        .map(|kv| kv.value.to_string())
}

/// Runs one answered turn and returns the exported spans.
fn spans_of_one_turn() -> Vec<SpanData> {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
    let cfg = AgentConfig {
        provider_name: "llama.cpp".into(),
        model_name: "qwen".into(),
        ..AgentConfig::default()
    };
    let (agent, _) = agent(Scripted::new(vec![Ok(text("hello"))]), ToolSet::new(), cfg);
    let ctx = ctx();
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return Vec::new();
    };
    let answered = tracing::subscriber::with_default(subscriber, || {
        rt.block_on(agent.run(&ctx, "what time is it"))
    });
    assert!(answered.is_ok(), "{answered:?}");
    let _ = provider.force_flush();
    exporter.get_finished_spans().unwrap_or_default()
}

#[test]
fn the_llm_span_carries_the_gen_ai_attributes() {
    let spans = spans_of_one_turn();
    let llm = spans.iter().find(|s| s.name == "llm");
    assert!(
        llm.is_some(),
        "{:?}",
        spans.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    let Some(llm) = llm else {
        return;
    };
    let ctx = ctx();
    for (key, want) in [
        ("gen_ai.operation.name", "chat"),
        ("gen_ai.provider.name", "llama.cpp"),
        ("gen_ai.request.model", "qwen"),
        ("gen_ai.response.model", "test"),
        ("gen_ai.usage.input_tokens", "10"),
        ("gen_ai.usage.output_tokens", "5"),
        ("sparky.span", "llm"),
        ("sparky.tools", "[]"),
        ("sparky.step", "1"),
        ("sparky.attempt", "0"),
    ] {
        assert_eq!(attr(llm, key).as_deref(), Some(want), "{key}");
    }
    assert!(attr(llm, "gen_ai.request.max_tokens").is_some());
    assert!(attr(llm, "gen_ai.request.temperature").is_some());
    let input = attr(llm, "gen_ai.input.messages").unwrap_or_default();
    assert!(
        input.starts_with('[') && input.contains("what time is it"),
        "{input}"
    );
    let output = attr(llm, "gen_ai.output.messages").unwrap_or_default();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&output).unwrap_or_default();
    assert_eq!(parsed.len(), 1, "{output}");
    assert!(output.contains("hello"), "{output}");
    assert_eq!(attr(llm, "user.id").as_deref(), Some(ctx.user_id.as_str()));
    assert!(attr(llm, "session.id").is_some());
}

#[test]
fn the_run_span_is_an_agent_invocation() {
    let spans = spans_of_one_turn();
    let run = spans.iter().find(|s| s.name == "agent.run");
    assert!(run.is_some());
    let Some(run) = run else {
        return;
    };
    assert_eq!(
        attr(run, "gen_ai.operation.name").as_deref(),
        Some("invoke_agent")
    );
    assert_eq!(attr(run, "gen_ai.agent.name").as_deref(), Some("sparky"));
    assert_eq!(
        attr(run, "sparky.input").as_deref(),
        Some("what time is it")
    );
    assert_eq!(attr(run, "sparky.output").as_deref(), Some("hello"));
    assert!(attr(run, "sparky.status").is_some());
}

#[test]
fn every_span_carries_the_openinference_attributes_phoenix_reads() {
    let spans = spans_of_one_turn();
    let ctx = ctx();
    for (name, kind) in [("agent.run", "CHAIN"), ("llm", "LLM")] {
        let span = spans.iter().find(|s| s.name == name);
        assert!(
            span.is_some(),
            "{:?}",
            spans.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        let Some(span) = span else {
            return;
        };
        assert_eq!(
            attr(span, "openinference.span.kind").as_deref(),
            Some(kind),
            "{name}"
        );
        assert!(attr(span, "session.id").is_some(), "{name} session.id");
        assert_eq!(attr(span, "user.id").as_deref(), Some(ctx.user_id.as_str()));
        assert!(
            attr(span, "input.value").is_some_and(|v| !v.is_empty()),
            "{name} input"
        );
        assert!(
            attr(span, "output.value").is_some_and(|v| !v.is_empty()),
            "{name} output"
        );
    }
    let llm = spans.iter().find(|s| s.name == "llm");
    let Some(llm) = llm else {
        return;
    };
    // The Phoenix keys carry the same values as the gen_ai ones.
    assert_eq!(attr(llm, "llm.model_name").as_deref(), Some("test"));
    assert_eq!(attr(llm, "llm.token_count.prompt").as_deref(), Some("10"));
    assert_eq!(
        attr(llm, "llm.token_count.completion").as_deref(),
        Some("5")
    );
    assert_eq!(
        attr(llm, "input.value").as_deref(),
        attr(llm, "gen_ai.input.messages").as_deref()
    );
    assert_eq!(
        attr(llm, "output.value").as_deref(),
        attr(llm, "gen_ai.output.messages").as_deref()
    );
}

#[test]
fn a_finished_span_says_whether_it_succeeded() {
    use opentelemetry::trace::Status;

    let spans = spans_of_one_turn();
    for name in ["agent.run", "llm"] {
        let Some(span) = spans.iter().find(|s| s.name == name) else {
            unreachable!("{name} is exported by an answered turn")
        };
        assert_eq!(
            span.status,
            Status::Ok,
            "{name} left its status unset, so every trace UI reads it as unknown"
        );
    }
}

/// Reads a grade: an authenticated result that must not reach a trace in clear.
struct Grades;

#[async_trait::async_trait]
impl crate::core::traits::tools::Tool for Grades {
    fn definition(&self) -> crate::core::types::tools::ToolDefinition {
        crate::core::types::tools::ToolDefinition {
            name: "grades".into(),
            description: "reads the caller's grades".into(),
            parameters: serde_json::json!({"type": "object"}),
            risk: crate::core::types::tools::RiskClass::ReadAuthenticated,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }
    async fn call(
        &self,
        _ctx: &crate::core::types::agent::context::RequestContext,
        _args: serde_json::Value,
    ) -> Result<crate::core::types::tools::ToolOutput, crate::core::types::tools::ToolError> {
        Ok(crate::core::types::tools::ToolOutput {
            content: "CSE 310: B+".into(),
            data: None,
            sources: Vec::new(),
        })
    }
}

#[test]
fn an_authenticated_result_is_masked_in_the_trace_and_every_span() {
    use std::sync::Arc;

    use crate::core::tests::support::{Named, calls};
    use crate::core::types::conversation::Visibility;
    use crate::core::types::trace::TraceEvent;

    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
    let tools = ToolSet::new()
        .with(Arc::new(Grades))
        .with(Arc::new(Named("lookup")));
    let model = Scripted::new(vec![
        Ok(calls(vec![
            ("c1", "grades", serde_json::json!({})),
            ("c2", "lookup", serde_json::json!({})),
        ])),
        Ok(text("You have a B+ in CSE 310.")),
    ]);
    let (agent, sink) = agent(model, tools, AgentConfig::default());
    let ctx = ctx().with_visibility(Visibility::Private);
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        unreachable!("a runtime builds")
    };
    let answered = tracing::subscriber::with_default(subscriber, || {
        rt.block_on(agent.run(&ctx, "what are my grades"))
    });
    assert!(answered.is_ok(), "{answered:?}");
    let _ = provider.force_flush();

    let results: Vec<(String, Result<String, String>)> = sink
        .records()
        .into_iter()
        .filter_map(|r| match r.event {
            TraceEvent::ToolCall { tool, result, .. } => Some((tool, result)),
            _ => None,
        })
        .collect();
    let traced = |name: &str| {
        results
            .iter()
            .find(|(tool, _)| tool == name)
            .and_then(|(_, result)| result.clone().ok())
            .unwrap_or_default()
    };
    assert_eq!(traced("grades"), "[authenticated result, 11 chars]");
    assert_eq!(
        traced("lookup"),
        "what the page says",
        "a public result stays readable"
    );

    let spans = exporter.get_finished_spans().unwrap_or_default();
    for span in spans.iter().filter(|s| s.name == "tool") {
        let output = attr(span, "output.value").unwrap_or_default();
        assert!(!output.contains("B+"), "tool span: {output}");
    }
    let prompts: Vec<String> = spans
        .iter()
        .filter(|s| s.name == "llm")
        .filter_map(|s| attr(s, "input.value"))
        .collect();
    let second = prompts.iter().find(|p| p.contains("what the page says"));
    assert!(second.is_some(), "{prompts:?}");
    let second = second.cloned().unwrap_or_default();
    assert!(!second.contains("CSE 310: B+"), "llm span: {second}");
    assert!(
        second.contains("[authenticated result, 11 chars]"),
        "{second}"
    );
}
