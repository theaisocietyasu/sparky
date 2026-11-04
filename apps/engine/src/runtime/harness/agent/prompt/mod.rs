//! The configurable wording assembly writes around the prompt sections.

pub mod assemble;
pub mod capability;

use crate::core::types::agent::assemble::Templates;

/// The configurable wording assembly writes around the sections.
#[derive(Debug, Clone)]
pub struct PromptText {
    /// Line naming the user, with {user} and {roles}.
    pub role_line: String,
    /// Line naming a user who holds no roles, with {user}.
    pub role_line_no_roles: String,
    /// Heading above recalled memories.
    pub memory_header: String,
    /// Heading above what the model may do.
    pub capabilities_header: String,
    /// Heading above the message of ours a reply answers.
    pub reply_header: String,
    /// Line closing a tool result cut to fit the prompt, with {chars}.
    pub result_cut_line: String,
    /// Line naming the current date, with {date}.
    pub date_line: String,
    /// Line added to a call that is offered no tools.
    pub answer_only_line: String,
    /// Line sent back when a tool failed and the sandbox was not tried. Empty turns it off.
    pub sandbox_retry_line: String,
    /// Line naming a file the user attached and its text.
    pub upload_line: String,
    /// Line naming a file no text could be pulled from.
    pub upload_raw_line: String,
    /// Line naming a file the user attached that could not be opened, with {reason}.
    pub upload_failed_line: String,
    /// What to remember and what never to store, for a private turn with the memory tools.
    pub memory_rules: String,
    /// How to find events and clubs from memories, for a private turn with search_live.
    pub discovery_rules: String,
    /// Hours from UTC the date is rendered in.
    pub utc_offset_hours: i32,
}

impl Default for PromptText {
    fn default() -> Self {
        Self::from(&crate::core::config::Prompt::default())
    }
}

impl From<&crate::core::config::Prompt> for PromptText {
    fn from(cfg: &crate::core::config::Prompt) -> Self {
        Self {
            role_line: cfg.role_line.clone(),
            role_line_no_roles: cfg.role_line_no_roles.clone(),
            memory_header: cfg.memory_header.clone(),
            capabilities_header: cfg.capabilities_header.clone(),
            reply_header: cfg.reply_header.clone(),
            result_cut_line: cfg.result_cut_line.clone(),
            date_line: cfg.date_line.clone(),
            answer_only_line: cfg.answer_only_line.clone(),
            sandbox_retry_line: cfg.sandbox_retry_line.clone(),
            upload_line: cfg.upload_line.clone(),
            upload_raw_line: cfg.upload_raw_line.clone(),
            upload_failed_line: cfg.upload_failed_line.clone(),
            memory_rules: cfg.memory_rules.clone(),
            discovery_rules: cfg.discovery_rules.clone(),
            utc_offset_hours: cfg.utc_offset_hours,
        }
    }
}

impl PromptText {
    /// Today's date in the configured offset, as the date line renders it.
    pub fn today(&self) -> String {
        const FORMAT: &str = "%A %-d %B %Y";
        let now = chrono::Utc::now();
        let offset = self
            .utc_offset_hours
            .checked_mul(3600)
            .and_then(chrono::FixedOffset::east_opt);
        if let Some(offset) = offset {
            return now.with_timezone(&offset).format(FORMAT).to_string();
        }
        tracing::warn!(
            utc_offset_hours = self.utc_offset_hours,
            "utc offset out of range; the date is rendered in UTC"
        );
        now.format(FORMAT).to_string()
    }

    /// Borrowed view for one assembly pass.
    pub(super) fn templates(&self) -> Templates<'_> {
        Templates {
            role_line: &self.role_line,
            role_line_no_roles: &self.role_line_no_roles,
            memory_header: &self.memory_header,
            capabilities_header: &self.capabilities_header,
            date_line: &self.date_line,
            reply_header: &self.reply_header,
            result_cut_line: &self.result_cut_line,
            memory_rules: &self.memory_rules,
            discovery_rules: &self.discovery_rules,
        }
    }
}
