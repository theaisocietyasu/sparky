//! Knowledge double: a fixed source registry and a canned search answer.

use async_trait::async_trait;
use serde_json::{Map, Value, json};

use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::query::{QueryError, QuerySourceInfo, SearchAnswer, SearchSpec};

/// A SourceQueries double: a fixed registry, a search schema over its keys, and one answer.
pub struct FakeQueries {
    sources: Vec<QuerySourceInfo>,
    answer: Result<SearchAnswer, String>,
    sent: std::sync::Arc<std::sync::Mutex<Vec<Map<String, Value>>>>,
}

impl FakeQueries {
    /// Offers sources and finds nothing for any search.
    pub fn new(sources: Vec<QuerySourceInfo>) -> Self {
        Self {
            sources,
            answer: Ok(SearchAnswer::None {
                note: "Nothing came back.".into(),
            }),
            sent: std::sync::Arc::default(),
        }
    }

    /// The arguments this double is sent, readable after it moves into a tool.
    pub fn sent(&self) -> std::sync::Arc<std::sync::Mutex<Vec<Map<String, Value>>>> {
        std::sync::Arc::clone(&self.sent)
    }

    /// Answers every search with answer.
    pub fn answering(self, answer: SearchAnswer) -> Self {
        Self {
            answer: Ok(answer),
            ..self
        }
    }

    /// Refuses every search with this reason.
    pub fn rejecting(self, reason: &str) -> Self {
        Self {
            answer: Err(reason.to_owned()),
            ..self
        }
    }
}

#[async_trait]
impl SourceQueries for FakeQueries {
    async fn sources(&self) -> Result<Vec<QuerySourceInfo>, QueryError> {
        Ok(self.sources.clone())
    }

    async fn spec(&self) -> Result<SearchSpec, QueryError> {
        let keys: Vec<&str> = self.sources.iter().map(|s| s.key.as_str()).collect();
        Ok(SearchSpec {
            description: "Search stored pages, then a live source, then the web.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "source": {"enum": keys},
                },
                "required": ["query"],
            }),
        })
    }

    async fn search(
        &self,
        _ctx: &RequestContext,
        args: &Map<String, Value>,
    ) -> Result<SearchAnswer, QueryError> {
        if let Ok(mut sent) = self.sent.lock() {
            sent.push(args.clone());
        }
        self.answer.clone().map_err(QueryError::Rejected)
    }
}
