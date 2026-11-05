//! search: the Platform search tool, stored pages first, then a live source, then the web.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::evidence::{Citation, age};
use crate::core::types::knowledge::query::{QueryError, SearchAnswer, SearchSpec, StoredPage};
use crate::core::types::tools::{RiskClass, SEARCH, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::structured;

/// The search tool, described and run by Platform.
pub struct Search {
    queries: Arc<dyn SourceQueries>,
    definition: ToolDefinition,
}

impl Search {
    /// Builds the tool from the description and schema Platform published.
    pub fn new(queries: Arc<dyn SourceQueries>, spec: SearchSpec, timeout_secs: u64) -> Self {
        let definition = ToolDefinition {
            name: SEARCH.to_owned(),
            description: spec.description,
            parameters: spec.parameters,
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: Some(timeout_secs),
        };
        Self {
            queries,
            definition,
        }
    }
}

/// The host of url, or url itself when it has none.
fn site(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_owned())
        })
        .unwrap_or_else(|| url.to_owned())
}

/// Stored passages as the model reads them, with the page url and fetch age on each.
pub fn render_stored(pages: &[StoredPage], note: Option<&str>, now: DateTime<Utc>) -> String {
    let mut out = String::new();
    for (i, page) in pages.iter().enumerate() {
        let link = page
            .url
            .as_deref()
            .map(|url| format!(" - {url}"))
            .unwrap_or_default();
        let fetched = page
            .fetched_at
            .map(|at| format!(" (stored copy, fetched {})", age(at, now)))
            .unwrap_or_default();
        let _ = write!(
            out,
            "[{}] {}{link}{fetched}\n{}\n\n",
            i + 1,
            page.title,
            page.content.trim()
        );
    }
    if let Some(note) = note.map(str::trim).filter(|n| !n.is_empty()) {
        out.push_str(note);
    }
    out.trim_end().to_owned()
}

/// One citation per page, best first. Passages of the same page collapse into one.
pub fn stored_citations(pages: &[StoredPage]) -> Vec<Citation> {
    let mut seen = HashSet::new();
    pages
        .iter()
        .filter(|p| seen.insert(p.url.clone().unwrap_or_else(|| p.title.clone())))
        .map(|p| Citation {
            key: p.url.clone().unwrap_or_else(|| p.title.clone()),
            title: p.title.clone(),
            url: p.url.clone(),
        })
        .collect()
}

/// What the model reads for an answer, and the sources the client shows.
pub fn output(answer: &SearchAnswer, now: DateTime<Utc>) -> ToolOutput {
    match answer {
        SearchAnswer::Stored { results, note } => ToolOutput {
            content: render_stored(results, note.as_deref(), now),
            data: structured(answer),
            sources: stored_citations(results),
        },
        SearchAnswer::Live { source, url, text } => ToolOutput {
            content: format!("Live result from {source} ({url}):\n\n{text}"),
            data: structured(answer),
            sources: vec![Citation {
                key: source.clone(),
                title: site(url),
                url: Some(url.clone()),
            }],
        },
        SearchAnswer::None { note } => ToolOutput {
            content: note.clone(),
            data: None,
            sources: Vec::new(),
        },
    }
}

#[async_trait]
impl Tool for Search {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let Value::Object(args) = args else {
            return Err(ToolError::InvalidArguments(
                "search takes an object with a query".into(),
            ));
        };
        let answer = self.queries.search(ctx, &args).await.map_err(|e| match e {
            QueryError::Rejected(reason) => ToolError::InvalidArguments(reason),
            QueryError::Cancelled => ToolError::Cancelled,
            QueryError::Timeout(_) => ToolError::Timeout,
            store @ QueryError::Store(_) => ToolError::Failed(store.to_string()),
        })?;
        Ok(output(&answer, Utc::now()))
    }
}
