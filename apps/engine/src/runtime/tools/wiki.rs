//! wikipedia_lookup: the intro of the best-matching Wikipedia article. No key.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use crate::core::config::Wikipedia as WikipediaConfig;
use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::http;
use crate::runtime::tools::papers::{query_arg, query_schema};
use crate::runtime::tools::structured;

/// Looks up the introduction of a Wikipedia article.
pub struct WikiTool {
    http: reqwest::Client,
    base_url: String,
    max_chars: usize,
}

/// The MediaWiki query response.
#[derive(Debug, Deserialize)]
struct WikiResponse {
    #[serde(default)]
    query: Option<WikiQuery>,
}

/// The pages a generator returned, keyed by page id.
#[derive(Debug, Deserialize)]
struct WikiQuery {
    #[serde(default)]
    pages: HashMap<String, WikiPage>,
}

/// One article page.
#[derive(Debug, Deserialize, Clone)]
struct WikiPage {
    #[serde(default)]
    index: i64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    extract: Option<String>,
}

#[async_trait]
impl Tool for WikiTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "wikipedia_lookup".to_owned(),
            description: "Look up the introduction of the best-matching English Wikipedia article \
                          for a topic."
                .to_owned(),
            parameters: query_schema("The subject to look up."),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }

    async fn call(&self, _ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let query = query_arg(&args)?;
        let url = reqwest::Url::parse_with_params(
            &self.base_url,
            &[
                ("action", "query"),
                ("format", "json"),
                ("prop", "extracts"),
                ("exintro", "1"),
                ("explaintext", "1"),
                ("redirects", "1"),
                ("generator", "search"),
                ("gsrsearch", query.as_str()),
                ("gsrlimit", "1"),
            ],
        )
        .map_err(|e| ToolError::Failed(e.to_string()))?;
        let response = self
            .http
            .get(url)
            .header(
                reqwest::header::USER_AGENT,
                "SparkyAI/2.0 (ASU student copilot)",
            )
            .send()
            .await
            .map_err(|e| {
                ToolError::Failed(format!("wikipedia unreachable: {}", http::failure(&e)))
            })?;
        if !response.status().is_success() {
            return Err(ToolError::Failed(format!(
                "wikipedia returned {}",
                response.status().as_u16()
            )));
        }
        let body = response.text().await.map_err(|e| {
            ToolError::Failed(format!("wikipedia unreachable: {}", http::failure(&e)))
        })?;
        from_json(&query, &body, self.max_chars).map_err(ToolError::Failed)
    }
}

/// Renders a lookup reply from a raw JSON body. Err when the body is not the expected shape.
pub(crate) fn from_json(query: &str, body: &str, max_chars: usize) -> Result<ToolOutput, String> {
    let found: WikiResponse =
        serde_json::from_str(body).map_err(|e| format!("wikipedia sent an odd reply: {e}"))?;
    Ok(render(query, found, max_chars))
}

/// The reply for a lookup: the top page, or a note that none matched.
fn render(query: &str, found: WikiResponse, max_chars: usize) -> ToolOutput {
    let best = found.query.and_then(|q| {
        q.pages
            .into_values()
            .min_by_key(|p| p.index)
            .filter(|p| p.extract.as_deref().is_some_and(|e| !e.trim().is_empty()))
    });
    let Some(page) = best else {
        return ToolOutput {
            content: format!("No Wikipedia article found for {query:?}."),
            data: None,
            sources: Vec::new(),
        };
    };
    let extract = page.extract.clone().unwrap_or_default();
    let body = if extract.chars().count() > max_chars {
        extract.chars().take(max_chars).collect::<String>() + "..."
    } else {
        extract
    };
    ToolOutput {
        content: format!("{}\n\n{}", page.title, body),
        data: structured(&page.title),
        sources: Vec::new(),
    }
}

/// The Wikipedia lookup tool, when it is enabled.
pub fn tools(cfg: &WikipediaConfig) -> Result<Vec<Arc<dyn Tool>>, String> {
    let http = http::client(Duration::from_secs(cfg.timeout_secs)).map_err(|e| e.to_string())?;
    let tool: Arc<dyn Tool> = Arc::new(WikiTool {
        http,
        base_url: cfg.base_url.clone(),
        max_chars: cfg.max_chars,
    });
    Ok(vec![tool])
}
