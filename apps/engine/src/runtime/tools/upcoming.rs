//! upcoming: the caller's tasks, Canvas due dates and calendar events in one dated list.

use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{FixedOffset, Utc};
use futures::future::join_all;
use serde_json::{Value, json};

use crate::core::traits::tools::Tool;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::tools::upcoming::UpcomingItem;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};

/// Name the model calls.
pub const UPCOMING: &str = "upcoming";

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

/// The upcoming tool over every source the engine registered.
pub struct UpcomingTool {
    sources: Vec<Arc<dyn UpcomingSource>>,
    window: Window,
    offset: FixedOffset,
}

impl UpcomingTool {
    /// Builds the tool over the sources, the window, and the offset dates are shown at.
    pub fn new(sources: Vec<Arc<dyn UpcomingSource>>, window: Window, offset: FixedOffset) -> Self {
        Self {
            sources,
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

#[async_trait]
impl Tool for UpcomingTool {
    fn definition(&self) -> ToolDefinition {
        let names: Vec<&str> = self.sources.iter().map(|s| s.label()).collect();
        ToolDefinition {
            name: UPCOMING.to_owned(),
            description: format!(
                "List what the user has coming up, soonest first, in one call: {}. Use it for \
                 what is due or what is on this week. Works only in a direct message.",
                names.join(", ")
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "days": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": self.window.max_days,
                        "description": format!(
                            "Days ahead to cover, from now. Default {}.",
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
                "What is coming up is available only in a direct message with me.".to_owned(),
            ));
        }
        let days = self.days(&args)?;
        let from = Utc::now();
        let until = from + chrono::Duration::days(i64::from(days));
        let budget = ctx.remaining();
        let reads = self.sources.iter().map(|source| async move {
            let read = tokio::time::timeout(budget, source.upcoming(ctx, from, until)).await;
            let read = read.unwrap_or_else(|_| Err("did not answer in time".to_owned()));
            (source.label(), read)
        });
        let mut items: Vec<(&'static str, UpcomingItem)> = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        for (label, read) in join_all(reads).await {
            match read {
                Ok(found) => items.extend(found.into_iter().map(|i| (label, i))),
                Err(why) => missing.push(format!("{label}: {why}")),
            }
        }
        items.sort_by_key(|(_, i)| i.at);
        Ok(render(
            &items,
            &missing,
            days,
            self.window.max_items,
            self.offset,
        ))
    }
}

/// The reply: one line per item at local time, then one line per source that gave nothing.
fn render(
    items: &[(&'static str, UpcomingItem)],
    missing: &[String],
    days: u32,
    max: usize,
    offset: FixedOffset,
) -> ToolOutput {
    let mut text = if items.is_empty() {
        format!("Nothing coming up in the next {days} days.")
    } else {
        format!("{} items in the next {days} days:", items.len().min(max))
    };
    for (label, item) in items.iter().take(max) {
        let local = item.at.with_timezone(&offset);
        let when = if item.all_day {
            local.format("%a %Y-%m-%d").to_string()
        } else {
            local.format("%a %Y-%m-%d %H:%M").to_string()
        };
        let detail = item
            .detail
            .as_deref()
            .map(|d| format!(" ({d})"))
            .unwrap_or_default();
        let link = item
            .url
            .as_deref()
            .map(|u| format!(" {u}"))
            .unwrap_or_default();
        let _ = write!(text, "\n- {when} [{label}] {}{detail}{link}", item.title);
    }
    if items.len() > max {
        let _ = write!(text, "\n{} more not shown.", items.len() - max);
    }
    for line in missing {
        let _ = write!(text, "\nNot included: {line}");
    }
    ToolOutput {
        content: text,
        data: None,
        sources: Vec::new(),
    }
}
