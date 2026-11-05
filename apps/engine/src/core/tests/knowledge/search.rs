//! The two search tools: their schemas, the query they demand, and what a query fills in.

use std::sync::Arc;

use chrono::NaiveDate;
use serde_json::{Value, json};

use crate::core::config::{
    KNOWLEDGE_DESCRIPTION, LIVE_DESCRIPTION, NOTHING_STORED, QUERY_DESCRIPTION, SOURCE_DESCRIPTION,
    STALE_NOTE, Tools,
};
use crate::core::tests::support::{FakeQueries, Stored, ctx};
use crate::core::traits::knowledge::retrieval::Retriever;
use crate::core::traits::tools::Tool;
use crate::core::types::knowledge::evidence::Evidence;
use crate::core::types::knowledge::query::{QueryParam, QuerySourceInfo};
use crate::core::types::model::tokens::estimate;
use crate::core::types::tools::SEARCH_LIVE;
use crate::core::types::tools::{RiskClass, ToolError};
use crate::runtime::harness::tools::ToolSet;
use crate::runtime::tools::knowledge::search::live::{
    LiveSearch, Wording as LiveWording, local_date,
};
use crate::runtime::tools::knowledge::search::sources::course_catalog::CourseCatalog;
use crate::runtime::tools::knowledge::search::sources::courses::{
    Courses, code_in, term_for, term_in,
};
use crate::runtime::tools::knowledge::search::sources::dining::Dining;
use crate::runtime::tools::knowledge::search::sources::library_hours::LibraryHours;
use crate::runtime::tools::knowledge::search::sources::shuttles::Shuttles;
use crate::runtime::tools::knowledge::search::sources::sports::Sports;
use crate::runtime::tools::knowledge::search::sources::study_rooms::StudyRooms;
use crate::runtime::tools::knowledge::search::sources::web::Web;
use crate::runtime::tools::knowledge::search::stored::{
    Stale, StoredSearch, Wording as StoredWording, render,
};
use crate::runtime::tools::knowledge::search::{
    Accepts, Freshness, LiveSource, Param, arguments, catalog, conforms, dated, named, params_for,
    source_help, source_keys,
};

/// The registry entry the platform would publish for source.
fn published(source: &dyn LiveSource) -> QuerySourceInfo {
    QuerySourceInfo {
        key: source.key().into(),
        params: source
            .params()
            .iter()
            .map(|p| {
                let (choices, many): (&[&str], bool) = match p.accepts {
                    Accepts::OneOf(c) => (c, false),
                    Accepts::AnyOf(c) => (c, true),
                    Accepts::Flag => (&["true", "false"], false),
                    Accepts::Text | Accepts::Date => (&[], false),
                };
                QueryParam {
                    name: p.name.into(),
                    required: p.required,
                    choices: choices.iter().map(|c| (*c).to_owned()).collect(),
                    many,
                }
            })
            .collect(),
        indexed: source.freshness().indexed(),
    }
}

fn live_wording() -> LiveWording {
    LiveWording {
        tool: LIVE_DESCRIPTION.into(),
        query: QUERY_DESCRIPTION.into(),
        source: SOURCE_DESCRIPTION.into(),
    }
}

fn stored_wording() -> StoredWording {
    StoredWording {
        tool: KNOWLEDGE_DESCRIPTION.into(),
        query: QUERY_DESCRIPTION.into(),
        source: SOURCE_DESCRIPTION.into(),
        empty: NOTHING_STORED.into(),
        stale: STALE_NOTE.into(),
    }
}

fn live_tool(queries: FakeQueries) -> LiveSearch {
    LiveSearch::new(catalog(), Arc::new(queries), "web", -7, &live_wording(), 90)
}

fn stored_tool(retriever: Arc<dyn Retriever>) -> StoredSearch {
    StoredSearch::new(&catalog(), retriever, 6, 14, &stored_wording())
}

fn enum_of(tool: &dyn Tool) -> Vec<String> {
    tool.definition().parameters["properties"]["source"]["enum"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_model_is_offered_one_stored_search_and_one_live_search() {
    let live = live_tool(FakeQueries::new(Vec::new())).definition();
    let stored = stored_tool(Arc::new(Stored::empty())).definition();
    assert_eq!(live.name, "search_live");
    assert_eq!(stored.name, "search_knowledge");
    for d in [&live, &stored] {
        assert_eq!(d.risk, RiskClass::ReadPublic, "{}", d.name);
        assert_eq!(
            d.parameters["required"],
            json!(["query"]),
            "{} asks for a query and nothing else",
            d.name
        );
        assert_eq!(d.parameters["properties"]["query"]["type"], "string");
        assert!(
            d.parameters["properties"]["source"]["enum"].is_array(),
            "{} narrows by source",
            d.name
        );
    }
    assert_eq!(
        live.timeout_secs,
        Some(90),
        "a live fetch outlives the default tool budget"
    );
    assert_eq!(
        stored.timeout_secs, None,
        "a read of the index takes the default budget"
    );
}

#[test]
fn the_source_filter_is_never_required_of_the_model() {
    for tool in [
        Box::new(live_tool(FakeQueries::new(Vec::new()))) as Box<dyn Tool>,
        Box::new(stored_tool(Arc::new(Stored::empty()))),
    ] {
        let required = tool.definition().parameters["required"].clone();
        assert_eq!(required, json!(["query"]));
    }
}

#[test]
fn the_stored_search_offers_only_the_sources_the_platform_indexes() {
    let live = enum_of(&live_tool(FakeQueries::new(Vec::new())));
    let stored = enum_of(&stored_tool(Arc::new(Stored::empty())));
    assert_eq!(live.len(), 17);
    assert!(live.contains(&"web".to_owned()));
    for never_stored in ["web", "shuttles", "study_rooms", "clubs", "events"] {
        assert!(
            !stored.contains(&never_stored.to_owned()),
            "{never_stored} is never written to the index, so it cannot be filtered on"
        );
    }
    assert_eq!(stored.len(), 12);
    assert!(stored.contains(&"courses".to_owned()));
}

#[test]
fn the_query_parameter_demands_a_standalone_keyword_query() {
    let text = QUERY_DESCRIPTION;
    assert!(text.contains("keywords"), "{text}");
    assert!(
        text.contains("nothing else from the conversation"),
        "it says the query cannot lean on the turn: {text}"
    );
    assert!(
        text.contains("Bad: ") && text.contains("Good: "),
        "a bad query is shown beside a good one: {text}"
    );
    let live = live_tool(FakeQueries::new(Vec::new())).definition();
    assert_eq!(live.parameters["properties"]["query"]["description"], text);
}

#[test]
fn the_source_filter_lists_a_hint_for_every_source_it_offers() {
    let sources = catalog();
    let help = source_help(SOURCE_DESCRIPTION, &sources, false);
    assert!(help.starts_with("Narrows the search"), "{help}");
    assert!(
        help.contains("Leave it out unless"),
        "leaving it out is named as the default: {help}"
    );
    for source in &sources {
        assert!(
            help.contains(&format!("{}: {}", source.key(), source.hint())),
            "{} carries its hint: {help}",
            source.key()
        );
    }
    let stored = source_help(SOURCE_DESCRIPTION, &sources, true);
    assert!(!stored.contains("web:"), "{stored}");
}

#[test]
fn two_search_schemas_cost_a_fraction_of_the_prompt_budget() {
    let mut tools = ToolSet::new();
    tools = tools.with(Arc::new(live_tool(FakeQueries::new(Vec::new()))) as Arc<dyn Tool>);
    tools = tools.with(Arc::new(stored_tool(Arc::new(Stored::empty()))) as Arc<dyn Tool>);
    let both = tools.estimated_tokens(4);
    // Half of agent.prompt_budget_tokens is the limit wiring::boot::fits_the_prompt enforces.
    assert!(both < 900, "the two search schemas need {both} tokens");
}

#[test]
fn a_source_whose_answer_is_never_stored_is_declared_live() {
    let live: Vec<&'static str> = catalog()
        .iter()
        .filter(|s| s.freshness() == Freshness::Live)
        .map(|s| s.key())
        .collect();
    assert_eq!(live, ["events", "clubs", "study_rooms", "shuttles", "web"]);
    assert_eq!(source_keys(&catalog(), true).len(), 12);
}

#[test]
fn a_source_the_platform_indexes_differently_is_named_at_boot() {
    let mut served = published(&Shuttles);
    served.indexed = true;
    let drift = conforms(&Shuttles, &served);
    assert!(
        drift.is_err_and(|e| e.contains("shuttles") && e.contains("live")),
        "a live source the platform stores is a disagreement"
    );

    let mut served = published(&Courses);
    served.indexed = false;
    assert!(conforms(&Courses, &served).is_err_and(|e| e.contains("stored")));
}

#[test]
fn a_call_with_no_query_is_refused_with_what_to_write_instead() {
    let keys = source_keys(&catalog(), false);
    let refused = |args: Value| match arguments(args, &keys, SEARCH_LIVE) {
        Err(message) => message,
        Ok(parsed) => unreachable!("expected a refusal, got {parsed:?}"),
    };
    assert!(refused(json!({})).contains("needs query"));
    assert!(refused(json!({"query": "   "})).contains("needs query"));
    assert!(refused(json!({"query": "hours", "term": "Fall 2026"})).contains("no parameter term"));
    assert!(refused(json!(["hours"])).contains("must be an object"));
    assert!(refused(json!({"query": 7})).contains("query takes text"));
    let bad = refused(json!({"query": "hours", "source": "libary"}));
    assert!(
        bad.contains("not one of") && bad.contains("library_hours"),
        "{bad}"
    );
}

#[test]
fn a_named_source_is_matched_without_case_and_an_absent_one_leaves_the_search_wide() {
    let keys = source_keys(&catalog(), false);
    assert_eq!(
        arguments(json!({"query": " hayden hours "}), &keys, SEARCH_LIVE),
        Ok(("hayden hours".to_owned(), None))
    );
    assert_eq!(
        arguments(
            json!({"query": "x", "source": "Library_Hours"}),
            &keys,
            SEARCH_LIVE
        ),
        Ok(("x".to_owned(), Some("library_hours".to_owned())))
    );
    assert_eq!(
        arguments(json!({"query": "x", "source": null}), &keys, SEARCH_LIVE),
        Ok(("x".to_owned(), None))
    );
    assert_eq!(
        arguments(json!({"query": "x", "source": "  "}), &keys, SEARCH_LIVE),
        Ok(("x".to_owned(), None))
    );
}

#[test]
fn a_choice_the_query_names_is_taken_and_one_inside_a_word_is_not() {
    let sports: &[&str] = &["football", "men's basketball", "women's basketball"];
    assert_eq!(named("who won the football game", sports), ["football"]);
    assert_eq!(
        named("women's basketball schedule", sports),
        ["women's basketball"],
        "the longer choice wins the words it covers"
    );
    let overlapping: &[&str] = &["basketball", "women's basketball"];
    assert_eq!(
        named("women's basketball schedule", overlapping),
        ["women's basketball"],
        "the longer choice wins over a shorter one standing on its own inside it"
    );
    let days: &[&str] = &["monday", "tuesday", "sunday"];
    assert_eq!(named("monday and sunday labs", days), ["monday", "sunday"]);
    assert!(
        named("footballer of the year", sports).is_empty(),
        "a choice a longer word ends with is not a match"
    );
    assert!(
        named("undergraduate courses", &["graduate"]).is_empty(),
        "a choice a longer word starts with is not a match"
    );
    assert_eq!(named("nothing here", sports).len(), 0);
}

#[test]
fn a_date_in_the_query_is_read_and_anything_else_is_not() {
    assert_eq!(
        dated("rooms on 2026-09-14"),
        NaiveDate::from_ymd_opt(2026, 9, 14)
    );
    assert_eq!(dated("rooms on sep 14"), None);
    assert_eq!(dated("no date at all"), None);
}

#[test]
fn a_query_fills_the_parameters_the_platform_takes() {
    let today = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap_or_default();
    let filled = |source: &dyn LiveSource, query: &str| match params_for(source, query, today) {
        Ok(params) => params,
        Err(message) => unreachable!("expected a filled query, got {message}"),
    };

    let courses = filled(&Courses, "CSE 310 monday open seats");
    assert_eq!(
        courses["keywords"], "CSE 310",
        "the keyword box matches a course code, so the words around it are cut"
    );
    assert_eq!(
        courses["days"], "monday",
        "a choice is still read from the whole query, not from what the keyword box kept"
    );
    assert_eq!(
        courses["term"], "Fall 2026",
        "a query naming no term takes the one running today"
    );
    assert_eq!(
        filled(&Courses, "CSE 310 in spring 2027")["term"],
        "Spring 2027",
        "a term the query names beats the one running today"
    );

    let rooms = filled(&StudyRooms, "study room at hayden");
    assert_eq!(rooms["library"], "hayden");
    assert_eq!(
        rooms["date"], "2026-09-15",
        "a required date the query omits takes today"
    );
    assert_eq!(
        filled(&StudyRooms, "hayden room on 2026-10-01")["date"],
        "2026-10-01"
    );

    assert_eq!(
        filled(&Web, "ASU score last week")["query"],
        "ASU score last week"
    );
    assert_eq!(filled(&Web, "ASU score last week")["time_range"], "week");
    assert!(
        filled(&LibraryHours, "when does hayden close").is_empty(),
        "a source that takes nothing is queued with nothing"
    );
    assert!(
        !filled(&Shuttles, "next shuttle to poly").contains_key("route"),
        "an optional choice the query does not name is left out"
    );
}

/// A source whose derived value names the parameter the query itself fills.
struct Overriding;

const OVERRIDING_PARAMS: &[Param] = &[Param::text("keywords", "What to look for.")];

impl LiveSource for Overriding {
    fn key(&self) -> &'static str {
        "overriding"
    }

    fn hint(&self) -> &'static str {
        "a source used only by this test"
    }

    fn label(&self) -> &'static str {
        "Overriding"
    }

    fn category(&self) -> &'static str {
        "overriding"
    }

    fn params(&self) -> &'static [Param] {
        OVERRIDING_PARAMS
    }

    fn derived(&self, _query: &str, _today: NaiveDate) -> Vec<(&'static str, String)> {
        vec![("keywords", "whatever the source felt like".to_owned())]
    }
}

#[test]
fn a_derived_value_never_displaces_what_the_query_itself_said() {
    let today = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap_or_default();
    let filled = params_for(&Overriding, "artificial intelligence club", today);
    assert_eq!(
        filled.map(|p| p["keywords"].clone()),
        Ok(json!("artificial intelligence club"))
    );
}

#[test]
fn a_query_missing_a_required_choice_is_refused_with_the_choices() {
    let today = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap_or_default();
    let refused = match params_for(&Sports, "when do they play next", today) {
        Err(message) => message,
        Ok(params) => unreachable!("expected a refusal, got {params:?}"),
    };
    assert!(refused.contains("sports needs sport"), "{refused}");
    assert!(refused.contains("football"), "{refused}");
    assert!(params_for(&Sports, "football schedule", today).is_ok());
}

#[test]
fn the_term_of_a_date_follows_the_academic_calendar() {
    let on = |m, d| term_for(NaiveDate::from_ymd_opt(2026, m, d).unwrap_or_default());
    assert_eq!(on(1, 5), "Spring 2026");
    assert_eq!(on(4, 30), "Spring 2026");
    assert_eq!(on(5, 1), "Summer 2026");
    assert_eq!(on(7, 31), "Summer 2026");
    assert_eq!(on(8, 1), "Fall 2026");
    assert_eq!(on(12, 31), "Fall 2026");
    assert_eq!(
        term_in("classes in FALL 2027"),
        Some("Fall 2027".to_owned())
    );
    assert_eq!(term_in("classes in fall"), None);
    assert_eq!(
        term_in("classes in fall 26"),
        None,
        "a term carries a four digit year"
    );
    assert_eq!(term_in("CSE 310"), None);
}

#[test]
fn a_course_code_is_read_however_the_query_spaces_it() {
    assert_eq!(code_in("CSE 485 prerequisites"), Some("CSE 485".to_owned()));
    assert_eq!(
        code_in("what does cse310 cover"),
        Some("CSE 310".to_owned())
    );
    assert_eq!(
        code_in("prerequisites for MAT 243"),
        Some("MAT 243".to_owned())
    );
    assert_eq!(code_in("ENG 102L seats"), Some("ENG 102L".to_owned()));
    assert_eq!(code_in("machine learning courses"), None);
    assert_eq!(
        code_in("what 400 level classes are offered"),
        None,
        "a word a student writes is not a subject the catalog knows"
    );
    assert_eq!(code_in("any 300 seats left"), None);
    assert_eq!(
        code_in("classes in fall 2026"),
        None,
        "a four digit year is not a catalog number"
    );
}

#[test]
fn the_class_search_is_given_the_course_code_and_not_the_words_around_it() {
    let today = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap_or_default();
    let filled = params_for(&Courses, "does CSE 310 have open seats in Fall 2026", today);
    let Ok(params) = filled else {
        unreachable!("expected the query to fill, got {filled:?}")
    };
    assert_eq!(params["keywords"], json!("CSE 310"));
    assert_eq!(params["term"], json!("Fall 2026"));
    assert_eq!(
        params["open_only"],
        json!("true"),
        "a question about seats left asks the catalog for open sections"
    );
}

#[test]
fn a_course_question_with_no_code_keeps_the_words_the_catalog_can_match() {
    let today = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap_or_default();
    let filled = params_for(&Courses, "what machine learning classes are offered", today);
    assert_eq!(
        filled.map(|p| p["keywords"].clone()),
        Ok(json!("machine learning"))
    );
}

#[test]
fn the_catalog_answers_prerequisites_and_leaves_the_term_out_unless_asked() {
    let today = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap_or_default();
    let Ok(params) = params_for(
        &CourseCatalog,
        "what are the prerequisites for CSE 485",
        today,
    ) else {
        unreachable!("expected the query to fill")
    };
    assert_eq!(params["keywords"], json!("CSE 485"));
    assert_eq!(
        params.get("term"),
        None,
        "a catalog entry is read for no term unless the query names one"
    );
    let Ok(termed) = params_for(&CourseCatalog, "CSE 485 prerequisites Fall 2026", today) else {
        unreachable!("expected the query to fill")
    };
    assert_eq!(termed["term"], json!("Fall 2026"));
}

#[test]
fn dining_hours_are_refused_until_the_query_names_a_campus() {
    let today = NaiveDate::from_ymd_opt(2026, 9, 15).unwrap_or_default();
    let refused = match params_for(&Dining, "when does dining close tonight", today) {
        Err(message) => message,
        Ok(params) => unreachable!("expected a refusal, got {params:?}"),
    };
    assert!(refused.contains("dining needs campus"), "{refused}");
    assert!(refused.contains("tempe"), "{refused}");
    let filled = params_for(&Dining, "dining hours on the tempe campus", today);
    assert_eq!(filled.map(|p| p["campus"].clone()), Ok(json!("tempe")));
}

#[test]
fn every_source_the_engine_offers_carries_a_label_that_is_not_its_key() {
    for source in catalog() {
        let label = source.label();
        assert!(!label.is_empty(), "{} has no label", source.key());
        assert_ne!(
            label,
            source.key(),
            "a citation of {} would read as a registry key",
            source.key()
        );
    }
}

#[test]
fn the_local_date_is_the_one_the_offset_puts_the_user_in() {
    let midnight_utc = chrono::DateTime::parse_from_rfc3339("2026-09-15T02:00:00Z")
        .map(|t| t.with_timezone(&chrono::Utc))
        .unwrap_or_default();
    assert_eq!(
        Some(local_date(midnight_utc, -7)),
        NaiveDate::from_ymd_opt(2026, 9, 14),
        "two in the morning in UTC is still the day before in Arizona"
    );
    assert_eq!(
        Some(local_date(midnight_utc, 0)),
        NaiveDate::from_ymd_opt(2026, 9, 15)
    );
}

#[test]
fn a_tool_that_matches_what_the_platform_publishes_conforms() {
    for source in catalog() {
        let served = published(source.as_ref());
        assert_eq!(
            conforms(source.as_ref(), &served),
            Ok(()),
            "{}",
            source.key()
        );
    }
}

#[test]
fn a_tool_that_drifts_from_the_platform_is_named_at_boot() {
    let mut served = published(&Courses);
    served.params.retain(|p| p.name != "session");
    assert!(conforms(&Courses, &served).is_err_and(|e| e.contains("session")));

    let mut served = published(&Courses);
    served.params.push(QueryParam {
        name: "campus".into(),
        required: false,
        choices: Vec::new(),
        many: false,
    });
    assert!(conforms(&Courses, &served).is_err_and(|e| e.contains("campus")));

    let mut served = published(&Courses);
    if let Some(days) = served.params.iter_mut().find(|p| p.name == "days") {
        days.choices.retain(|c| c != "sunday");
    }
    assert!(conforms(&Courses, &served).is_err_and(|e| e.contains("sunday")));

    let mut served = published(&Courses);
    if let Some(term) = served.params.iter_mut().find(|p| p.name == "term") {
        term.required = false;
    }
    assert!(conforms(&Courses, &served).is_err_and(|e| e.contains("required")));

    let mut served = published(&Courses);
    if let Some(days) = served.params.iter_mut().find(|p| p.name == "days") {
        days.many = false;
    }
    assert!(conforms(&Courses, &served).is_err_and(|e| e.contains("list")));
}

#[tokio::test]
async fn a_live_call_queues_the_source_it_names_and_cites_the_page() {
    let queries = FakeQueries::new(vec![published(&Courses)]).answering("courses", "CSE 310 open");
    let sent = queries.sent();
    let tool = live_tool(queries);
    let out = tool
        .call(
            &ctx(),
            json!({"query": "CSE 310 open seats", "source": "courses"}),
        )
        .await;
    let Ok(output) = out else {
        unreachable!("the source answered, got {out:?}")
    };
    assert!(
        output.content.contains("CSE 310 open"),
        "{}",
        output.content
    );
    assert_eq!(
        output.sources[0].url.as_deref(),
        Some("https://example.test/courses")
    );
    let requests = sent.lock().map(|r| r.clone()).unwrap_or_default();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].source, "courses");
    assert_eq!(requests[0].params["keywords"], "CSE 310");
}

#[tokio::test]
async fn a_live_call_that_names_no_source_takes_the_fallback() {
    let queries = FakeQueries::new(vec![published(&Web)]).answering("web", "final score 24-17");
    let sent = queries.sent();
    let tool = live_tool(queries);
    let out = tool
        .call(&ctx(), json!({"query": "ASU football score"}))
        .await;
    assert!(out.is_ok(), "{out:?}");
    let requests = sent.lock().map(|r| r.clone()).unwrap_or_default();
    assert_eq!(requests[0].source, "web");
    assert_eq!(requests[0].params["query"], "ASU football score");
}

#[tokio::test]
async fn a_bad_argument_never_reaches_the_queue() {
    let queries = FakeQueries::new(vec![published(&Courses)]);
    let sent = queries.sent();
    let tool = live_tool(queries);
    let out = tool.call(&ctx(), json!({"source": "courses"})).await;
    assert!(
        matches!(out, Err(ToolError::InvalidArguments(_))),
        "{out:?}"
    );
    assert!(sent.lock().is_ok_and(|r| r.is_empty()));
}

#[tokio::test]
async fn a_platform_refusal_comes_back_as_something_the_model_can_fix() {
    let queries = FakeQueries::new(vec![published(&Courses)])
        .rejecting("courses", "courses returned a page with no readable text");
    let tool = live_tool(queries);
    match tool
        .call(&ctx(), json!({"query": "CSE 310", "source": "courses"}))
        .await
    {
        Err(ToolError::InvalidArguments(reason)) => assert!(reason.contains("no readable text")),
        other => unreachable!("expected a correctable refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn a_stored_call_narrows_to_the_category_of_the_source_it_names() {
    let index = Stored::holding("library_hours", "Hayden closes at 2am");
    let asked = index.asked();
    let tool = stored_tool(Arc::new(index));
    let out = tool
        .call(
            &ctx(),
            json!({"query": "Hayden Library hours Sunday", "source": "library_hours"}),
        )
        .await;
    let Ok(output) = out else {
        unreachable!("the index answered, got {out:?}")
    };
    assert!(
        output.content.contains("Hayden closes at 2am"),
        "{}",
        output.content
    );
    assert!(
        output.content.contains("[1] library_hours"),
        "{}",
        output.content
    );
    assert_eq!(output.sources.len(), 1);
    let queries = asked.lock().map(|q| q.clone()).unwrap_or_default();
    assert_eq!(queries[0].text, "Hayden Library hours Sunday");
    assert_eq!(
        queries[0].category.as_deref(),
        Some("library"),
        "the source names a key; the index is filtered by its category"
    );
}

#[tokio::test]
async fn a_source_that_holds_nothing_is_searched_past() {
    let index = Stored::holding("advising", "Submit the change of major request").unfiled();
    let asked = index.asked();
    let tool = stored_tool(Arc::new(index));
    let out = tool
        .call(
            &ctx(),
            json!({"query": "change major process", "source": "courses"}),
        )
        .await;
    let Ok(output) = out else {
        unreachable!("the index answered, got {out:?}")
    };
    assert!(
        output.content.contains("change of major"),
        "{}",
        output.content
    );
    let queries = asked.lock().map(|q| q.clone()).unwrap_or_default();
    let categories: Vec<Option<String>> = queries.iter().map(|q| q.category.clone()).collect();
    assert_eq!(categories.len(), 2, "the named source, then every source");
    assert!(
        categories[0].is_some() && categories[1].is_none(),
        "{categories:?}"
    );
}

#[tokio::test]
async fn a_stored_call_that_names_no_source_searches_every_category() {
    let index = Stored::holding("news", "robotics lab opens");
    let asked = index.asked();
    let tool = stored_tool(Arc::new(index));
    let out = tool
        .call(&ctx(), json!({"query": "ASU robotics lab"}))
        .await;
    assert!(out.is_ok(), "{out:?}");
    let queries = asked.lock().map(|q| q.clone()).unwrap_or_default();
    assert_eq!(queries[0].category, None);
}

#[tokio::test]
async fn an_empty_index_answers_with_what_to_do_next() {
    let tool = stored_tool(Arc::new(Stored::empty()));
    let out = tool.call(&ctx(), json!({"query": "moon landing"})).await;
    let Ok(output) = out else {
        unreachable!("an empty index is not a failure, got {out:?}")
    };
    assert_eq!(output.content, NOTHING_STORED);
    assert_eq!(output.sources.len(), 0);
    assert!(
        output.content.contains("search_live"),
        "it names the way on"
    );
}

#[test]
fn the_wording_the_model_reads_defaults_to_the_settings_in_the_file() {
    let tools = Tools::default();
    assert_eq!(tools.live_default_source, "web");
    assert_eq!(tools.query_description, QUERY_DESCRIPTION);
    let cost = estimate(&tools.query_description, 4);
    assert!(
        cost < 120,
        "the query wording stays short enough to read every step, got {cost} tokens"
    );
}

#[test]
fn an_old_page_with_a_url_is_marked_for_a_fresh_read() {
    let now = chrono::Utc::now();
    let page = |days: i64, url: Option<&str>| Evidence {
        source_id: uuid::Uuid::nil(),
        chunk_id: uuid::Uuid::nil(),
        key: "asu".into(),
        title: "Parking permits".into(),
        content: "Permits cost 780 dollars.".into(),
        url: url.map(str::to_owned),
        fetched_at: now - chrono::Duration::days(days),
        score: 1.0,
    };
    let stale = Stale {
        after: chrono::Duration::days(14),
        note: STALE_NOTE.into(),
    };
    let url = Some("https://cfo.asu.edu/parking");
    assert!(render(&[page(30, url)], now, &stale).contains(STALE_NOTE));
    assert!(!render(&[page(3, url)], now, &stale).contains(STALE_NOTE));
    assert!(
        !render(&[page(30, None)], now, &stale).contains(STALE_NOTE),
        "a page with no url cannot be read again"
    );
}
