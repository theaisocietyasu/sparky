//! Profile extraction: what the classifier gates, the graph agent produces, unreadable answers.

use std::sync::Arc;

use crate::core::tests::support::{Scripted, ctx, text};
use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::profile::ProfileError;
use crate::core::types::model::{ModelError, ModelResponse};
use crate::runtime::harness::agent::task::{Task, TaskConfig};
use crate::runtime::harness::memory::profile::{GRAPH_INSTRUCTIONS, GraphAgent};

fn graph_agent(replies: Vec<Result<ModelResponse, ModelError>>) -> GraphAgent {
    GraphAgent::new(Task::new(
        Arc::new(Scripted::new(replies)),
        "profile_graph",
        GRAPH_INSTRUCTIONS,
        TaskConfig::default(),
    ))
}

#[tokio::test]
async fn the_graph_agent_turns_a_turn_into_facts() {
    let g = graph_agent(vec![Ok(text(
        r#"{"facts":[
             {"subject":{"kind":"person","label":"the user"},
              "relation":"studies",
              "object":{"kind":"course","label":"CSE 310"},
              "confidence":0.9},
             {"subject":{"kind":"person","label":"the user"},
              "relation":"belongs_to",
              "object":{"kind":"club","label":"AI Society"}}
           ]}"#,
    ))]);
    let Ok(facts) = g
        .extract(&ctx(), "i'm taking CSE 310 and i'm in the AI Society")
        .await
    else {
        unreachable!("the answer parses")
    };
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].subject.label, "the user");
    assert_eq!(facts[0].relation, "studies");
    assert_eq!(facts[0].object.kind, "course");
    assert_eq!(facts[0].object.label, "CSE 310");
    assert!(facts[0].confidence < 1.0);
    // A fact with no confidence is stored as certain.
    assert!(facts[1].confidence > 0.99);
}

#[tokio::test]
async fn an_answer_around_the_json_still_parses() {
    let g = graph_agent(vec![Ok(text(
        "```json\n{\"facts\":[{\"subject\":{\"kind\":\"person\",\"label\":\"the user\"},\
         \"relation\":\"lives_in\",\"object\":{\"kind\":\"place\",\"label\":\"Tempe\"}}]}\n```",
    ))]);
    let Ok(facts) = g.extract(&ctx(), "i live in tempe").await else {
        unreachable!("the fenced answer parses")
    };
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].object.label, "Tempe");
}

#[tokio::test]
async fn an_unparseable_extraction_is_an_error_rather_than_no_facts() {
    let g = graph_agent(vec![Ok(text(
        "Sure, I found a couple of things about them.",
    ))]);
    // An unreadable answer is Malformed, not an empty extraction.
    let Err(ProfileError::Malformed(_)) = g.extract(&ctx(), "i'm taking CSE 310").await else {
        unreachable!("an unreadable answer is malformed")
    };
}

#[tokio::test]
async fn json_that_is_not_the_extraction_shape_is_an_error() {
    let g = graph_agent(vec![Ok(text(r#"{"answer":"they study CSE 310"}"#))]);
    let Err(ProfileError::Malformed(_)) = g.extract(&ctx(), "i'm taking CSE 310").await else {
        unreachable!("the wrong shape is malformed")
    };
}

#[tokio::test]
async fn a_failed_model_call_is_reported_as_a_model_error() {
    let g = graph_agent(Vec::new());
    let Err(ProfileError::Model(_)) = g.extract(&ctx(), "i'm taking CSE 310").await else {
        unreachable!("the script is exhausted")
    };
}

#[test]
fn a_relation_reads_as_a_sentence_in_the_prompt() {
    use crate::core::types::memory::profile::{ProfileEntity, ProfileRelation};
    use crate::core::types::memory::{Memory, MemoryKind};

    let relation = ProfileRelation {
        subject: ProfileEntity {
            kind: "person".into(),
            label: "the student".into(),
        },
        relation: "studies".into(),
        object: ProfileEntity {
            kind: "subject".into(),
            label: "computer science".into(),
        },
        confidence: 0.9,
    };
    assert_eq!(relation.to_string(), "the student studies computer science");

    // A node says what a user is connected to. Only the relation says how.
    let memory = Memory::from(&relation);
    assert_eq!(memory.kind, MemoryKind::Fact);
    assert_eq!(memory.content, "the student studies computer science");
}

#[test]
fn a_recalled_node_reads_as_its_kind_and_label() {
    use crate::core::types::memory::profile::ProfileNode;
    use crate::core::types::memory::{Memory, MemoryKind};

    let node = ProfileNode {
        id: uuid::Uuid::new_v4(),
        kind: "interest".into(),
        label: "robotics".into(),
        confidence: 1.0,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let memory = Memory::from(&node);
    assert_eq!(memory.kind, MemoryKind::Fact);
    assert_eq!(memory.content, "interest: robotics");
}

/// Records which forget the route asked for.
#[derive(Default)]
struct Forgetful {
    calls: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl crate::core::traits::memory::profile::ProfileGraph for Forgetful {
    async fn upsert(
        &self,
        _ctx: &RequestContext,
        _facts: &[crate::core::types::memory::profile::ProfileFact],
    ) -> Result<(), crate::core::types::memory::profile::ProfileError> {
        Ok(())
    }

    async fn recall(
        &self,
        _ctx: &RequestContext,
        _limit: usize,
    ) -> Result<Vec<crate::core::types::memory::profile::ProfileNode>, ProfileError> {
        Ok(Vec::new())
    }

    async fn relations(
        &self,
        _ctx: &RequestContext,
        _limit: usize,
    ) -> Result<Vec<crate::core::types::memory::profile::ProfileRelation>, ProfileError> {
        Ok(Vec::new())
    }

    async fn matching(
        &self,
        _ctx: &RequestContext,
        _subject: &str,
        _relation: &str,
    ) -> Result<Vec<crate::core::types::memory::profile::ProfileRelation>, ProfileError> {
        Ok(Vec::new())
    }

    async fn drop_relation(
        &self,
        _ctx: &RequestContext,
        _relation: &crate::core::types::memory::profile::ProfileRelation,
    ) -> Result<bool, ProfileError> {
        Ok(false)
    }

    async fn forget(&self, ctx: &RequestContext, label: &str) -> Result<u64, ProfileError> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(format!("forget {} for {}", label, ctx.user_id));
        }
        Ok(1)
    }

    async fn forget_all(&self, ctx: &RequestContext) -> Result<u64, ProfileError> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(format!("forget_all for {}", ctx.user_id));
        }
        Ok(3)
    }
}

#[tokio::test]
async fn a_label_removes_one_thing_and_no_label_removes_everything() {
    use axum::extract::State;
    use axum::http::HeaderMap;
    use secrecy::SecretString;

    use crate::core::types::memory::profile::ForgetRequest;
    use crate::routes::profile::{ProfileState, forget};

    let graph = std::sync::Arc::new(Forgetful::default());
    let state = ProfileState {
        graph: Some(graph.clone()),
        list_limit: 50,
        request_budget: std::time::Duration::from_secs(30),
        rate_limit: crate::routes::rate_limit::RateLimiter::new(0),
        default_tenant: "g".into(),
        service_token: SecretString::from("t"),
    };
    let mut headers = HeaderMap::new();
    let Ok(value) = "Bearer t".parse() else {
        unreachable!("a header value")
    };
    headers.insert("authorization", value);

    forget(
        State(state.clone()),
        headers.clone(),
        axum::Json(ForgetRequest {
            user_id: "u".into(),
            tenant_id: None,
            label: Some("robotics".into()),
        }),
    )
    .await;
    forget(
        State(state),
        headers,
        axum::Json(ForgetRequest {
            user_id: "u".into(),
            tenant_id: None,
            label: None,
        }),
    )
    .await;

    let calls = graph.calls.lock().map(|c| c.clone()).unwrap_or_default();
    assert_eq!(calls, vec!["forget robotics for u", "forget_all for u"]);
}

#[tokio::test]
async fn forgetting_without_the_token_is_refused() {
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use secrecy::SecretString;

    use crate::core::types::memory::profile::ForgetRequest;
    use crate::routes::profile::{ProfileState, forget};

    let graph = std::sync::Arc::new(Forgetful::default());
    let state = ProfileState {
        graph: Some(graph.clone()),
        list_limit: 50,
        request_budget: std::time::Duration::from_secs(30),
        rate_limit: crate::routes::rate_limit::RateLimiter::new(0),
        default_tenant: "g".into(),
        service_token: SecretString::from("t"),
    };
    let response = forget(
        State(state),
        HeaderMap::new(),
        axum::Json(ForgetRequest {
            user_id: "u".into(),
            tenant_id: None,
            label: None,
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        graph.calls.lock().is_ok_and(|c| c.is_empty()),
        "nothing was deleted"
    );
}

#[test]
fn a_reconciler_answer_names_statements_to_withdraw() {
    use crate::runtime::harness::memory::profile::parse_indices;

    assert_eq!(parse_indices("2", 3), vec![1]);
    assert_eq!(parse_indices("1, 3", 3), vec![0, 2]);
    assert_eq!(
        parse_indices("1,1,2", 3),
        vec![0, 1],
        "a repeat is one removal"
    );
}

#[test]
fn anything_unreadable_withdraws_nothing() {
    use crate::runtime::harness::memory::profile::parse_indices;

    // An unreadable answer withdraws nothing.
    for answer in [
        "none",
        "None.",
        "I think none of them apply",
        "",
        "   ",
        "banana",
    ] {
        assert!(
            parse_indices(answer, 3).is_empty(),
            "{answer:?} withdrew something"
        );
    }
    // An index outside the list is dropped.
    assert_eq!(parse_indices("9", 3).len(), 0);
    assert_eq!(parse_indices("0, 2", 3), vec![1], "numbering starts at one");
}

/// A graph that reports what is recorded and remembers what was withdrawn.
#[derive(Default)]
struct Recorded {
    existing: Vec<crate::core::types::memory::profile::ProfileRelation>,
    dropped: std::sync::Mutex<Vec<String>>,
    written: std::sync::Mutex<Vec<crate::core::types::memory::profile::ProfileFact>>,
}

#[async_trait::async_trait]
impl crate::core::traits::memory::profile::ProfileGraph for Recorded {
    async fn upsert(
        &self,
        _ctx: &RequestContext,
        facts: &[crate::core::types::memory::profile::ProfileFact],
    ) -> Result<(), ProfileError> {
        if let Ok(mut written) = self.written.lock() {
            written.extend_from_slice(facts);
        }
        Ok(())
    }

    async fn recall(
        &self,
        _ctx: &RequestContext,
        _limit: usize,
    ) -> Result<Vec<crate::core::types::memory::profile::ProfileNode>, ProfileError> {
        Ok(Vec::new())
    }

    async fn relations(
        &self,
        _ctx: &RequestContext,
        _limit: usize,
    ) -> Result<Vec<crate::core::types::memory::profile::ProfileRelation>, ProfileError> {
        Ok(self.existing.clone())
    }

    async fn matching(
        &self,
        _ctx: &RequestContext,
        subject: &str,
        relation: &str,
    ) -> Result<Vec<crate::core::types::memory::profile::ProfileRelation>, ProfileError> {
        Ok(self
            .existing
            .iter()
            .filter(|e| {
                e.subject.label.eq_ignore_ascii_case(subject)
                    && e.relation.eq_ignore_ascii_case(relation)
            })
            .cloned()
            .collect())
    }

    async fn drop_relation(
        &self,
        _ctx: &RequestContext,
        relation: &crate::core::types::memory::profile::ProfileRelation,
    ) -> Result<bool, ProfileError> {
        if let Ok(mut dropped) = self.dropped.lock() {
            dropped.push(relation.to_string());
        }
        Ok(true)
    }

    async fn forget(&self, _ctx: &RequestContext, _label: &str) -> Result<u64, ProfileError> {
        Ok(0)
    }

    async fn forget_all(&self, _ctx: &RequestContext) -> Result<u64, ProfileError> {
        Ok(0)
    }
}

fn relation(
    subject: &str,
    rel: &str,
    object: &str,
) -> crate::core::types::memory::profile::ProfileRelation {
    use crate::core::types::memory::profile::{ProfileEntity, ProfileRelation};
    ProfileRelation {
        subject: ProfileEntity {
            kind: "person".into(),
            label: subject.into(),
        },
        relation: rel.into(),
        object: ProfileEntity {
            kind: "thing".into(),
            label: object.into(),
        },
        confidence: 1.0,
    }
}

/// Runs one turn through the writer and reports what the graph saw.
async fn record(
    existing: Vec<crate::core::types::memory::profile::ProfileRelation>,
    extraction: &str,
    verdict: &str,
    turn: &str,
) -> (Vec<String>, usize) {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::core::tests::support::{Scripted, text};
    use crate::runtime::harness::agent::task::{Task, TaskConfig};
    use crate::runtime::harness::memory::detect::RuleDetector;
    use crate::runtime::harness::memory::profile::{GraphAgent, ProfileWriter, Reconciler};

    let graph = Arc::new(Recorded {
        existing,
        ..Recorded::default()
    });
    let agent = GraphAgent::new(Task::new(
        Arc::new(Scripted::new(vec![Ok(text(extraction))])),
        "extract",
        "extract",
        TaskConfig::default(),
    ));
    let reconciler = Reconciler::new(Task::new(
        Arc::new(Scripted::new(vec![Ok(text(verdict))])),
        "reconcile",
        "reconcile",
        TaskConfig::default(),
    ));
    let writer = ProfileWriter::new(
        Arc::new(RuleDetector::default()),
        agent,
        Some(reconciler),
        graph.clone(),
        Duration::from_secs(5),
        0.5,
    );
    writer.record("g".into(), "u".into(), turn.to_owned()).await;
    let dropped = graph.dropped.lock().map(|d| d.clone()).unwrap_or_default();
    let written = graph.written.lock().map_or(0, |w| w.len());
    (dropped, written)
}

#[test]
fn a_fact_with_an_empty_label_or_low_confidence_is_not_keepable() {
    use crate::core::types::memory::profile::{ProfileEntity, ProfileFact};
    use crate::runtime::harness::memory::profile::keepable;

    let fact = |object: &str, confidence: f32| ProfileFact {
        subject: ProfileEntity {
            kind: "person".into(),
            label: "the user".into(),
        },
        relation: "studies".into(),
        object: ProfileEntity {
            kind: "topic".into(),
            label: object.into(),
        },
        confidence,
    };
    assert!(keepable(&fact("physics", 0.9), 0.5));
    assert!(!keepable(&fact("", 0.9), 0.5));
    assert!(!keepable(&fact("  ", 0.9), 0.5));
    assert!(!keepable(&fact("physics", 0.2), 0.5));
}

#[tokio::test]
async fn an_extraction_that_echoes_the_empty_template_writes_nothing() {
    let (_, written) = record(
        Vec::new(),
        r#"{"facts":[{"subject":{"kind":"person","label":"the user"},"relation":"studies",
            "object":{"kind":"","label":""},"confidence":0.0}]}"#,
        "none",
        "I study here every day",
    )
    .await;
    assert_eq!(written, 0);
}

#[tokio::test]
async fn a_changed_major_withdraws_the_old_one() {
    // The reconciler withdraws the old major when a new one is stated.
    let (dropped, written) = record(
        vec![relation("the user", "studies", "computer science")],
        r#"{"facts":[{"subject":{"kind":"person","label":"the user"},"relation":"studies",
            "object":{"kind":"subject","label":"physics"},"confidence":0.9}]}"#,
        "1",
        "I switched my major to physics this semester",
    )
    .await;
    assert_eq!(dropped, vec!["the user studies computer science"]);
    assert_eq!(written, 1, "the new fact is still written");
}

#[tokio::test]
async fn two_statements_that_can_both_be_true_are_both_kept() {
    // The reconciler answers none and both preferences stay.
    let (dropped, written) = record(
        vec![relation("the user", "prefers", "mornings for lectures")],
        r#"{"facts":[{"subject":{"kind":"person","label":"the user"},"relation":"prefers",
            "object":{"kind":"time","label":"evenings for study"},"confidence":0.9}]}"#,
        "none",
        "I prefer studying in the evenings",
    )
    .await;
    assert!(dropped.is_empty(), "nothing was withdrawn: {dropped:?}");
    assert_eq!(written, 1);
}

#[tokio::test]
async fn repeating_a_fact_withdraws_nothing() {
    let (dropped, _) = record(
        vec![relation("the user", "studies", "physics")],
        r#"{"facts":[{"subject":{"kind":"person","label":"the user"},"relation":"studies",
            "object":{"kind":"subject","label":"physics"},"confidence":0.9}]}"#,
        "1",
        "I am studying physics this year",
    )
    .await;
    // The repeated statement is filtered before the reconciler sees it.
    assert!(dropped.is_empty(), "{dropped:?}");
}

#[tokio::test]
async fn a_list_shows_nodes_and_relations_by_label() {
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use secrecy::SecretString;

    use crate::core::tests::support::Known;
    use crate::core::types::memory::profile::ListRequest;
    use crate::routes::profile::{ProfileState, list};

    let mut headers = HeaderMap::new();
    let Ok(value) = "Bearer t".parse() else {
        unreachable!("a header value")
    };
    headers.insert("authorization", value);
    let request = || {
        axum::Json(ListRequest {
            user_id: "u".into(),
            tenant_id: None,
        })
    };
    let state = ProfileState {
        graph: Some(Arc::new(Known::default())),
        list_limit: 50,
        request_budget: std::time::Duration::from_secs(30),
        rate_limit: crate::routes::rate_limit::RateLimiter::new(0),
        default_tenant: "g".into(),
        service_token: SecretString::from("t"),
    };

    let response = list(State(state.clone()), headers.clone(), request()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let Ok(bytes) = axum::body::to_bytes(response.into_body(), usize::MAX).await else {
        unreachable!("a readable body")
    };
    let Ok(listed) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        unreachable!("a json body")
    };
    assert_eq!(listed["nodes"][0]["kind"], "course");
    assert_eq!(listed["nodes"][0]["label"], "CSE 310");
    assert!(listed["nodes"][0]["confidence"].is_number());
    assert_eq!(listed["relations"][0]["subject"], "the user");
    assert_eq!(listed["relations"][0]["relation"], "studies");
    assert_eq!(listed["relations"][0]["object"], "CSE 310");
    assert!(listed["relations"][0]["confidence"].is_number());

    let disabled = ProfileState {
        graph: None,
        ..state
    };
    let response = list(State(disabled), headers, request()).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn listing_past_the_rate_limit_is_refused() {
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use secrecy::SecretString;

    use crate::core::tests::support::Known;
    use crate::core::types::memory::profile::ListRequest;
    use crate::routes::profile::{ProfileState, list};

    let mut headers = HeaderMap::new();
    let Ok(value) = "Bearer t".parse() else {
        unreachable!("a header value")
    };
    headers.insert("authorization", value);
    let state = ProfileState {
        graph: Some(Arc::new(Known::default())),
        list_limit: 50,
        request_budget: std::time::Duration::from_secs(30),
        rate_limit: crate::routes::rate_limit::RateLimiter::new(1),
        default_tenant: "g".into(),
        service_token: SecretString::from("t"),
    };
    let request = || {
        axum::Json(ListRequest {
            user_id: "u".into(),
            tenant_id: None,
        })
    };
    let first = list(State(state.clone()), headers.clone(), request()).await;
    assert_eq!(first.status(), StatusCode::OK);
    let second = list(State(state), headers, request()).await;
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
}
