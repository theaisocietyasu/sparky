//! Google Calendar tool: read the caller's upcoming events, in a direct message only.

pub mod client;
mod render;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Offset, Utc};
use serde_json::{Value, json};

use crate::core::config::Gcal as GcalConfig;
use crate::core::traits::oauth::OAuthStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::gcal::GoogleCalendar;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::tools::gcal::GCalError;
use crate::core::types::tools::upcoming::{UpcomingItem, parse_start};
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::Built;
use crate::runtime::tools::account::gcal::client::GCalClient;
use crate::runtime::tools::account::gcal::render::output;
use crate::runtime::tools::account::grant::Credentials;

/// Name the model calls.
pub const GOOGLE_CALENDAR: &str = "google_calendar";

/// The provider key Google grants are stored under.
const PROVIDER: &str = "google";

/// The Google Calendar read tool.
pub struct GcalTool {
    client: Arc<dyn GoogleCalendar>,
    creds: Arc<Credentials>,
    max_items: usize,
}

impl GcalTool {
    /// Builds the tool over the client and the shared credentials.
    pub fn new(client: Arc<dyn GoogleCalendar>, creds: Arc<Credentials>, max_items: usize) -> Self {
        Self {
            client,
            creds,
            max_items,
        }
    }
}

/// Maps a Calendar failure to the text the model reads.
fn failed(error: GCalError) -> ToolError {
    match error {
        GCalError::Refused(401 | 403) => {
            ToolError::Failed("Google rejected the token; it may have expired.".to_owned())
        }
        other => ToolError::Failed(other.to_string()),
    }
}

#[async_trait]
impl Tool for GcalTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: GOOGLE_CALENDAR.to_owned(),
            description: "List the user's upcoming Google Calendar events, soonest first. Works \
                          only in a direct message, and only after the user has connected Google."
                .to_owned(),
            parameters: json!({ "type": "object", "properties": {} }),
            risk: RiskClass::ReadAuthenticated,
            private: false,
            sequential: false,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, _args: Value) -> Result<ToolOutput, ToolError> {
        if ctx.visibility != Visibility::Private {
            return Err(ToolError::Failed(
                "Google Calendar is available only in a direct message with me, not in a server."
                    .to_owned(),
            ));
        }
        let Some(token) = self.creds.resolve(ctx).await? else {
            return Err(ToolError::Failed(
                "Google is not linked. Call settings with action connect and account google, and hand the student the link."
                    .to_owned(),
            ));
        };
        let events = self.client.events(&token).await.map_err(failed)?;
        Ok(output(events, self.max_items))
    }
}

/// The caller's Google events, as a source of the upcoming tool.
pub struct GcalSource {
    client: Arc<dyn GoogleCalendar>,
    creds: Arc<Credentials>,
    offset: FixedOffset,
}

impl GcalSource {
    /// Builds the source over the client, the shared credentials, and the offset of all-day dates.
    pub fn new(
        client: Arc<dyn GoogleCalendar>,
        creds: Arc<Credentials>,
        offset: FixedOffset,
    ) -> Self {
        Self {
            client,
            creds,
            offset,
        }
    }
}

#[async_trait]
impl UpcomingSource for GcalSource {
    fn label(&self) -> &'static str {
        "Google Calendar"
    }

    fn tool(&self) -> &'static str {
        GOOGLE_CALENDAR
    }

    async fn upcoming(
        &self,
        ctx: &RequestContext,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<UpcomingItem>, String> {
        let token = self
            .creds
            .resolve(ctx)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "not linked; settings with action connect links it".to_owned())?;
        let events = self
            .client
            .events(&token)
            .await
            .map_err(|e| failed(e).to_string())?;
        Ok(events
            .into_iter()
            .filter_map(|e| {
                let (at, all_day) = parse_start(e.start.as_deref()?, self.offset)?;
                (at >= from && at <= until).then_some(UpcomingItem {
                    at,
                    all_day,
                    title: e.summary,
                    detail: e.location,
                    url: e.url,
                })
            })
            .collect())
    }
}

/// The Google Calendar tool and its upcoming source, over the client and the per-user grant store.
pub fn tools(
    cfg: &GcalConfig,
    store: Arc<dyn OAuthStore>,
    utc_offset_hours: i32,
) -> Result<Built, GCalError> {
    let client: Arc<dyn GoogleCalendar> = Arc::new(GCalClient::new(
        &cfg.base_url,
        Duration::from_secs(cfg.timeout_secs),
        cfg.max_items,
    )?);
    let creds = Arc::new(Credentials::new(store, PROVIDER));
    let offset = FixedOffset::east_opt(utc_offset_hours * 3600).unwrap_or(Utc.fix());
    let source: Arc<dyn UpcomingSource> =
        Arc::new(GcalSource::new(client.clone(), creds.clone(), offset));
    let tool: Arc<dyn Tool> = Arc::new(GcalTool::new(client, creds, cfg.max_items));
    Ok(Built {
        tools: vec![tool],
        upcoming: vec![source],
    })
}
