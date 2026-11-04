//! Buttons the bot puts on its own messages, and the custom_id that identifies them.

use serenity::all::{ButtonStyle, CreateActionRow, CreateButton};
use uuid::Uuid;

use crate::core::types::ChatResponse;
use crate::render::card::source_label;

/// Source buttons one reply carries. Discord allows five buttons in a row.
const MAX_SOURCES: usize = 5;

/// Marks a custom_id as belonging to this bot. A component from anywhere else is ignored.
const PREFIX: &str = "sparky";
/// Tag of the button that erases everything remembered.
const FORGET_ALL: &str = "forget_all";
/// Tag of the button that closes the forget prompt.
const KEEP_ALL: &str = "keep_all";
/// Tag of the button that marks an answer helpful.
const HELPFUL: &str = "helpful";
/// Tag of the button that marks an answer not helpful.
const UNHELPFUL: &str = "unhelpful";

/// What a confirmation button does when someone presses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Run the held action and let the agent carry on.
    Approve,
    /// Drop the held action.
    Deny,
}

impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Deny => "deny",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "approve" => Some(Self::Approve),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

/// Everything a pressed button has to tell the bot. Discord caps custom_id at 100 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomId {
    /// Answers a held action.
    Confirm {
        /// What to do.
        action: Action,
        /// The confirmation being answered.
        token: Uuid,
        /// The conversation it belongs to.
        conversation: Uuid,
    },
    /// Erases everything remembered about the user who asked.
    ForgetAll {
        /// Discord id of the user who asked.
        user: u64,
    },
    /// Closes the forget prompt without erasing anything.
    KeepAll {
        /// Discord id of the user who asked.
        user: u64,
    },
    /// Rates one answer.
    Feedback {
        /// Whether the answer helped.
        helpful: bool,
        /// The request the answer came from.
        request: Uuid,
        /// Discord id of the user who asked.
        user: u64,
    },
}

impl CustomId {
    /// Builds an id for one confirmation button.
    pub fn new(action: Action, token: Uuid, conversation: Uuid) -> Self {
        Self::Confirm {
            action,
            token,
            conversation,
        }
    }

    /// Whether presser may press this button. Forget buttons belong to the user who asked.
    pub fn may_press(&self, presser: u64) -> bool {
        match self {
            Self::Confirm { .. } => true,
            Self::ForgetAll { user } | Self::KeepAll { user } | Self::Feedback { user, .. } => {
                *user == presser
            }
        }
    }

    /// Reads an id back, or None when the bot did not mint it.
    pub fn parse(raw: &str) -> Option<Self> {
        let mut parts = raw.split(':');
        if parts.next()? != PREFIX {
            return None;
        }
        let id = match parts.next()? {
            FORGET_ALL => Self::ForgetAll {
                user: parts.next()?.parse().ok()?,
            },
            KEEP_ALL => Self::KeepAll {
                user: parts.next()?.parse().ok()?,
            },
            tag @ (HELPFUL | UNHELPFUL) => Self::Feedback {
                helpful: tag == HELPFUL,
                request: Uuid::parse_str(parts.next()?).ok()?,
                user: parts.next()?.parse().ok()?,
            },
            other => Self::Confirm {
                action: Action::parse(other)?,
                token: Uuid::parse_str(parts.next()?).ok()?,
                conversation: Uuid::parse_str(parts.next()?).ok()?,
            },
        };
        if parts.next().is_some() {
            return None;
        }
        Some(id)
    }
}

impl std::fmt::Display for CustomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Confirm {
                action,
                token,
                conversation,
            } => write!(
                f,
                "{PREFIX}:{}:{}:{}",
                action.as_str(),
                token.simple(),
                conversation.simple()
            ),
            Self::ForgetAll { user } => write!(f, "{PREFIX}:{FORGET_ALL}:{user}"),
            Self::KeepAll { user } => write!(f, "{PREFIX}:{KEEP_ALL}:{user}"),
            Self::Feedback {
                helpful,
                request,
                user,
            } => {
                let tag = if *helpful { HELPFUL } else { UNHELPFUL };
                write!(f, "{PREFIX}:{tag}:{}:{user}", request.simple())
            }
        }
    }
}

/// One button, described without serenity types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonSpec {
    /// Presses back into the bot.
    Press {
        /// What pressing it means.
        id: CustomId,
        /// Text on the button.
        label: &'static str,
        /// Whether it reads as the safe or the consequential answer.
        danger: bool,
    },
    /// Opens a page. Discord sends no press for one.
    Link {
        /// Text on the button.
        label: String,
        /// Where it goes.
        url: String,
    },
}

/// The buttons a reply should carry: the approval answers, then the sources behind the answer.
pub fn rows_for(resp: &ChatResponse) -> Vec<Vec<ButtonSpec>> {
    let mut rows = Vec::new();
    if let Some(confirmation) = &resp.confirmation {
        rows.push(vec![
            ButtonSpec::Press {
                id: CustomId::new(Action::Approve, confirmation.token, resp.conversation_id),
                label: "Yes, do it",
                danger: true,
            },
            ButtonSpec::Press {
                id: CustomId::new(Action::Deny, confirmation.token, resp.conversation_id),
                label: "No",
                danger: false,
            },
        ]);
    }
    let sources: Vec<ButtonSpec> = resp
        .citations
        .iter()
        .filter_map(|c| {
            let url = c.url.as_deref()?.trim();
            let linkable = url.starts_with("https://") || url.starts_with("http://");
            linkable.then(|| ButtonSpec::Link {
                label: source_label(&c.title),
                url: url.to_owned(),
            })
        })
        .take(MAX_SOURCES)
        .collect();
    if !sources.is_empty() {
        rows.push(sources);
    }
    rows
}

/// The rating buttons under a finished answer, bound to the user who asked.
pub fn feedback_row(request: Uuid, user: u64) -> Vec<ButtonSpec> {
    [(true, "Helpful"), (false, "Not helpful")]
        .into_iter()
        .map(|(helpful, label)| ButtonSpec::Press {
            id: CustomId::Feedback {
                helpful,
                request,
                user,
            },
            label,
            danger: false,
        })
        .collect()
}

/// The buttons under the forget everything prompt, bound to the user who asked.
pub fn forget_rows(user: u64) -> Vec<Vec<ButtonSpec>> {
    vec![vec![
        ButtonSpec::Press {
            id: CustomId::ForgetAll { user },
            label: "Forget everything",
            danger: true,
        },
        ButtonSpec::Press {
            id: CustomId::KeepAll { user },
            label: "Cancel",
            danger: false,
        },
    ]]
}

/// Turns described rows into what serenity sends.
pub fn to_action_rows(rows: &[Vec<ButtonSpec>]) -> Vec<CreateActionRow> {
    rows.iter()
        .map(|row| {
            CreateActionRow::Buttons(
                row.iter()
                    .map(|b| match b {
                        ButtonSpec::Press { id, label, danger } => {
                            CreateButton::new(id.to_string())
                                .label(*label)
                                .style(if *danger {
                                    ButtonStyle::Danger
                                } else {
                                    ButtonStyle::Secondary
                                })
                        }
                        ButtonSpec::Link { label, url } => CreateButton::new_link(url).label(label),
                    })
                    .collect(),
            )
        })
        .collect()
}
