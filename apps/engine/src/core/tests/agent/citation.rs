//! What an answer may cite, and what tool output leaves behind in a trace.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};

use crate::core::tests::support::{agent, calls, text};
use crate::core::traits::tools::Tool;
use crate::core::types::agent::AgentConfig;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::evidence::Citation;
use crate::core::types::safety::redact::redact_text;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};

fn ctx() -> RequestContext {
    RequestContext::new("g", "u", Duration::from_secs(5))
}

/// A tool that reads one live page.
struct Finder(Citation);

#[async_trait]
impl Tool for Finder {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "finder".into(),
            description: "finds".into(),
            parameters: json!({"type": "object"}),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }
    async fn call(&self, _ctx: &RequestContext, _args: Value) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput {
            content: "found one".into(),
            data: None,
            sources: vec![self.0.clone()],
        })
    }
}

#[tokio::test]
async fn a_page_a_tool_read_is_cited() {
    use crate::core::tests::support::{MemorySink, Scripted};
    use crate::runtime::harness::agent::{Agent, AgentDeps};
    use crate::runtime::harness::safety::policy::RiskPolicy;
    use crate::runtime::harness::tools::ToolSet;

    let found = Citation {
        key: "courses".into(),
        title: "ASU Class Search".into(),
        url: Some("https://catalog.apps.asu.edu/catalog/classes/classlist".into()),
    };
    let wanted = found.clone();
    let deps = AgentDeps {
        model: Arc::new(Scripted::new(vec![
            Ok(calls(vec![("1", "finder", json!({}))])),
            Ok(text("answered")),
        ])),
        tools: ToolSet::new().with(Arc::new(Finder(found))),
        policy: Arc::new(RiskPolicy::default()),
        trace: Arc::new(MemorySink::new()),
        conversations: None,
        memory: None,
        confirmations: None,
        sandbox: None,
        files: None,
        compactor: None,
        guardrail: None,
    };
    let Ok(answer) = Agent::new(deps, AgentConfig::default(), "sys")
        .run(&ctx(), "q")
        .await
    else {
        unreachable!("the run answered")
    };
    assert_eq!(
        answer.citations(),
        vec![wanted],
        "what a tool read is cited"
    );
}

#[tokio::test]
async fn an_answer_without_tools_cites_nothing() {
    let (a, _sink) = agent(
        crate::core::tests::support::Scripted::new(vec![Ok(text("hello"))]),
        crate::runtime::harness::tools::ToolSet::new(),
        AgentConfig::default(),
    );
    let Ok(answer) = a.run(&ctx(), "hi").await else {
        unreachable!("the run answered")
    };
    assert_eq!(answer.citations().len(), 0);
}

#[test]
fn a_secret_in_tool_output_is_masked_before_it_reaches_a_trace() {
    let page = "Welcome back\nAuthorization: Bearer abc123\nsession_token = zzz\nordinary line";
    let out = redact_text(page);
    assert!(!out.contains("abc123"), "{out}");
    assert!(!out.contains("zzz"), "{out}");
    assert!(out.contains("Welcome back"), "{out}");
    assert!(out.contains("ordinary line"), "{out}");
    assert!(out.contains("[redacted]"), "{out}");
}

#[test]
fn redaction_leaves_text_that_carries_no_secret_alone() {
    let page = "Hayden Library closes at 2am.\nAsk at the front desk.";
    assert_eq!(redact_text(page), page);
}

#[test]
fn a_key_with_no_value_after_it_is_left_as_it_reads() {
    // A sentence mentioning the word token is not a secret.
    let page = "You will need a token to continue";
    assert_eq!(redact_text(page), page);
}

#[test]
fn citations_list_each_page_once_best_first() {
    use crate::core::types::knowledge::query::StoredPage;
    use crate::runtime::tools::knowledge::search::stored_citations;

    let page = |title: &str, url: Option<&str>| StoredPage {
        title: title.into(),
        url: url.map(str::to_owned),
        fetched_at: None,
        content: String::new(),
    };
    let pages = vec![
        page("library_hours", Some("https://lib.asu.edu/hours")),
        page("events", Some("https://asu.edu/events")),
        page("library_hours", Some("https://lib.asu.edu/hours")),
        page("note", None),
        page("note", None),
    ];

    let lines = stored_citations(&pages);

    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0].title, "library_hours");
    assert_eq!(lines[0].url.as_deref(), Some("https://lib.asu.edu/hours"));
    assert_eq!(lines[1].title, "events");
    assert_eq!(lines[2].title, "note");
    assert_eq!(lines[2].url, None, "a page with no url of its own");
}

/// A tool that reads every page it is given in one call.
struct Many(Vec<Citation>);

#[async_trait]
impl Tool for Many {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "many".into(),
            description: "finds several".into(),
            parameters: json!({"type": "object"}),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }
    async fn call(&self, _ctx: &RequestContext, _args: Value) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput {
            content: "found several".into(),
            data: None,
            sources: self.0.clone(),
        })
    }
}

#[tokio::test]
async fn a_source_that_would_show_as_the_same_chip_is_cited_once() {
    let cite = |title: &str, url: &str| Citation {
        key: "web".into(),
        title: title.into(),
        url: Some(url.into()),
    };
    let found = vec![
        cite("Web Search", "https://www.google.com/search?q=mat+434"),
        cite("Web Search", "https://www.google.com/search?q=mat+434+tips"),
        cite("MAT 434", "https://catalog.asu.edu/mat434"),
        cite("MAT 434", "https://catalog.asu.edu/mat434"),
        cite("Admission", "https://admission.asu.edu"),
    ];
    let (a, _sink) = agent(
        crate::core::tests::support::Scripted::new(vec![
            Ok(calls(vec![
                ("1", "many", json!({"a": 1})),
                ("2", "many", json!({"a": 2})),
            ])),
            Ok(text("answered")),
        ]),
        crate::runtime::harness::tools::ToolSet::new().with(Arc::new(Many(found))),
        AgentConfig::default(),
    );
    let Ok(answer) = a.run(&ctx(), "q").await else {
        unreachable!("the run answered")
    };
    let titles: Vec<String> = answer.sources.iter().map(|c| c.title.clone()).collect();
    assert_eq!(titles, vec!["Web Search", "MAT 434", "Admission"]);
}
