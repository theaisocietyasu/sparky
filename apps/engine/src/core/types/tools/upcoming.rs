//! What the upcoming tool lists: one dated item from any source, and how a start time is read.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};

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

/// A start as a calendar API returns it: RFC 3339, a date, or a UTC time with no offset.
pub fn parse_start(raw: &str, offset: FixedOffset) -> Option<(DateTime<Utc>, bool)> {
    let raw = raw.trim();
    if let Ok(at) = DateTime::parse_from_rfc3339(raw) {
        return Some((at.with_timezone(&Utc), false));
    }
    if let Ok(day) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        let local = offset
            .from_local_datetime(&day.and_hms_opt(0, 0, 0)?)
            .single()?;
        return Some((local.with_timezone(&Utc), true));
    }
    let naive = NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
    Some((Utc.from_utc_datetime(&naive), false))
}
