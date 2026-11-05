//! Reminders a student sets and the notifications they turn into when due.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// How often a reminder comes back after it fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    /// Fires once, then is gone.
    Once,
    /// Fires every day at the same time.
    Daily,
    /// Fires every week at the same time.
    Weekly,
}

impl Repeat {
    /// Database column value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
        }
    }

    /// Parses a database column value.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "once" => Some(Self::Once),
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            _ => None,
        }
    }

    /// The first time after now this repeat lands on, starting from at, or None for once.
    pub fn next_after(self, at: DateTime<Utc>, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let step = match self {
            Self::Once => return None,
            Self::Daily => Duration::days(1),
            Self::Weekly => Duration::weeks(1),
        };
        let mut next = at + step;
        while next <= now {
            next += step;
        }
        Some(next)
    }
}

/// A reminder before it is stored.
#[derive(Debug, Clone)]
pub struct NewReminder {
    /// What to remind the student of, one short line.
    pub text: String,
    /// When it first fires.
    pub due_at: DateTime<Utc>,
    /// How often it comes back.
    pub repeat: Repeat,
}

/// A stored reminder.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reminder {
    /// Stable id.
    pub id: String,
    /// What to remind the student of.
    pub text: String,
    /// When it fires next.
    pub due_at: DateTime<Utc>,
    /// How often it comes back.
    pub repeat: Repeat,
}

/// A message for the student, made when a reminder fires.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Notification {
    /// Stable id.
    pub id: String,
    /// What it says.
    pub text: String,
    /// When it was made.
    pub created_at: DateTime<Utc>,
    /// Whether the student has seen it on the web.
    pub read: bool,
    /// What made it: reminder, or the kind of a nudge.
    pub kind: String,
    /// What the student said about it, when they said anything.
    pub helpful: Option<bool>,
}

/// A notification the bot has not yet sent as a direct message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outgoing {
    /// Notification id, sent back once delivered.
    pub id: String,
    /// The Discord user to message.
    pub user_id: String,
    /// What it says.
    pub text: String,
}

/// The web list of notifications, newest first, and how many are unread.
#[derive(Debug, Serialize)]
pub struct NotificationList {
    /// Newest first.
    pub notifications: Vec<Notification>,
    /// How many are unread.
    pub unread: usize,
}

/// The web list of reminders, soonest first.
#[derive(Debug, Serialize)]
pub struct ReminderList {
    /// Soonest first.
    pub reminders: Vec<Reminder>,
}

/// Notifications waiting for the bot.
#[derive(Debug, Serialize, Deserialize)]
pub struct OutgoingList {
    /// Oldest first.
    pub notifications: Vec<Outgoing>,
}

/// The notifications the bot delivered.
#[derive(Debug, Serialize, Deserialize)]
pub struct SentRequest {
    /// Ids from an outgoing list.
    pub ids: Vec<String>,
}
