//! QuerySourceInfo, SearchSpec, SearchAnswer: the sources Platform publishes and its search tool.

use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

/// One parameter a query source accepts, as the platform published it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryParam {
    /// Parameter name.
    pub name: String,
    /// Whether the query fails without it.
    #[serde(default)]
    pub required: bool,
    /// The only values the platform accepts, compared without case. Empty accepts any text.
    #[serde(default)]
    pub choices: Vec<String>,
    /// Whether a comma-separated list of choices is accepted.
    #[serde(default)]
    pub many: bool,
}

/// A query source the platform serves, read from the registry it publishes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuerySourceInfo {
    /// Registry key.
    pub key: String,
    /// What it answers.
    #[serde(default)]
    pub description: String,
    /// Parameters it accepts.
    pub params: Vec<QueryParam>,
}

/// The search tool as Platform describes it: what the model reads and the arguments it takes.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SearchSpec {
    /// What the tool does, for the model.
    pub description: String,
    /// JSON Schema of the arguments.
    #[serde(rename = "input_schema")]
    pub parameters: serde_json::Value,
}

/// One stored passage a search returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPage {
    /// Page title.
    pub title: String,
    /// Page url, when the page has one.
    #[serde(default)]
    pub url: Option<String>,
    /// When Platform fetched the page.
    #[serde(default, deserialize_with = "utc_or_naive")]
    pub fetched_at: Option<DateTime<Utc>>,
    /// Passage text.
    pub content: String,
}

/// An ISO 8601 time, read as UTC when it carries no offset. Text that is not a time reads as none.
fn utc_or_naive<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
    let text: Option<String> = Option::deserialize(d)?;
    Ok(text.as_deref().and_then(|t| {
        DateTime::parse_from_rfc3339(t)
            .map(|at| at.with_timezone(&Utc))
            .ok()
            .or_else(|| {
                NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.f")
                    .ok()
                    .map(|at| at.and_utc())
            })
    }))
}

/// What a search answered with, as the from field names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum SearchAnswer {
    /// Passages from pages Platform keeps.
    Stored {
        /// Passages, best first.
        results: Vec<StoredPage>,
        /// Note for the model, set when some pages are old.
        #[serde(default)]
        note: Option<String>,
    },
    /// One page a live source fetched now.
    Live {
        /// Registry key of the source.
        source: String,
        /// The url fetched.
        url: String,
        /// Readable text of the page.
        text: String,
    },
    /// Nothing found.
    None {
        /// What the model can try next.
        note: String,
    },
}

/// Why a search did not answer.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// Platform could not be reached or answered with something unreadable.
    #[error("platform search: {0}")]
    Store(String),
    /// Platform refused the arguments: unknown source, missing detail, unreadable page.
    #[error("{0}")]
    Rejected(String),
    /// Platform answered, but the live source behind it failed or was down.
    #[error("{0}")]
    Unavailable(String),
    /// Platform did not answer inside the budget.
    #[error("the platform did not answer within {0:?}")]
    Timeout(std::time::Duration),
    /// The request ended before the search did.
    #[error("search cancelled")]
    Cancelled,
}
