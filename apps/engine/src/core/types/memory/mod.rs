//! MemoryKind, Memory, NewMemory: what Sparky remembers about one student.

pub mod listing;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What kind of memory this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    /// Something the student stated about themselves: major, year, campus, clubs.
    Fact,
    /// How the student wants answers.
    Preference,
    /// Something due on a date. Expires after it.
    Task,
}

impl MemoryKind {
    /// Database column value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Task => "task",
        }
    }

    /// Parses a database column value.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "fact" => Some(Self::Fact),
            "preference" => Some(Self::Preference),
            "task" => Some(Self::Task),
            _ => None,
        }
    }

    /// The kind of a platform memory: episodic and semantic are facts, profile is a preference.
    pub fn from_platform(s: &str) -> Option<Self> {
        match s {
            "episodic" | "semantic" | "fact" => Some(Self::Fact),
            "profile" | "preference" => Some(Self::Preference),
            "task" => Some(Self::Task),
            _ => None,
        }
    }
}

/// A stored memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    /// Row id. The first 8 hex digits are the short id the prompt and the tools use.
    pub id: Uuid,
    /// Kind.
    pub kind: MemoryKind,
    /// The remembered line.
    pub content: String,
    /// Sealed at rest.
    pub sensitive: bool,
    /// When it was written.
    pub created_at: DateTime<Utc>,
    /// When it stops being recalled; for a task, the end of its due date.
    pub expires_at: Option<DateTime<Utc>>,
}

impl Memory {
    /// The short id: the first 8 hex digits of the row id.
    pub fn short_id(&self) -> String {
        short_id(self.id)
    }
}

/// The first 8 hex digits of id.
pub fn short_id(id: Uuid) -> String {
    id.simple().to_string().chars().take(8).collect()
}

/// One memory to write for the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMemory {
    /// Kind.
    pub kind: MemoryKind,
    /// The line to remember.
    pub text: String,
    /// For a task, the day it is due.
    pub due: Option<NaiveDate>,
    /// Seal it at rest.
    pub sensitive: bool,
}
