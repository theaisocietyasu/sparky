//! What the upcoming tool lists: one dated item from any source.

use chrono::{DateTime, Utc};

/// One dated entry: a task due, an assignment due, or a calendar event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpcomingItem {
    /// When it is due or starts.
    pub at: DateTime<Utc>,
    /// Whether it has a date and no time.
    pub all_day: bool,
    /// What it is.
    pub title: String,
    /// A course or a place, when the source gives one.
    pub detail: Option<String>,
    /// Link to it, when the source gives one.
    pub url: Option<String>,
}
