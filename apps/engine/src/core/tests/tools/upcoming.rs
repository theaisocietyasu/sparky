//! upcoming: merges and sorts every source, names the ones that gave nothing, and keeps to the deadline.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::core::traits::memory::MemoryStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::memory::{Memory, MemoryKind, NewMemory};
use crate::core::types::store::StoreError;
use crate::core::types::tools::upcoming::{UpcomingItem, parse_start};
use crate::core::types::tools::{RiskClass, ToolError};
use crate::runtime::tools::memory::MemoryTasks;
use crate::runtime::tools::upcoming::{UpcomingTool, Window};

fn utc() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap_or_else(|| unreachable!("a valid offset"))
}

fn arizona() -> FixedOffset {
    FixedOffset::west_opt(7 * 3600).unwrap_or_else(|| unreachable!("a valid offset"))
}

fn item(title: &str, hours: i64) -> UpcomingItem {
    UpcomingItem {
        at: Utc::now() + chrono::Duration::hours(hours),
        all_day: false,
        title: title.into(),
        detail: None,
        url: None,
    }
}

/// A source with canned items, a canned failure, or one that never answers.
struct Fake {
    label: &'static str,
    answer: Result<Vec<UpcomingItem>, String>,
    hang: bool,
}

#[async_trait]
impl UpcomingSource for Fake {
    fn label(&self) -> &'static str {
        self.label
    }
    fn tool(&self) -> &'static str {
        "fake"
    }
    async fn upcoming(
        &self,
        _ctx: &RequestContext,
        _from: DateTime<Utc>,
        _until: DateTime<Utc>,
    ) -> Result<Vec<UpcomingItem>, String> {
        if self.hang {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        self.answer.clone()
    }
}

fn fake(label: &'static str, answer: Result<Vec<UpcomingItem>, String>) -> Arc<dyn UpcomingSource> {
    Arc::new(Fake {
        label,
        answer,
        hang: false,
    })
}

fn tool(sources: Vec<Arc<dyn UpcomingSource>>) -> UpcomingTool {
    let window = Window {
        default_days: 7,
        max_days: 30,
        max_items: 3,
    };
    UpcomingTool::new(sources, window, utc())
}

fn dm(budget: Duration) -> RequestContext {
    RequestContext::new("g", "u", budget).with_visibility(Visibility::Private)
}

#[test]
fn the_tool_is_private_and_authenticated() {
    let definition = tool(vec![fake("Canvas", Ok(Vec::new()))]).definition();
    assert!(definition.private);
    assert_eq!(definition.risk, RiskClass::ReadAuthenticated);
    assert!(definition.description.contains("Canvas"));
}

#[tokio::test]
async fn items_from_every_source_come_back_in_one_list_by_date() {
    let upcoming = tool(vec![
        fake("Canvas", Ok(vec![item("Homework 3", 30), item("Quiz", 2)])),
        fake("Google Calendar", Ok(vec![item("Club meeting", 10)])),
        fake(
            "Outlook",
            Err("not connected; send /login and choose Outlook".into()),
        ),
    ]);
    let Ok(out) = upcoming.call(&dm(Duration::from_secs(5)), json!({})).await else {
        unreachable!("the tool answers")
    };
    let at = |s: &str| out.content.find(s).unwrap_or(usize::MAX);
    assert!(
        at("[Canvas] Quiz") < at("[Google Calendar] Club meeting"),
        "{}",
        out.content
    );
    assert!(
        at("Club meeting") < at("[Canvas] Homework 3"),
        "{}",
        out.content
    );
    assert!(
        out.content.contains("Not included: Outlook: not connected"),
        "{}",
        out.content
    );
    assert!(out.data.is_none());
}

#[tokio::test]
async fn the_list_is_capped() {
    let many = (1..=5).map(|h| item(&format!("Item {h}"), h)).collect();
    let Ok(out) = tool(vec![fake("Canvas", Ok(many))])
        .call(&dm(Duration::from_secs(5)), json!({}))
        .await
    else {
        unreachable!("the tool answers")
    };
    assert!(
        out.content.contains("Item 3") && !out.content.contains("Item 4"),
        "{}",
        out.content
    );
    assert!(out.content.contains("2 more not shown"), "{}", out.content);
}

#[tokio::test]
async fn a_slow_source_is_cut_at_the_deadline() {
    let slow: Arc<dyn UpcomingSource> = Arc::new(Fake {
        label: "Outlook",
        answer: Ok(vec![item("Never", 1)]),
        hang: true,
    });
    let upcoming = tool(vec![slow, fake("Canvas", Ok(vec![item("Quiz", 2)]))]);
    let Ok(out) = upcoming
        .call(&dm(Duration::from_millis(100)), json!({}))
        .await
    else {
        unreachable!("the tool answers")
    };
    assert!(
        out.content.contains("Quiz") && !out.content.contains("Never"),
        "{}",
        out.content
    );
    assert!(
        out.content.contains("Outlook: did not answer in time"),
        "{}",
        out.content
    );
}

#[tokio::test]
async fn the_tool_is_refused_in_a_server_and_checks_days() {
    let upcoming = tool(vec![fake("Canvas", Ok(Vec::new()))]);
    let server = RequestContext::new("g", "u", Duration::from_secs(5));
    let out = upcoming.call(&server, json!({})).await;
    assert!(
        matches!(&out, Err(ToolError::Failed(m)) if m.contains("direct message")),
        "{out:?}"
    );
    for days in [json!(0), json!(31), json!("soon")] {
        let out = upcoming
            .call(&dm(Duration::from_secs(5)), json!({ "days": days }))
            .await;
        assert!(
            matches!(out, Err(ToolError::InvalidArguments(_))),
            "{days}: {out:?}"
        );
    }
}

#[test]
fn a_start_is_read_in_each_form_the_calendars_send() {
    let at = |y, mo, d, h, mi| {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0)
            .single()
            .unwrap_or_default()
    };
    assert_eq!(
        parse_start("2026-11-05T09:00:00-07:00", arizona()),
        Some((at(2026, 11, 5, 16, 0), false))
    );
    assert_eq!(
        parse_start("2026-11-05", arizona()),
        Some((at(2026, 11, 5, 7, 0), true))
    );
    assert_eq!(
        parse_start("2026-11-05T17:30:00.0000000", arizona()),
        Some((at(2026, 11, 5, 17, 30), false))
    );
    assert_eq!(parse_start("soon", arizona()), None);
}

/// A store that holds fixed memories.
struct Held(Vec<Memory>);

#[async_trait]
impl MemoryStore for Held {
    async fn recall(&self, _ctx: &RequestContext) -> Result<Vec<Memory>, StoreError> {
        Ok(self.0.clone())
    }
    async fn remember(&self, _ctx: &RequestContext, _m: &NewMemory) -> Result<Memory, StoreError> {
        Err(StoreError::Database("read only".into()))
    }
    async fn forget(&self, _ctx: &RequestContext, _id: Uuid) -> Result<bool, StoreError> {
        Ok(false)
    }
    async fn forget_all(&self, _ctx: &RequestContext) -> Result<u64, StoreError> {
        Ok(0)
    }
}

fn memory(kind: MemoryKind, text: &str, due_in_days: Option<i64>) -> Memory {
    Memory {
        id: Uuid::new_v4(),
        kind,
        content: text.into(),
        sensitive: false,
        created_at: Utc::now(),
        expires_at: due_in_days.map(|d| Utc::now() + chrono::Duration::days(d)),
    }
}

#[tokio::test]
async fn memory_tasks_in_the_window_are_listed_and_facts_are_not() {
    let store = Held(vec![
        memory(MemoryKind::Task, "Lab report", Some(3)),
        memory(MemoryKind::Task, "Far away", Some(40)),
        memory(MemoryKind::Fact, "studies CSE", Some(2)),
    ]);
    let source = MemoryTasks::new(Arc::new(store));
    let now = Utc::now();
    let found = source
        .upcoming(
            &dm(Duration::from_secs(5)),
            now,
            now + chrono::Duration::days(7),
        )
        .await;
    let titles: Vec<String> = found
        .unwrap_or_default()
        .into_iter()
        .map(|i| i.title)
        .collect();
    assert_eq!(titles, vec!["Lab report".to_owned()]);
}
