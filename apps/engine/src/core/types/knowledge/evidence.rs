//! Citation: a source an answer rests on, and how old a fetched page is.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One source an answer rests on. A client decides how to show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    /// Key of the source behind it: a sources row key, or a live source's registry key.
    pub key: String,
    /// Human-readable source name.
    pub title: String,
    /// Canonical page URL, when the source has one.
    pub url: Option<String>,
}

/// How long before now a page was fetched, in hours under two days and in days after.
pub fn age(fetched: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let hours = (now - fetched).num_hours();
    match hours {
        ..1 => "under an hour ago".to_owned(),
        1 => "1 hour ago".to_owned(),
        2..48 => format!("{hours} hours ago"),
        _ => format!("{} days ago", hours / 24),
    }
}
