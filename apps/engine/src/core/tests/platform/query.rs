//! Platform search: the source registry, the tool description, and one search over /api/tools.

use std::time::Duration;

use serde_json::{Map, Value, json};

use super::{Fake, caller, client};
use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::query::{QueryError, SearchAnswer};
use crate::stores::platform::PlatformQueries;

fn args(query: &str) -> Map<String, Value> {
    let mut args = Map::new();
    args.insert("query".into(), json!(query));
    args
}

#[tokio::test]
async fn the_registry_reads_keys_descriptions_and_params() {
    let fake = Fake::default();
    fake.on(
        "GET",
        "/api/asu/queries",
        200,
        json!({"queries": [{
            "key": "library_hours",
            "description": "Hours",
            "params": [{"name": "campus", "description": "d", "required": true,
                        "example": "tempe", "choices": ["tempe", "west"], "many": false}]
        }]}),
    );
    let Ok(sources) = PlatformQueries::new(client(&fake).await).sources().await else {
        unreachable!("the registry loads")
    };
    assert_eq!(sources[0].key, "library_hours");
    assert_eq!(sources[0].description, "Hours");
    assert!(sources[0].params[0].required);
    assert_eq!(sources[0].params[0].choices, vec!["tempe", "west"]);
}

#[tokio::test]
async fn the_spec_is_the_search_tool_platform_lists() {
    let fake = Fake::default();
    fake.on(
        "GET",
        "/api/tools",
        200,
        json!({"tools": [
            {"name": "org.info", "description": "Org", "input_schema": {"type": "object"}},
            {"name": "search", "description": "Search it all", "input_schema":
                {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}}
        ]}),
    );
    let Ok(spec) = PlatformQueries::new(client(&fake).await).spec().await else {
        unreachable!("the spec loads")
    };
    assert_eq!(spec.description, "Search it all");
    assert_eq!(spec.parameters["required"], json!(["query"]));
}

#[tokio::test]
async fn a_token_without_search_fails_with_the_scope_it_needs() {
    let fake = Fake::default();
    fake.on("GET", "/api/tools", 200, json!({"tools": []}));
    let result = PlatformQueries::new(client(&fake).await).spec().await;
    assert!(matches!(result, Err(QueryError::Store(m)) if m.contains("knowledge:read")));
}

#[tokio::test]
async fn a_search_sends_the_arguments_and_reads_each_kind_of_answer() {
    let fake = Fake::default();
    fake.on(
        "POST",
        "/api/tools/search",
        200,
        json!({"result": {"from": "stored", "results": [{"title": "Advising", "url": "https://asu.edu/a",
            "fetched_at": "2026-11-01T10:00:00.123456", "content": "Book online."}], "note": "Some pages are old."}}),
    );
    let Ok(answer) = PlatformQueries::new(client(&fake).await)
        .search(&caller(), &args("advising"))
        .await
    else {
        unreachable!("the search answers")
    };
    assert_eq!(fake.last().body, json!({"query": "advising"}));
    let SearchAnswer::Stored { results, note } = answer else {
        unreachable!("a stored answer")
    };
    assert_eq!(results[0].url.as_deref(), Some("https://asu.edu/a"));
    assert!(results[0].fetched_at.is_some());
    assert_eq!(note.as_deref(), Some("Some pages are old."));

    let live = Fake::default();
    live.on(
        "POST",
        "/api/tools/search",
        200,
        json!({"result": {"from": "live", "source": "library_hours", "url": "https://lib.asu.edu/hours", "text": "Open 7 to 2"}}),
    );
    let answer = PlatformQueries::new(client(&live).await)
        .search(&caller(), &args("hayden hours"))
        .await;
    assert!(
        matches!(answer, Ok(SearchAnswer::Live { url, .. }) if url == "https://lib.asu.edu/hours")
    );
}

#[tokio::test]
async fn a_live_source_that_is_down_is_tried_twice_and_not_blamed_on_the_arguments() {
    let fake = Fake::default();
    fake.on(
        "POST",
        "/api/tools/search",
        502,
        json!({"error": "https://example.edu could not be reached"}),
    );
    let result = PlatformQueries::new(client(&fake).await)
        .search(&caller(), &args("hayden hours"))
        .await;
    assert!(
        matches!(result, Err(QueryError::Unavailable(m)) if m.contains("could not be reached"))
    );
    let searches = fake
        .seen()
        .iter()
        .filter(|s| s.path == "/api/tools/search")
        .count();
    assert_eq!(searches, 2);
}

#[tokio::test]
async fn a_refused_search_goes_back_to_the_model() {
    let fake = Fake::default();
    fake.on(
        "POST",
        "/api/tools/search",
        422,
        json!({"error": "courses needs a subject in the query"}),
    );
    let result = PlatformQueries::new(client(&fake).await)
        .search(&caller(), &args("seats"))
        .await;
    assert!(matches!(result, Err(QueryError::Rejected(m)) if m.contains("subject")));
}

#[tokio::test]
async fn a_bad_token_is_a_store_failure_not_a_rejection() {
    let fake = Fake::default();
    fake.on(
        "POST",
        "/api/tools/search",
        403,
        json!({"error": "missing scope"}),
    );
    let result = PlatformQueries::new(client(&fake).await)
        .search(&caller(), &args("x"))
        .await;
    assert!(matches!(result, Err(QueryError::Store(_))));
}

#[tokio::test]
async fn a_slow_search_stops_at_the_request_deadline() {
    let fake = Fake::default();
    fake.after(
        "POST",
        "/api/tools/search",
        200,
        json!({"result": {"from": "none", "note": "n"}}),
        Duration::from_secs(5),
    );
    let ctx = RequestContext::new("guild", "111", Duration::from_millis(200));
    let result = PlatformQueries::new(client(&fake).await)
        .search(&ctx, &args("x"))
        .await;
    assert!(matches!(result, Err(QueryError::Timeout(_))));
}

#[tokio::test]
async fn a_cancelled_request_cancels_the_search() {
    let fake = Fake::default();
    fake.after(
        "POST",
        "/api/tools/search",
        200,
        json!({"result": {"from": "none", "note": "n"}}),
        Duration::from_secs(5),
    );
    let ctx = caller();
    let cancel = ctx.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();
    });
    let result = PlatformQueries::new(client(&fake).await)
        .search(&ctx, &args("x"))
        .await;
    assert!(matches!(result, Err(QueryError::Cancelled)));
}
