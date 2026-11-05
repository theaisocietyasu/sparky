//! Canvas tool: the caller's upcoming due dates and events from their calendar feed, in a direct message only.

pub mod client;

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use serde_json::{Value, json};

use crate::core::config::Canvas as CanvasConfig;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::canvas::{CanvasCalendar, CanvasFeeds};
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::tools::canvas::{CanvasError, CanvasItem, parse_ics};
use crate::core::types::tools::upcoming::UpcomingItem;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::Built;
use crate::runtime::tools::account::canvas::client::FeedClient;

/// Name the model calls.
pub const CANVAS_DEADLINES: &str = "canvas_deadlines";

/// How many days a call covers, and how many items it returns.
#[derive(Debug, Clone, Copy)]
pub struct Window {
    /// Days ahead when the model gives none.
    pub default_days: u32,
    /// Most days ahead one call may cover.
    pub max_days: u32,
    /// Most items returned.
    pub max_items: usize,
}

/// The Canvas due dates tool.
pub struct CanvasTool {
    feeds: Arc<dyn CanvasFeeds>,
    calendar: Arc<dyn CanvasCalendar>,
    window: Window,
    offset: FixedOffset,
}

impl CanvasTool {
    /// Builds the tool over the feed links, the feed reader, the window, and the local offset.
    pub fn new(
        feeds: Arc<dyn CanvasFeeds>,
        calendar: Arc<dyn CanvasCalendar>,
        window: Window,
        offset: FixedOffset,
    ) -> Self {
        Self {
            feeds,
            calendar,
            window,
            offset,
        }
    }

    /// The days argument, or the default.
    fn days(&self, args: &Value) -> Result<u32, ToolError> {
        let Some(raw) = args.get("days").filter(|v| !v.is_null()) else {
            return Ok(self.window.default_days);
        };
        raw.as_u64()
            .and_then(|d| u32::try_from(d).ok())
            .filter(|d| (1..=self.window.max_days).contains(d))
            .ok_or_else(|| {
                ToolError::InvalidArguments(format!(
                    "days must be a whole number from 1 to {}",
                    self.window.max_days
                ))
            })
    }
}

/// Maps a feed failure to the text the model reads.
fn failed(error: CanvasError) -> ToolError {
    match error {
        CanvasError::Refused(401 | 403 | 404) => ToolError::Failed(
            "Canvas no longer accepts the saved calendar feed link. Tell the student to link the \
             current Calendar Feed link with /canvas in a Discord direct message or on the \
             Connections page of the web chat, never by pasting it into the chat."
                .to_owned(),
        ),
        other => ToolError::Failed(other.to_string()),
    }
}

#[async_trait]
impl Tool for CanvasTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: CANVAS_DEADLINES.to_owned(),
            description: "List the user's upcoming Canvas assignment due dates and course \
                          events, soonest first, from their Canvas calendar feed. No grades. \
                          Works only in a direct message, after the user has sent /canvas."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "days": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": self.window.max_days,
                        "description": format!(
                            "Days ahead to cover, from today. Default {}.",
                            self.window.default_days
                        ),
                    }
                }
            }),
            risk: RiskClass::ReadAuthenticated,
            private: true,
            sequential: false,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        if ctx.visibility != Visibility::Private {
            return Err(ToolError::Failed(
                "Canvas is available only in a direct message with me, not in a server.".to_owned(),
            ));
        }
        let days = self.days(&args)?;
        let now = Utc::now();
        let end = now + chrono::Duration::days(i64::from(days));
        let items = self.items(ctx, now, end).await?;
        Ok(render(&items, days, self.window.max_items, self.offset))
    }
}

#[async_trait]
impl UpcomingSource for CanvasTool {
    fn label(&self) -> &'static str {
        "Canvas"
    }

    fn tool(&self) -> &'static str {
        CANVAS_DEADLINES
    }

    async fn upcoming(
        &self,
        ctx: &RequestContext,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<UpcomingItem>, String> {
        let items = self
            .items(ctx, from, until)
            .await
            .map_err(|e| e.to_string())?;
        Ok(items
            .into_iter()
            .map(|i| UpcomingItem {
                at: i.at,
                all_day: i.all_day,
                title: i.title,
                detail: i.course,
                url: i.url,
            })
            .collect())
    }
}

impl CanvasTool {
    /// The caller's feed items from from until until, soonest first.
    async fn items(
        &self,
        ctx: &RequestContext,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<CanvasItem>, ToolError> {
        let link = self.feeds.load(&ctx.user_id).await.map_err(|error| {
            tracing::error!(%error, "could not read a Canvas feed link");
            ToolError::Failed("could not read your Canvas link".to_owned())
        })?;
        let Some(link) = link else {
            return Err(ToolError::Failed(
                "You have not linked Canvas, so there is nothing to read. Do not call this tool \
                 again in this turn. Tell the student to link the Canvas Calendar Feed (Canvas, \
                 Calendar, then Calendar Feed) with /canvas in a Discord direct message or on the \
                 Connections page of the web chat."
                    .to_owned(),
            ));
        };
        let text = self.calendar.fetch(&link).await.map_err(failed)?;
        let mut items: Vec<CanvasItem> = parse_ics(&text, self.offset)
            .into_iter()
            .filter(|i| {
                i.at <= until
                    && (i.at >= from || i.all_day && i.at + chrono::Duration::days(1) > from)
            })
            .collect();
        items.sort_by_key(|i| i.at);
        Ok(items)
    }
}

/// The reply: one line per item, at local time.
fn render(items: &[CanvasItem], days: u32, max: usize, offset: FixedOffset) -> ToolOutput {
    let content = if items.is_empty() {
        format!("Nothing due on Canvas in the next {days} days.")
    } else {
        let shown = items.len().min(max);
        let mut text = format!("{shown} Canvas items in the next {days} days:");
        for item in items.iter().take(max) {
            let local = item.at.with_timezone(&offset);
            let when = if item.all_day {
                local.format("%a %Y-%m-%d").to_string()
            } else {
                local.format("%a %Y-%m-%d %H:%M").to_string()
            };
            let course = item
                .course
                .as_deref()
                .map(|c| format!(" ({c})"))
                .unwrap_or_default();
            let link = item
                .url
                .as_deref()
                .map(|u| format!(" {u}"))
                .unwrap_or_default();
            let _ = write!(text, "\n- {when} {}{course}{link}", item.title);
        }
        if items.len() > max {
            let _ = write!(text, "\n{} more not shown.", items.len() - max);
        }
        text
    };
    ToolOutput {
        content,
        data: None,
        sources: Vec::new(),
    }
}

/// The Canvas tool over the feed link store, with a feed client built from the settings.
pub fn tools(
    cfg: &CanvasConfig,
    feeds: Arc<dyn CanvasFeeds>,
    utc_offset_hours: i32,
) -> Result<Built, String> {
    let calendar: Arc<dyn CanvasCalendar> = Arc::new(
        FeedClient::new(
            Duration::from_secs(cfg.timeout_secs),
            cfg.max_bytes,
            &cfg.user_agent,
        )
        .map_err(|e| e.to_string())?,
    );
    let offset = FixedOffset::east_opt(utc_offset_hours * 3600)
        .ok_or_else(|| format!("utc offset {utc_offset_hours} is out of range"))?;
    let window = Window {
        default_days: cfg.default_days,
        max_days: cfg.max_days,
        max_items: cfg.max_items,
    };
    let tool = Arc::new(CanvasTool::new(feeds, calendar, window, offset));
    Ok(Built {
        tools: vec![tool.clone() as Arc<dyn Tool>],
        upcoming: vec![tool as Arc<dyn UpcomingSource>],
    })
}
