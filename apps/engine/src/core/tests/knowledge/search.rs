//! The search tool: Platform's schema, the arguments it passes on, and how answers read.

use chrono::{Duration, Utc};
use serde_json::json;

use crate::core::tests::support::{FakeQueries, ctx};
use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::traits::tools::Tool;
use crate::core::types::knowledge::query::{QuerySourceInfo, SearchAnswer, StoredPage};
use crate::core::types::tools::{RiskClass, SEARCH, ToolError};
use crate::runtime::tools::knowledge::search::{Search, output, render_stored};

fn source(key: &str) -> QuerySourceInfo {
    QuerySourceInfo {
        key: key.into(),
        description: String::new(),
        params: Vec::new(),
    }
}

async fn tool(queries: FakeQueries) -> Search {
    let queries = std::sync::Arc::new(queries);
    let Ok(spec) = queries.spec().await else {
        unreachable!("the double describes search")
    };
    Search::new(queries, spec, 90)
}

fn page(title: &str, url: Option<&str>, days: i64) -> StoredPage {
    StoredPage {
        title: title.into(),
        url: url.map(str::to_owned),
        fetched_at: Some(Utc::now() - Duration::days(days)),
        content: format!("{title} text"),
    }
}

#[tokio::test]
async fn the_model_gets_platform_wording_and_schema_and_calls_run_together() {
    let search = tool(FakeQueries::new(vec![source("events"), source("web")])).await;
    let def = search.definition();
    assert_eq!(def.name, SEARCH);
    assert_eq!(def.risk, RiskClass::ReadPublic);
    assert!(!def.sequential, "several searches in one step run at once");
    assert_eq!(def.timeout_secs, Some(90));
    assert!(def.description.contains("then the web"));
    assert_eq!(
        def.parameters["properties"]["source"]["enum"],
        json!(["events", "web"])
    );
}

#[tokio::test]
async fn a_call_passes_its_arguments_to_platform_as_sent() {
    let queries = FakeQueries::new(vec![source("events")]);
    let sent = queries.sent();
    let search = tool(queries).await;
    let _ = search
        .call(&ctx(), json!({"query": "career fair", "source": "events"}))
        .await;
    let sent = sent.lock().map(|s| s.clone()).unwrap_or_default();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["query"], "career fair");
    assert_eq!(sent[0]["source"], "events");
}

#[tokio::test]
async fn a_call_that_is_not_an_object_never_reaches_platform() {
    let queries = FakeQueries::new(Vec::new());
    let sent = queries.sent();
    let result = tool(queries).await.call(&ctx(), json!("hours")).await;
    assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    assert_eq!(sent.lock().map_or(1, |s| s.len()), 0);
}

#[tokio::test]
async fn a_platform_refusal_comes_back_as_something_the_model_can_fix() {
    let queries = FakeQueries::new(Vec::new()).rejecting("courses needs a subject in the query");
    let result = tool(queries)
        .await
        .call(&ctx(), json!({"query": "seats"}))
        .await;
    assert!(matches!(result, Err(ToolError::InvalidArguments(m)) if m.contains("subject")));
}

#[test]
fn stored_pages_read_with_url_age_and_the_old_page_note() {
    let pages = vec![
        page("Advising", Some("https://asu.edu/advising"), 3),
        page("Old fees", Some("https://asu.edu/fees"), 40),
    ];
    let text = render_stored(&pages, Some("Some pages are old."), Utc::now());
    assert!(
        text.starts_with(
            "[1] Advising - https://asu.edu/advising (stored copy, fetched 3 days ago)"
        ),
        "{text}"
    );
    assert!(
        text.contains("[2] Old fees - https://asu.edu/fees (stored copy, fetched 40 days ago)"),
        "{text}"
    );
    assert!(text.ends_with("Some pages are old."), "{text}");
}

#[test]
fn a_stored_answer_cites_each_page() {
    let answer = SearchAnswer::Stored {
        results: vec![
            page("Advising", Some("https://asu.edu/advising"), 1),
            page("Advising", Some("https://asu.edu/advising"), 1),
        ],
        note: None,
    };
    let out = output(&answer, Utc::now());
    assert_eq!(out.sources.len(), 1);
    assert_eq!(
        out.sources[0].url.as_deref(),
        Some("https://asu.edu/advising")
    );
}

#[test]
fn a_live_answer_names_its_source_and_cites_the_site() {
    let answer = SearchAnswer::Live {
        source: "library_hours".into(),
        url: "https://www.lib.asu.edu/hours".into(),
        text: "Hayden: 7 AM to 2 AM".into(),
    };
    let out = output(&answer, Utc::now());
    assert!(
        out.content
            .starts_with("Live result from library_hours (https://www.lib.asu.edu/hours)")
    );
    assert!(out.content.ends_with("Hayden: 7 AM to 2 AM"));
    assert_eq!(out.sources[0].key, "library_hours");
    assert_eq!(out.sources[0].title, "lib.asu.edu");
}

#[test]
fn nothing_found_reads_as_the_note_with_no_sources() {
    let answer = SearchAnswer::None {
        note: "Nothing came back for that query.".into(),
    };
    let out = output(&answer, Utc::now());
    assert_eq!(out.content, "Nothing came back for that query.");
    assert_eq!(out.sources.len(), 0);
}

#[tokio::test]
async fn a_live_answer_comes_back_with_its_citation() {
    let queries = FakeQueries::new(vec![source("library_hours")]).answering(SearchAnswer::Live {
        source: "library_hours".into(),
        url: "https://lib.asu.edu/hours".into(),
        text: "Hayden: 7 AM to 2 AM".into(),
    });
    let Ok(out) = tool(queries)
        .await
        .call(
            &ctx(),
            json!({"query": "hayden hours", "source": "library_hours"}),
        )
        .await
    else {
        unreachable!("the search answers")
    };
    assert!(out.content.contains("Hayden: 7 AM to 2 AM"));
    assert_eq!(
        out.sources[0].url.as_deref(),
        Some("https://lib.asu.edu/hours")
    );
}
