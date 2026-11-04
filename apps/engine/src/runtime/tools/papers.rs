//! search_papers: academic paper search over the Semantic Scholar graph API. No key.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::core::config::Papers as PapersConfig;
use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::http;
use crate::runtime::tools::structured;

/// Name of the query parameter.
const QUERY: &str = "query";
/// Longest abstract kept, in characters.
const ABSTRACT_CHARS: usize = 320;

/// Searches Semantic Scholar for papers matching a query.
pub struct PapersTool {
    http: reqwest::Client,
    base_url: String,
    max_items: usize,
}

/// The search response.
#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default = "Vec::new")]
    data: Vec<RawPaper>,
}

/// One paper as the API returns it.
#[derive(Debug, Deserialize)]
struct RawPaper {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    authors: Vec<RawAuthor>,
    #[serde(default)]
    year: Option<i64>,
    #[serde(default, rename = "abstract")]
    abstract_: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

/// One author on a paper.
#[derive(Debug, Deserialize)]
struct RawAuthor {
    #[serde(default)]
    name: Option<String>,
}

/// The query a search call carries, trimmed. Err is shown to the model.
pub fn query_arg(args: &Value) -> Result<String, ToolError> {
    match args.get(QUERY) {
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(text.trim().to_owned()),
        _ => Err(ToolError::InvalidArguments(format!(
            "{QUERY} is required: the words to search for"
        ))),
    }
}

/// The JSON schema of a tool taking one required text query.
pub fn query_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "properties": { QUERY: { "type": "string", "description": description } },
        "required": [QUERY],
    })
}

#[async_trait]
impl Tool for PapersTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_papers".to_owned(),
            description: "Search academic papers on Semantic Scholar by topic, title, or author. \
                          Returns titles, authors, year, and a short abstract."
                .to_owned(),
            parameters: query_schema("What to search for, in keywords."),
            risk: RiskClass::ReadPublic,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }

    async fn call(&self, _ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let query = query_arg(&args)?;
        let url = reqwest::Url::parse_with_params(
            &format!("{}/paper/search", self.base_url),
            &[
                ("query", query.as_str()),
                ("limit", &self.max_items.to_string()),
                ("fields", "title,authors,year,abstract,url"),
            ],
        )
        .map_err(|e| ToolError::Failed(e.to_string()))?;
        let response = self.http.get(url).send().await.map_err(|e| {
            ToolError::Failed(format!(
                "semantic scholar unreachable: {}",
                http::failure(&e)
            ))
        })?;
        if !response.status().is_success() {
            return Err(ToolError::Failed(format!(
                "semantic scholar returned {}",
                response.status().as_u16()
            )));
        }
        let body = response.text().await.map_err(|e| {
            ToolError::Failed(format!(
                "semantic scholar unreachable: {}",
                http::failure(&e)
            ))
        })?;
        from_json(&query, &body, self.max_items).map_err(ToolError::Failed)
    }
}

/// Renders a search reply from a raw JSON body. Err when the body is not the expected shape.
pub(crate) fn from_json(query: &str, body: &str, max: usize) -> Result<ToolOutput, String> {
    let found: SearchResponse = serde_json::from_str(body)
        .map_err(|e| format!("semantic scholar sent an odd reply: {e}"))?;
    Ok(render(query, found.data, max))
}

/// One abstract held to ABSTRACT_CHARS.
fn clip(text: &str) -> String {
    if text.chars().count() > ABSTRACT_CHARS {
        text.chars().take(ABSTRACT_CHARS).collect::<String>() + "..."
    } else {
        text.to_owned()
    }
}

/// The reply for a search.
fn render(query: &str, papers: Vec<RawPaper>, max: usize) -> ToolOutput {
    let shown: Vec<RawPaper> = papers.into_iter().take(max).collect();
    let mut text = if shown.is_empty() {
        format!("No papers found for {query:?}.")
    } else {
        let mut lines = format!("{} papers for {query:?}:", shown.len());
        for p in &shown {
            let title = p.title.as_deref().unwrap_or("untitled");
            let year = p.year.map(|y| format!(" ({y})")).unwrap_or_default();
            let authors: Vec<&str> = p
                .authors
                .iter()
                .filter_map(|a| a.name.as_deref())
                .take(4)
                .collect();
            let by = if authors.is_empty() {
                String::new()
            } else {
                format!(" by {}", authors.join(", "))
            };
            let _ = write!(lines, "\n- {title}{year}{by}");
            if let Some(summary) = p.abstract_.as_deref().filter(|s| !s.trim().is_empty()) {
                let _ = write!(lines, "\n  {}", clip(summary));
            }
            if let Some(url) = p.url.as_deref() {
                let _ = write!(lines, "\n  {url}");
            }
        }
        lines
    };
    text.push('\n');
    ToolOutput {
        content: text.trim_end().to_owned(),
        data: structured(
            &shown
                .iter()
                .map(|p| p.title.clone().unwrap_or_default())
                .collect::<Vec<_>>(),
        ),
        sources: Vec::new(),
    }
}

/// The paper search tool, when it is enabled.
pub fn tools(cfg: &PapersConfig) -> Result<Vec<Arc<dyn Tool>>, String> {
    let http = http::client(Duration::from_secs(cfg.timeout_secs)).map_err(|e| e.to_string())?;
    let tool: Arc<dyn Tool> = Arc::new(PapersTool {
        http,
        base_url: cfg.base_url.trim_end_matches('/').to_owned(),
        max_items: cfg.max_items,
    });
    Ok(vec![tool])
}
