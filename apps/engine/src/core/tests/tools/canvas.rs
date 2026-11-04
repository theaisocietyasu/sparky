//! Canvas calendar feed: link checks, the iCalendar parser, and the canvas_deadlines tool.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{FixedOffset, TimeZone, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;

use crate::core::traits::tools::Tool;
use crate::core::traits::tools::canvas::{CanvasCalendar, CanvasFeeds};
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::store::StoreError;
use crate::core::types::tools::canvas::{CanvasError, feed_link, parse_ics};
use crate::core::types::tools::{RiskClass, ToolError};
use crate::runtime::tools::account::canvas::{CanvasTool, Window};

const LINK: &str = "https://canvas.asu.edu/feeds/calendars/user_AbC123.ics";

fn hosts() -> Vec<String> {
    vec!["canvas.asu.edu".into()]
}

fn arizona() -> FixedOffset {
    FixedOffset::west_opt(7 * 3600).unwrap_or_else(|| unreachable!("a valid offset"))
}

#[test]
fn a_canvas_calendar_feed_link_is_accepted() {
    assert_eq!(
        feed_link(&format!("  {LINK} "), &hosts()).as_deref(),
        Ok(LINK)
    );
}

#[test]
fn links_that_are_not_a_canvas_feed_are_refused() {
    for bad in [
        "",
        "not a link",
        "http://canvas.asu.edu/feeds/calendars/user_A.ics",
        "https://evil.example.com/feeds/calendars/user_A.ics",
        "https://canvas.asu.edu.evil.com/feeds/calendars/user_A.ics",
        "https://me:pw@canvas.asu.edu/feeds/calendars/user_A.ics",
        "https://canvas.asu.edu:8443/feeds/calendars/user_A.ics",
        "https://canvas.asu.edu/courses/1/assignments",
        "https://canvas.asu.edu/feeds/calendars/user_A.txt",
    ] {
        assert!(feed_link(bad, &hosts()).is_err(), "{bad} was accepted");
    }
    let long = format!(
        "https://canvas.asu.edu/feeds/calendars/user_{}.ics",
        "a".repeat(600)
    );
    assert!(feed_link(&long, &hosts()).is_err());
}

const FEED: &str = "BEGIN:VCALENDAR\r
VERSION:2.0\r
BEGIN:VEVENT\r
DTSTART:20261110T065900Z\r
SUMMARY:Homework 3\\, part A [CSE310-2026Fall]\r
URL;VALUE=URI:https://canvas.asu.edu/courses/1/assignments/9\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;VALUE=DATE:20261112\r
SUMMARY:Midterm review [MAT 343]\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;TZID=America/Phoenix:20261111T090000\r
SUMMARY:Lab sign-up opens for a very long ti\r
 tle\r
URL:javascript:alert(1)\r
END:VEVENT\r
BEGIN:VEVENT\r
SUMMARY:No start\r
END:VEVENT\r
END:VCALENDAR\r
";

#[test]
fn the_feed_parses_into_dated_items() {
    let items = parse_ics(FEED, arizona());
    assert_eq!(items.len(), 3, "{items:?}");
    assert_eq!(items[0].title, "Homework 3, part A");
    assert_eq!(items[0].course.as_deref(), Some("CSE310-2026Fall"));
    assert_eq!(
        items[0].at,
        Utc.with_ymd_and_hms(2026, 11, 10, 6, 59, 0)
            .single()
            .unwrap_or_default()
    );
    assert!(!items[0].all_day);
    assert_eq!(
        items[0].url.as_deref(),
        Some("https://canvas.asu.edu/courses/1/assignments/9")
    );
    assert!(items[1].all_day);
    assert_eq!(items[1].course.as_deref(), Some("MAT 343"));
    assert_eq!(
        items[1].at,
        Utc.with_ymd_and_hms(2026, 11, 12, 7, 0, 0)
            .single()
            .unwrap_or_default()
    );
    assert_eq!(items[2].title, "Lab sign-up opens for a very long title");
    assert_eq!(
        items[2].at,
        Utc.with_ymd_and_hms(2026, 11, 11, 16, 0, 0)
            .single()
            .unwrap_or_default()
    );
    assert_eq!(items[2].url, None);
}

/// One link per user, held in memory.
#[derive(Default)]
struct Feeds(Mutex<Option<String>>);

#[async_trait]
impl CanvasFeeds for Feeds {
    async fn save(&self, _user: &str, link: &str) -> Result<(), StoreError> {
        if let Ok(mut held) = self.0.lock() {
            *held = Some(link.to_owned());
        }
        Ok(())
    }
    async fn load(&self, _user: &str) -> Result<Option<SecretString>, StoreError> {
        Ok(self
            .0
            .lock()
            .ok()
            .and_then(|h| h.clone())
            .map(SecretString::from))
    }
    async fn delete(&self, _user: &str) -> Result<bool, StoreError> {
        Ok(self.0.lock().ok().and_then(|mut h| h.take()).is_some())
    }
}

/// A calendar that serves canned text, or refuses with a status, and records the link it read.
struct Calendar {
    text: String,
    status: Option<u16>,
    read: Mutex<Vec<String>>,
}

#[async_trait]
impl CanvasCalendar for Calendar {
    async fn fetch(&self, link: &SecretString) -> Result<String, CanvasError> {
        if let Ok(mut read) = self.read.lock() {
            read.push(link.expose_secret().to_owned());
        }
        match self.status {
            Some(code) => Err(CanvasError::Refused(code)),
            None => Ok(self.text.clone()),
        }
    }
}

fn stamp(days: i64) -> String {
    (Utc::now() + chrono::Duration::days(days))
        .format("%Y%m%dT%H%M%SZ")
        .to_string()
}

fn event(summary: &str, days: i64) -> String {
    format!(
        "BEGIN:VEVENT\nDTSTART:{}\nSUMMARY:{summary}\nEND:VEVENT\n",
        stamp(days)
    )
}

fn window() -> Window {
    Window {
        default_days: 14,
        max_days: 60,
        max_items: 2,
    }
}

fn tool(linked: bool, text: String, status: Option<u16>) -> (CanvasTool, Arc<Calendar>) {
    let feeds = Feeds::default();
    if linked && let Ok(mut held) = feeds.0.lock() {
        *held = Some(LINK.to_owned());
    }
    let calendar = Arc::new(Calendar {
        text,
        status,
        read: Mutex::new(Vec::new()),
    });
    let tool = CanvasTool::new(
        Arc::new(feeds),
        Arc::clone(&calendar) as Arc<dyn CanvasCalendar>,
        window(),
        arizona(),
    );
    (tool, calendar)
}

fn dm() -> RequestContext {
    RequestContext::new("g", "u", Duration::from_secs(5)).with_visibility(Visibility::Private)
}

#[test]
fn the_tool_is_private_and_authenticated() {
    let (tool, _) = tool(true, String::new(), None);
    let definition = tool.definition();
    assert!(definition.private);
    assert_eq!(definition.risk, RiskClass::ReadAuthenticated);
}

#[tokio::test]
async fn the_tool_is_refused_outside_a_direct_message() {
    let (tool, calendar) = tool(true, String::new(), None);
    let server = RequestContext::new("g", "u", Duration::from_secs(5));
    let out = tool.call(&server, json!({})).await;
    assert!(
        matches!(&out, Err(ToolError::Failed(m)) if m.contains("direct message")),
        "{out:?}"
    );
    assert!(calendar.read.lock().is_ok_and(|r| r.is_empty()));
}

#[tokio::test]
async fn an_unlinked_user_is_told_to_send_the_feed_link() {
    let (tool, _) = tool(false, String::new(), None);
    let out = tool.call(&dm(), json!({})).await;
    assert!(
        matches!(&out, Err(ToolError::Failed(m)) if m.contains("/canvas")),
        "{out:?}"
    );
}

#[tokio::test]
async fn items_in_the_window_are_listed_soonest_first_and_capped() {
    let feed = format!(
        "BEGIN:VCALENDAR\n{}{}{}{}{}END:VCALENDAR\n",
        event("Later [B]", 5),
        event("Past [A]", -2),
        event("Soon [A]", 1),
        event("Out of window [C]", 30),
        event("Third [D]", 6),
    );
    let (tool, calendar) = tool(true, feed, None);
    let Ok(out) = tool.call(&dm(), json!({"days": 10})).await else {
        unreachable!("the tool answers")
    };
    let soon = out.content.find("Soon (A)");
    let later = out.content.find("Later (B)");
    assert!(
        soon.is_some() && later.is_some() && soon < later,
        "{}",
        out.content
    );
    assert!(!out.content.contains("Past") && !out.content.contains("Out of window"));
    assert!(!out.content.contains("Third") && out.content.contains("1 more not shown"));
    assert!(out.data.is_none());
    assert!(
        calendar
            .read
            .lock()
            .is_ok_and(|r| r.as_slice() == [LINK.to_owned()])
    );
}

#[tokio::test]
async fn days_outside_the_window_are_invalid() {
    let (tool, _) = tool(true, String::new(), None);
    for days in [json!(0), json!(61), json!("soon")] {
        let out = tool.call(&dm(), json!({ "days": days })).await;
        assert!(
            matches!(out, Err(ToolError::InvalidArguments(_))),
            "{days}: {out:?}"
        );
    }
}

#[tokio::test]
async fn a_revoked_feed_asks_for_a_new_link() {
    let (tool, _) = tool(true, String::new(), Some(401));
    let out = tool.call(&dm(), json!({})).await;
    assert!(
        matches!(&out, Err(ToolError::Failed(m)) if m.contains("/canvas")),
        "{out:?}"
    );
}
