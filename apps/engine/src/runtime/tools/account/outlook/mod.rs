//! Outlook tools: read the caller's calendar and mail, in a direct message only.

pub mod client;
mod render;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Offset, Utc};
use serde_json::{Value, json};

use crate::core::config::Outlook as OutlookConfig;
use crate::core::traits::oauth::OAuthStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::outlook::Outlook;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::tools::outlook::OutlookError;
use crate::core::types::tools::upcoming::{UpcomingItem, parse_start};
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};
use crate::runtime::tools::Built;
use crate::runtime::tools::account::grant::Credentials;
use crate::runtime::tools::account::outlook::client::GraphClient;
use crate::runtime::tools::account::outlook::render::{calendar_output, mail_output};

/// The provider key Outlook grants are stored under.
const PROVIDER: &str = "microsoft";

/// Which read an Outlook tool performs.
#[derive(Debug, Clone, Copy)]
pub enum Query {
    /// Upcoming calendar events.
    Calendar,
    /// Recent mail.
    Mail,
}

/// Every read an Outlook tool is built for.
const ALL: [Query; 2] = [Query::Calendar, Query::Mail];

impl Query {
    /// The tool name the model calls.
    fn name(self) -> &'static str {
        match self {
            Self::Calendar => "outlook_calendar",
            Self::Mail => "outlook_mail",
        }
    }

    /// What the tool does, for the model.
    fn description(self) -> &'static str {
        match self {
            Self::Calendar => {
                "List the user's upcoming Outlook calendar events, soonest first. Works only in a \
                 direct message, and only after the user has connected Outlook."
            }
            Self::Mail => {
                "List the subjects and senders of the user's most recent Outlook mail. Works only \
                 in a direct message, and only after the user has connected Outlook."
            }
        }
    }
}

/// One Outlook read, offered to the model.
pub struct OutlookTool {
    query: Query,
    client: Arc<dyn Outlook>,
    creds: Arc<Credentials>,
    max_items: usize,
}

impl OutlookTool {
    /// Builds a tool over the client and the shared credentials.
    pub fn new(
        query: Query,
        client: Arc<dyn Outlook>,
        creds: Arc<Credentials>,
        max_items: usize,
    ) -> Self {
        Self {
            query,
            client,
            creds,
            max_items,
        }
    }
}

/// Maps a Graph failure to the text the model reads.
fn failed(error: OutlookError) -> ToolError {
    match error {
        OutlookError::Refused(401 | 403) => {
            ToolError::Failed("Outlook rejected the token; it may have expired.".to_owned())
        }
        other => ToolError::Failed(other.to_string()),
    }
}

#[async_trait]
impl Tool for OutlookTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.query.name().to_owned(),
            description: self.query.description().to_owned(),
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
                "Outlook is available only in a direct message with me, not in a server."
                    .to_owned(),
            ));
        }
        let Some(token) = self.creds.resolve(ctx).await? else {
            return Err(ToolError::Failed(
                "Outlook is not linked. Call settings with action connect and account outlook, and hand the student the link."
                    .to_owned(),
            ));
        };
        match self.query {
            Query::Calendar => {
                let events = self.client.calendar(&token).await.map_err(failed)?;
                Ok(calendar_output(events, self.max_items))
            }
            Query::Mail => {
                let mail = self.client.mail(&token).await.map_err(failed)?;
                Ok(mail_output(mail, self.max_items))
            }
        }
    }
}

/// The caller's Outlook events, as a source of the upcoming tool.
pub struct OutlookSource {
    client: Arc<dyn Outlook>,
    creds: Arc<Credentials>,
}

impl OutlookSource {
    /// Builds the source over the Graph client and the shared credentials.
    pub fn new(client: Arc<dyn Outlook>, creds: Arc<Credentials>) -> Self {
        Self { client, creds }
    }
}

#[async_trait]
impl UpcomingSource for OutlookSource {
    fn label(&self) -> &'static str {
        "Outlook"
    }

    fn tool(&self) -> &'static str {
        Query::Calendar.name()
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
            .calendar(&token)
            .await
            .map_err(|e| failed(e).to_string())?;
        Ok(events
            .into_iter()
            .filter_map(|e| {
                // Graph returns start times in UTC with no offset.
                let (at, all_day) = parse_start(e.start.as_deref()?, Utc.fix())?;
                (at >= from && at <= until).then_some(UpcomingItem {
                    at,
                    all_day,
                    title: e.subject,
                    detail: e.location,
                    url: e.url,
                })
            })
            .collect())
    }
}

/// The Outlook tools and the calendar source, over the Graph client and the per-user grant store.
pub fn tools(cfg: &OutlookConfig, store: Arc<dyn OAuthStore>) -> Result<Built, OutlookError> {
    let client: Arc<dyn Outlook> = Arc::new(GraphClient::new(
        &cfg.base_url,
        Duration::from_secs(cfg.timeout_secs),
        cfg.max_items,
    )?);
    let creds = Arc::new(Credentials::new(store, PROVIDER));
    let source: Arc<dyn UpcomingSource> =
        Arc::new(OutlookSource::new(client.clone(), creds.clone()));
    let tools = ALL
        .into_iter()
        .map(|q| {
            Arc::new(OutlookTool::new(
                q,
                client.clone(),
                creds.clone(),
                cfg.max_items,
            )) as Arc<dyn Tool>
        })
        .collect();
    Ok(Built {
        tools,
        upcoming: vec![source],
    })
}
