//! search_knowledge: the stored index, optionally narrowed to the pages of one source.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;

use crate::core::traits::knowledge::retrieval::Retriever;
use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::knowledge::evidence::{Evidence, age};
use crate::core::types::knowledge::retrieval::{RetrievalError, RetrievalQuery};
use crate::core::types::tools::{
    RiskClass, SEARCH_KNOWLEDGE, ToolDefinition, ToolError, ToolOutput,
};
use crate::runtime::tools::knowledge::search::{
    LiveSource, arguments, parameters, source_help, source_keys,
};
use crate::runtime::tools::structured;

/// A search of the stored index, narrowed by the source argument.
pub struct StoredSearch {
    retriever: Arc<dyn Retriever>,
    keys: Vec<&'static str>,
    categories: HashMap<&'static str, &'static str>,
    top_k: usize,
    empty: String,
    stale: Stale,
    definition: ToolDefinition,
}

/// The wording of the stored search tool, from the tools section.
#[derive(Debug, Clone)]
pub struct Wording {
    /// What the tool does, for the model.
    pub tool: String,
    /// What the query parameter must carry.
    pub query: String,
    /// Lead of the source filter, before the key and hint of each source.
    pub source: String,
    /// What the tool answers when the index holds nothing for the query.
    pub empty: String,
    /// Note on a passage whose page was fetched longer ago than stale_days.
    pub stale: String,
}

/// When a stored passage counts as old, and the note it carries then.
#[derive(Debug, Clone)]
pub struct Stale {
    /// Age past which a passage is marked.
    pub after: chrono::Duration,
    /// Note added after the fetch age of a marked passage that has a url.
    pub note: String,
}

impl StoredSearch {
    /// Builds the tool over the sources the platform indexes.
    pub fn new(
        sources: &[Box<dyn LiveSource>],
        retriever: Arc<dyn Retriever>,
        top_k: usize,
        stale_days: u32,
        wording: &Wording,
    ) -> Self {
        let keys = source_keys(sources, true);
        let categories = sources
            .iter()
            .filter(|s| s.freshness().indexed())
            .map(|s| (s.key(), s.category()))
            .collect();
        let definition = ToolDefinition {
            name: SEARCH_KNOWLEDGE.to_owned(),
            description: wording.tool.clone(),
            parameters: parameters(
                &wording.query,
                &source_help(&wording.source, sources, true),
                &keys,
            ),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        };
        Self {
            retriever,
            keys,
            categories,
            top_k,
            empty: wording.empty.clone(),
            stale: Stale {
                after: chrono::Duration::days(i64::from(stale_days)),
                note: wording.stale.clone(),
            },
            definition,
        }
    }
}

/// Retrieved passages as the model reads them, fetch age and page url on each, old pages marked.
pub fn render(evidence: &[Evidence], now: chrono::DateTime<Utc>, stale: &Stale) -> String {
    let mut out = String::new();
    for (i, e) in evidence.iter().enumerate() {
        let page = e
            .url
            .as_deref()
            .map(|url| format!(" - {url}"))
            .unwrap_or_default();
        let note = if e.url.is_some() && now - e.fetched_at > stale.after {
            format!(" {}", stale.note)
        } else {
            String::new()
        };
        let _ = write!(
            out,
            "[{}] {}{page} (stored copy, fetched {}){note}\n{}\n\n",
            i + 1,
            e.title,
            age(e.fetched_at, now),
            e.content.trim()
        );
    }
    out.trim_end().to_owned()
}

impl StoredSearch {
    /// One retrieval, its failures as tool errors.
    async fn search(
        &self,
        ctx: &RequestContext,
        request: &RetrievalQuery,
    ) -> Result<Vec<Evidence>, ToolError> {
        self.retriever
            .retrieve(ctx, request)
            .await
            .map_err(|e| match e {
                store @ RetrievalError::Store(_) => ToolError::Failed(store.to_string()),
                embedding @ RetrievalError::Embedding(_) => {
                    ToolError::Failed(embedding.to_string())
                }
            })
    }
}

#[async_trait]
impl Tool for StoredSearch {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let (query, named) =
            arguments(args, &self.keys, SEARCH_KNOWLEDGE).map_err(ToolError::InvalidArguments)?;
        let broad = RetrievalQuery::new(query, self.top_k);
        let category = named
            .as_deref()
            .and_then(|key| self.categories.get(key).copied());
        let mut evidence = match category {
            Some(category) => {
                self.search(ctx, &broad.clone().in_category(category))
                    .await?
            }
            None => Vec::new(),
        };
        // A source that holds nothing for the query is searched past, across every source.
        if evidence.is_empty() {
            evidence = self.search(ctx, &broad).await?;
        }
        if evidence.is_empty() {
            return Ok(ToolOutput {
                content: self.empty.clone(),
                data: None,
                sources: Vec::new(),
            });
        }
        Ok(ToolOutput {
            content: render(&evidence, Utc::now(), &self.stale),
            data: structured(&evidence),
            sources: Evidence::citations(&evidence),
        })
    }
}
