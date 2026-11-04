//! Canvas calendar feed: the link a student shares, the items it lists, and how a feed fails.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};

/// Longest feed link accepted.
const MAX_LINK_CHARS: usize = 512;

/// Path every Canvas user calendar feed starts with.
const FEED_PATH: &str = "/feeds/calendars/user_";

/// Feed failures. Reported to the model as text.
#[derive(Debug, thiserror::Error)]
pub enum CanvasError {
    /// The feed host could not be reached.
    #[error("Canvas could not be reached: {0}")]
    Unreachable(String),
    /// The feed host answered with an error status.
    #[error("Canvas refused the calendar feed ({0})")]
    Refused(u16),
    /// The feed was larger than the configured limit.
    #[error("the Canvas calendar feed is larger than {0} bytes")]
    TooLarge(usize),
}

/// One dated entry of the feed: an assignment due date or a calendar event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasItem {
    /// Title without the course suffix.
    pub title: String,
    /// Course code Canvas appends in brackets, when present.
    pub course: Option<String>,
    /// When it is due or starts.
    pub at: DateTime<Utc>,
    /// Whether the entry has a date and no time.
    pub all_day: bool,
    /// Link to the item in Canvas, when the feed gives one.
    pub url: Option<String>,
}

/// The feed link when it is an https Canvas user calendar feed on one of hosts, else why not.
pub fn feed_link(raw: &str, hosts: &[String]) -> Result<String, String> {
    let link = raw.trim();
    if link.is_empty() || link.chars().count() > MAX_LINK_CHARS {
        return Err("the link is empty or too long".into());
    }
    let parsed = url::Url::parse(link).map_err(|_| "that is not a link".to_owned())?;
    if parsed.scheme() != "https" {
        return Err("the link must start with https://".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.port().is_some() {
        return Err("the link must not carry a user name, password or port".into());
    }
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    if !hosts.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
        return Err(format!("the link must be on {}", hosts.join(" or ")));
    }
    let path = parsed.path();
    if !path.starts_with(FEED_PATH) || !path.to_ascii_lowercase().ends_with(".ics") {
        return Err(
            "that is not a Canvas calendar feed link (Calendar, then Calendar Feed)".into(),
        );
    }
    Ok(parsed.to_string())
}

/// Every event of an iCalendar text. A TZID time is read at offset; a floating time too.
pub fn parse_ics(text: &str, offset: FixedOffset) -> Vec<CanvasItem> {
    let mut items = Vec::new();
    let mut current: Option<Draft> = None;
    for line in unfold(text) {
        let Some((head, value)) = line.split_once(':') else {
            continue;
        };
        let (name, params) = head.split_once(';').unwrap_or((head, ""));
        match (name.to_ascii_uppercase().as_str(), value) {
            ("BEGIN", "VEVENT") => current = Some(Draft::default()),
            ("END", "VEVENT") => {
                if let Some(item) = current.take().and_then(|d| d.finish(offset)) {
                    items.push(item);
                }
            }
            ("SUMMARY", v) => {
                if let Some(d) = current.as_mut() {
                    d.summary = Some(unescape(v));
                }
            }
            ("DTSTART", v) => {
                if let Some(d) = current.as_mut() {
                    d.start = Some((params.to_ascii_uppercase(), v.trim().to_owned()));
                }
            }
            ("URL", v) => {
                if let Some(d) = current.as_mut() {
                    d.url = Some(v.trim().to_owned());
                }
            }
            _ => {}
        }
    }
    items
}

/// The fields of one VEVENT as they are read.
#[derive(Default)]
struct Draft {
    summary: Option<String>,
    start: Option<(String, String)>,
    url: Option<String>,
}

impl Draft {
    /// The item, or None when the event has no readable start.
    fn finish(self, offset: FixedOffset) -> Option<CanvasItem> {
        let (params, value) = self.start?;
        let (at, all_day) = start(&params, &value, offset)?;
        let summary = self.summary.unwrap_or_else(|| "(no title)".to_owned());
        let (title, course) = split_course(&summary);
        Some(CanvasItem {
            title,
            course,
            at,
            all_day,
            url: self.url.filter(|u| u.starts_with("https://")),
        })
    }
}

/// A DTSTART value as an instant and whether it is a date only.
fn start(params: &str, value: &str, offset: FixedOffset) -> Option<(DateTime<Utc>, bool)> {
    if params.contains("VALUE=DATE") && !params.contains("VALUE=DATE-TIME") || value.len() == 8 {
        let day = NaiveDate::parse_from_str(value, "%Y%m%d").ok()?;
        let local = offset
            .from_local_datetime(&day.and_hms_opt(0, 0, 0)?)
            .single()?;
        return Some((local.with_timezone(&Utc), true));
    }
    if let Some(utc) = value.strip_suffix('Z') {
        let naive = NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S").ok()?;
        return Some((Utc.from_utc_datetime(&naive), false));
    }
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%S").ok()?;
    let local = offset.from_local_datetime(&naive).single()?;
    Some((local.with_timezone(&Utc), false))
}

/// The title and the bracketed course code Canvas appends to a summary.
fn split_course(summary: &str) -> (String, Option<String>) {
    let trimmed = summary.trim();
    if let Some(body) = trimmed.strip_suffix(']')
        && let Some(open) = body.rfind('[')
    {
        let title = body[..open].trim();
        let course = body[open + 1..].trim();
        if !title.is_empty() && !course.is_empty() {
            return (title.to_owned(), Some(course.to_owned()));
        }
    }
    (trimmed.to_owned(), None)
}

/// Content lines with folded continuations joined back.
fn unfold(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.lines() {
        let raw = raw.trim_end_matches('\r');
        if let Some(rest) = raw.strip_prefix([' ', '\t'])
            && let Some(last) = lines.last_mut()
        {
            last.push_str(rest);
        } else {
            lines.push(raw.to_owned());
        }
    }
    lines
}

/// Text with the iCalendar escapes undone.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push(' '),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out.trim().to_owned()
}
