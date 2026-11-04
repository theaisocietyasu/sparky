//! search_live: one fetch of an ASU source or the open web, run by the platform.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use serde_json::Value;

use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::evidence::{Citation, age};
use crate::core::types::knowledge::query::{QueryError, QueryRequest};
use crate::core::types::tools::{RiskClass, SEARCH_LIVE, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::knowledge::search::{
    LiveSource, arguments, parameters, params_for, source_help, source_keys,
};
use crate::runtime::tools::structured;

/// One live search over every source, narrowed by the source argument.
pub struct LiveSearch {
    sources: Vec<Box<dyn LiveSource>>,
    keys: Vec<&'static str>,
    fallback: String,
    queries: Arc<dyn SourceQueries>,
    utc_offset_hours: i32,
    definition: ToolDefinition,
}

/// The date at an offset from UTC, as the user reads it.
pub fn local_date(now: chrono::DateTime<Utc>, utc_offset_hours: i32) -> NaiveDate {
    (now + chrono::Duration::hours(i64::from(utc_offset_hours))).date_naive()
}

/// The wording of the live search tool, from the tools section.
#[derive(Debug, Clone)]
pub struct Wording {
    /// What the tool does, for the model.
    pub tool: String,
    /// What the query parameter must carry.
    pub query: String,
    /// Lead of the source filter, before the key and hint of each source.
    pub source: String,
}

impl LiveSearch {
    /// Builds the tool over sources, answering a call with no source from fallback.
    pub fn new(
        sources: Vec<Box<dyn LiveSource>>,
        queries: Arc<dyn SourceQueries>,
        fallback: &str,
        utc_offset_hours: i32,
        wording: &Wording,
        timeout_secs: u64,
    ) -> Self {
        let keys = source_keys(&sources, false);
        let definition = ToolDefinition {
            name: SEARCH_LIVE.to_owned(),
            description: wording.tool.clone(),
            parameters: parameters(
                &wording.query,
                &source_help(&wording.source, &sources, false),
                &keys,
            ),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: Some(timeout_secs),
        };
        Self {
            sources,
            keys,
            fallback: fallback.to_owned(),
            queries,
            utc_offset_hours,
            definition,
        }
    }

    /// Today where the user is, so a query that names no date takes the local one.
    fn today(&self) -> NaiveDate {
        local_date(Utc::now(), self.utc_offset_hours)
    }

    /// The source a call runs against: the one it named, else the fallback.
    fn pick(&self, named: Option<&str>) -> Option<&dyn LiveSource> {
        let wanted = named.unwrap_or(&self.fallback);
        self.sources
            .iter()
            .find(|s| s.key() == wanted)
            .map(std::convert::AsRef::as_ref)
    }
}

#[async_trait]
impl Tool for LiveSearch {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let (query, named) =
            arguments(args, &self.keys, SEARCH_LIVE).map_err(ToolError::InvalidArguments)?;
        let source = self.pick(named.as_deref()).ok_or_else(|| {
            ToolError::Failed(format!(
                "no live source named {:?} is registered",
                named.as_deref().unwrap_or(&self.fallback)
            ))
        })?;
        let params =
            params_for(source, &query, self.today()).map_err(ToolError::InvalidArguments)?;
        let request = QueryRequest {
            source: source.key().to_owned(),
            params,
        };
        let outcome = self.queries.run(ctx, &request).await.map_err(|e| match e {
            // A rejection reaches the model as an argument error.
            QueryError::Rejected(reason) => ToolError::InvalidArguments(reason),
            QueryError::Cancelled => ToolError::Cancelled,
            QueryError::Timeout(_) => ToolError::Timeout,
            store @ QueryError::Store(_) => ToolError::Failed(store.to_string()),
        })?;
        let when = outcome
            .fetched_at
            .map(|at| format!(", fetched {}", age(at, Utc::now())))
            .unwrap_or_default();
        Ok(ToolOutput {
            content: format!(
                "Live result from {} ({}{when}):\n\n{}",
                source.label(),
                outcome.url,
                outcome.text
            ),
            data: structured(&outcome),
            sources: vec![Citation {
                key: source.key().to_owned(),
                title: source.label().to_owned(),
                url: Some(outcome.url.clone()),
            }],
        })
    }
}
