//! remind and cancel_reminder: the model sets and drops reminders that come back as notifications.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, FixedOffset, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::core::traits::reminder::ReminderStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::reminder::{NewReminder, Reminder, Repeat};
use crate::core::types::tools::upcoming::UpcomingItem;
use crate::core::types::tools::{
    CANCEL_REMINDER, REMIND, RiskClass, ToolDefinition, ToolError, ToolOutput,
};

/// Characters of a reminder id the model sees and sends back.
const SHORT_ID: usize = 8;

/// What remind checks before it writes.
#[derive(Debug, Clone, Copy)]
pub struct RemindRules {
    /// Longest reminder text, in characters.
    pub max_chars: usize,
    /// Most days ahead a reminder may be set.
    pub max_days_ahead: u32,
    /// The offset local times are read in.
    pub offset: FixedOffset,
}

/// The arguments of remind.
#[derive(Debug, Deserialize)]
pub struct RemindArgs {
    /// What to remind the student of.
    pub text: String,
    /// Local date and time, YYYY-MM-DD HH:MM.
    #[serde(default)]
    pub at: Option<String>,
    /// Minutes from now.
    #[serde(default)]
    pub in_minutes: Option<i64>,
    /// How often it comes back.
    #[serde(default)]
    pub repeat: Option<Repeat>,
}

impl RemindRules {
    /// The reminder to keep, or why it is refused.
    pub fn check(&self, args: &RemindArgs, now: DateTime<Utc>) -> Result<NewReminder, String> {
        let text = args.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return Err("the text is empty".into());
        }
        if text.chars().count() > self.max_chars {
            return Err(format!(
                "the text is longer than {} characters",
                self.max_chars
            ));
        }
        let due_at = match (args.at.as_deref().map(str::trim), args.in_minutes) {
            (Some(at), None) if !at.is_empty() => self.local(at)?,
            (None | Some(""), Some(minutes)) if minutes > 0 => now + Duration::minutes(minutes),
            (None | Some(""), Some(_)) => return Err("in_minutes must be at least 1".into()),
            _ => return Err("give either at or in_minutes".into()),
        };
        if due_at <= now {
            return Err("that time has already passed".into());
        }
        if due_at > now + Duration::days(i64::from(self.max_days_ahead)) {
            return Err(format!(
                "a reminder can be at most {} days ahead",
                self.max_days_ahead
            ));
        }
        Ok(NewReminder {
            text,
            due_at,
            repeat: args.repeat.unwrap_or(Repeat::Once),
        })
    }

    /// A local YYYY-MM-DD HH:MM in UTC.
    fn local(&self, at: &str) -> Result<DateTime<Utc>, String> {
        let naive = NaiveDateTime::parse_from_str(at, "%Y-%m-%d %H:%M")
            .map_err(|_| "at must be YYYY-MM-DD HH:MM".to_owned())?;
        self.offset
            .from_local_datetime(&naive)
            .single()
            .map(|t| t.with_timezone(&Utc))
            .ok_or_else(|| "at is not a valid local time".to_owned())
    }

    /// A time as the student reads it.
    pub fn shown(&self, at: DateTime<Utc>) -> String {
        at.with_timezone(&self.offset)
            .format("%A %-d %B, %-I:%M %p")
            .to_string()
    }
}

/// The id of a reminder as the model sees it.
fn short(reminder: &Reminder) -> &str {
    reminder.id.get(..SHORT_ID).unwrap_or(&reminder.id)
}

/// A refusal handed back to the model as the tool result.
fn refused(reason: &str) -> ToolOutput {
    ToolOutput {
        content: format!("No reminder set: {reason}."),
        data: None,
        sources: Vec::new(),
    }
}

/// Sets one reminder for the caller.
pub struct RemindTool {
    store: Arc<dyn ReminderStore>,
    rules: RemindRules,
}

impl RemindTool {
    /// Writes to store under rules.
    pub fn new(store: Arc<dyn ReminderStore>, rules: RemindRules) -> Self {
        Self { store, rules }
    }
}

#[async_trait]
impl Tool for RemindTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: REMIND.to_owned(),
            description: "Set a reminder the student asked for. At the time, Sparky sends them a \
                          direct message and a notification on the web. Give either at, a local \
                          date and time, or in_minutes for a time relative to now. Say back the \
                          time it was set for."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": format!("What to remind them of, at most {} characters.", self.rules.max_chars)
                    },
                    "at": {"type": "string", "description": "Local date and time, YYYY-MM-DD HH:MM, 24 hour."},
                    "in_minutes": {"type": "integer", "description": "Minutes from now, instead of at."},
                    "repeat": {"type": "string", "enum": ["once", "daily", "weekly"], "description": "Defaults to once."}
                },
                "required": ["text"]
            }),
            risk: RiskClass::PrepareWrite,
            private: true,
            sequential: true,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let args: RemindArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let reminder = match self.rules.check(&args, Utc::now()) {
            Ok(reminder) => reminder,
            Err(reason) => return Ok(refused(&reason)),
        };
        let stored = self
            .store
            .add(&ctx.tenant_id, &ctx.user_id, &reminder)
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        let Some(stored) = stored else {
            return Ok(refused(
                "the student has the most reminders allowed; cancel one first",
            ));
        };
        let repeat = match stored.repeat {
            Repeat::Once => String::new(),
            other => format!(", repeating {}", other.as_str()),
        };
        Ok(ToolOutput {
            content: format!(
                "Reminder [{}] set for {}{repeat}.",
                short(&stored),
                self.rules.shown(stored.due_at)
            ),
            data: None,
            sources: Vec::new(),
        })
    }
}

/// Cancels one reminder of the caller by its short id.
pub struct CancelReminderTool {
    store: Arc<dyn ReminderStore>,
}

impl CancelReminderTool {
    /// Removes from store.
    pub fn new(store: Arc<dyn ReminderStore>) -> Self {
        Self { store }
    }
}

#[derive(Debug, Deserialize)]
struct CancelArgs {
    id: String,
}

#[async_trait]
impl Tool for CancelReminderTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: CANCEL_REMINDER.to_owned(),
            description: "Cancel one reminder the student asks to drop, by the id in brackets \
                          that remind or upcoming showed."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "The 8 character id in brackets."}
                },
                "required": ["id"]
            }),
            risk: RiskClass::PrepareWrite,
            private: true,
            sequential: true,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let args: CancelArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let wanted = args.id.trim().trim_matches(['[', ']']).to_lowercase();
        let all = self
            .store
            .list(&ctx.tenant_id, &ctx.user_id)
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        let found = all
            .iter()
            .find(|r| !wanted.is_empty() && r.id.starts_with(&wanted));
        let content = match found {
            Some(reminder) => {
                self.store
                    .cancel(&ctx.tenant_id, &ctx.user_id, &reminder.id)
                    .await
                    .map_err(|e| ToolError::Failed(e.to_string()))?;
                format!("Cancelled the reminder: {}.", reminder.text)
            }
            None => "No reminder has that id.".to_owned(),
        };
        Ok(ToolOutput {
            content,
            data: None,
            sources: Vec::new(),
        })
    }
}

/// The caller's reminders, as a source of the upcoming tool.
pub struct ReminderSource {
    store: Arc<dyn ReminderStore>,
}

impl ReminderSource {
    /// Builds the source over the reminder store.
    pub fn new(store: Arc<dyn ReminderStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl UpcomingSource for ReminderSource {
    fn label(&self) -> &'static str {
        "Reminder"
    }

    fn tool(&self) -> &'static str {
        REMIND
    }

    async fn upcoming(
        &self,
        ctx: &RequestContext,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<Vec<UpcomingItem>, String> {
        let all = self
            .store
            .list(&ctx.tenant_id, &ctx.user_id)
            .await
            .map_err(|error| {
                tracing::error!(%error, "could not list reminders");
                "could not read your reminders".to_owned()
            })?;
        Ok(all
            .iter()
            .filter(|r| r.due_at >= from && r.due_at <= until)
            .map(|r| UpcomingItem {
                at: r.due_at,
                all_day: false,
                title: format!("[{}] {}", short(r), r.text),
                detail: (r.repeat != Repeat::Once)
                    .then(|| format!("repeats {}", r.repeat.as_str())),
                url: None,
            })
            .collect())
    }
}

/// The remind tools and the upcoming source over one store.
pub fn tools(store: &Arc<dyn ReminderStore>, rules: RemindRules) -> crate::runtime::tools::Built {
    crate::runtime::tools::Built {
        tools: vec![
            Arc::new(RemindTool::new(Arc::clone(store), rules)),
            Arc::new(CancelReminderTool::new(Arc::clone(store))),
        ],
        upcoming: vec![Arc::new(ReminderSource::new(Arc::clone(store)))],
    }
}
