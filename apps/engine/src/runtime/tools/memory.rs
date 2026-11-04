//! remember and forget_memory: the model keeps and drops what a student states about themselves.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{FixedOffset, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::core::config::MemorySettings;
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::trace::TraceSink;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::{MemoryKind, NewMemory, short_id};
use crate::core::types::tools::{
    FORGET_MEMORY, REMEMBER, RiskClass, ToolDefinition, ToolError, ToolOutput,
};
use crate::core::types::trace::TraceEvent;

/// What remember checks before it writes.
#[derive(Debug, Clone)]
pub struct MemoryRules {
    /// Longest memory, in characters.
    pub max_chars: usize,
    /// Lowercase fragments that make remember refuse the text.
    pub refuse_terms: Vec<String>,
    /// Lowercase fragments that make remember seal the text.
    pub sensitive_terms: Vec<String>,
    /// Hours from UTC of the day a due date is read in.
    pub utc_offset_hours: i32,
}

impl MemoryRules {
    /// The rules from the memory section and the prompt offset.
    pub fn new(cfg: &MemorySettings, utc_offset_hours: i32) -> Self {
        let lower = |terms: &[String]| terms.iter().map(|t| t.to_lowercase()).collect();
        Self {
            max_chars: cfg.max_chars,
            refuse_terms: lower(&cfg.refuse_terms),
            sensitive_terms: lower(&cfg.sensitive_terms),
            utc_offset_hours,
        }
    }

    /// Today at the offset.
    fn today(&self) -> NaiveDate {
        FixedOffset::east_opt(self.utc_offset_hours * 3600).map_or_else(
            || Utc::now().date_naive(),
            |offset| Utc::now().with_timezone(&offset).date_naive(),
        )
    }

    /// The memory to write, or why it is refused.
    pub fn check(&self, args: &RememberArgs) -> Result<NewMemory, String> {
        let text = args.text.trim().to_owned();
        if text.is_empty() {
            return Err("the text is empty".into());
        }
        if text.chars().count() > self.max_chars {
            return Err(format!(
                "the text is longer than {} characters; keep one short line",
                self.max_chars
            ));
        }
        let lower = text.to_lowercase();
        if let Some(term) = self
            .refuse_terms
            .iter()
            .find(|t| lower.contains(t.as_str()))
        {
            return Err(format!("text about {term} is never stored"));
        }
        let due = match (args.kind, args.due.as_deref().map(str::trim)) {
            (MemoryKind::Task, None | Some("")) => {
                return Err("a task needs a due date, YYYY-MM-DD".into());
            }
            (MemoryKind::Task, Some(day)) => {
                let due = NaiveDate::parse_from_str(day, "%Y-%m-%d")
                    .map_err(|_| format!("due {day} is not a YYYY-MM-DD date"))?;
                if due < self.today() {
                    return Err(format!("due {day} has passed"));
                }
                Some(due)
            }
            _ => None,
        };
        let sensitive = args.sensitive
            || self
                .sensitive_terms
                .iter()
                .any(|t| lower.contains(t.as_str()));
        Ok(NewMemory {
            kind: args.kind,
            text,
            due,
            sensitive,
        })
    }
}

/// Arguments of remember.
#[derive(Debug, Deserialize)]
pub struct RememberArgs {
    /// Kind of memory.
    pub kind: MemoryKind,
    /// The line to keep.
    pub text: String,
    /// Due date of a task.
    #[serde(default)]
    pub due: Option<String>,
    /// Seal it at rest.
    #[serde(default)]
    pub sensitive: bool,
}

/// Arguments of forget_memory.
#[derive(Debug, Deserialize)]
struct ForgetArgs {
    id: String,
}

/// A refusal handed back to the model as the tool result.
fn refused(reason: &str) -> ToolOutput {
    ToolOutput {
        content: format!("Not remembered: {reason}."),
        data: None,
        sources: Vec::new(),
    }
}

/// Writes one memory for the caller.
pub struct RememberTool {
    store: Arc<dyn MemoryStore>,
    trace: Arc<dyn TraceSink>,
    rules: MemoryRules,
}

impl RememberTool {
    /// Writes to store under rules and reports each write on trace.
    pub fn new(store: Arc<dyn MemoryStore>, trace: Arc<dyn TraceSink>, rules: MemoryRules) -> Self {
        Self {
            store,
            trace,
            rules,
        }
    }
}

#[async_trait]
impl Tool for RememberTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: REMEMBER.to_owned(),
            description: "Keep one short line the student stated about themselves for later \
                          conversations: a fact (major, year, campus, clubs), a preference (how \
                          they want answers), or a task with its due date. Only what they said, \
                          never a guess."
                .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "enum": ["fact", "preference", "task"]},
                    "text": {
                        "type": "string",
                        "description": format!("One line, at most {} characters.", self.rules.max_chars)
                    },
                    "due": {"type": "string", "description": "YYYY-MM-DD. Required for a task."},
                    "sensitive": {"type": "boolean", "description": "Seal it at rest."}
                },
                "required": ["kind", "text"]
            }),
            risk: RiskClass::PrepareWrite,
            private: true,
            sequential: true,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let args: RememberArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let memory = match self.rules.check(&args) {
            Ok(memory) => memory,
            Err(reason) => return Ok(refused(&reason)),
        };
        let stored = self
            .store
            .remember(ctx, &memory)
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        self.trace.emit(
            ctx,
            TraceEvent::MemoryWritten {
                memory: stored.kind,
            },
        );
        Ok(ToolOutput {
            content: format!("Remembered as {}.", stored.short_id()),
            data: None,
            sources: Vec::new(),
        })
    }
}

/// Removes one memory of the caller by its short id.
pub struct ForgetMemoryTool {
    store: Arc<dyn MemoryStore>,
    trace: Arc<dyn TraceSink>,
}

impl ForgetMemoryTool {
    /// Removes from store and reports each removal on trace.
    pub fn new(store: Arc<dyn MemoryStore>, trace: Arc<dyn TraceSink>) -> Self {
        Self { store, trace }
    }
}

#[async_trait]
impl Tool for ForgetMemoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: FORGET_MEMORY.to_owned(),
            description: "Remove one memory the student asks you to forget, by the short id in \
                          brackets in the memory section."
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
        let args: ForgetArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let wanted = args.id.trim().trim_matches(['[', ']']).to_lowercase();
        let held = self
            .store
            .recall(ctx)
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        let found = held.iter().find(|m| short_id(m.id) == wanted);
        let removed = match found {
            Some(memory) => self
                .store
                .forget(ctx, memory.id)
                .await
                .map_err(|e| ToolError::Failed(e.to_string()))?,
            None => false,
        };
        if removed {
            self.trace.emit(ctx, TraceEvent::MemoryForgotten);
        }
        let content = if removed {
            format!("Forgot {wanted}.")
        } else {
            format!("No memory {wanted}; nothing was removed.")
        };
        Ok(ToolOutput {
            content,
            data: None,
            sources: Vec::new(),
        })
    }
}

/// Both memory tools over one store.
pub fn tools(
    store: &Arc<dyn MemoryStore>,
    trace: &Arc<dyn TraceSink>,
    rules: MemoryRules,
) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(RememberTool::new(
            Arc::clone(store),
            Arc::clone(trace),
            rules,
        )),
        Arc::new(ForgetMemoryTool::new(Arc::clone(store), Arc::clone(trace))),
    ]
}
