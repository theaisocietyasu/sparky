//! Nudges: notifications Sparky sends on its own, and what a student set about them.

use serde::{Deserialize, Serialize};

/// What made a nudge. Each kind can be switched off by the student.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NudgeKind {
    /// What is due today and this week, on a weekday morning.
    Brief,
    /// Something due within a day.
    DueSoon,
    /// A big assignment or exam a few days out.
    StartEarly,
    /// An assignment that was not there before.
    NewItem,
    /// An event that fits the student.
    Event,
    /// A check-in on something the student said they would do.
    FollowUp,
    /// An academic calendar date coming up.
    Calendar,
    /// Where to study late before a deadline.
    Study,
    /// The week ahead, on Sunday evening.
    WeekPlan,
}

impl NudgeKind {
    /// Every kind, in the order Settings lists them.
    pub const ALL: [Self; 9] = [
        Self::Brief,
        Self::DueSoon,
        Self::StartEarly,
        Self::NewItem,
        Self::Event,
        Self::FollowUp,
        Self::Calendar,
        Self::Study,
        Self::WeekPlan,
    ];

    /// Database and wire value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Brief => "brief",
            Self::DueSoon => "due_soon",
            Self::StartEarly => "start_early",
            Self::NewItem => "new_item",
            Self::Event => "event",
            Self::FollowUp => "follow_up",
            Self::Calendar => "calendar",
            Self::Study => "study",
            Self::WeekPlan => "week_plan",
        }
    }

    /// Parses a database or wire value.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// What a student set about nudges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NudgePrefs {
    /// Local hour quiet time starts, 0 to 23.
    pub quiet_start: u8,
    /// Local hour quiet time ends, 0 to 23.
    pub quiet_end: u8,
    /// Kinds the student switched off.
    pub off: Vec<NudgeKind>,
}

impl NudgePrefs {
    /// Whether local hour falls in quiet time. Equal ends mean no quiet time.
    pub fn quiet_at(&self, hour: u8) -> bool {
        let (start, end) = (self.quiet_start, self.quiet_end);
        match start.cmp(&end) {
            std::cmp::Ordering::Equal => false,
            std::cmp::Ordering::Less => (start..end).contains(&hour),
            std::cmp::Ordering::Greater => hour >= start || hour < end,
        }
    }

    /// Whether the student lets this kind through.
    pub fn allows(&self, kind: NudgeKind) -> bool {
        !self.off.contains(&kind)
    }
}

/// One nudge ready to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nudge {
    /// What made it.
    pub kind: NudgeKind,
    /// Keys of what it tells, each sent at most once.
    pub keys: Vec<String>,
    /// What it says.
    pub text: String,
}

/// One student a nudge can go to: the tenant they last talked in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// Tenant of their latest private conversation.
    pub tenant: String,
    /// Their user id.
    pub user: String,
}

/// A student's answer on one nudge.
#[derive(Debug, Deserialize)]
pub struct NudgeFeedback {
    /// Whether it was useful.
    pub helpful: bool,
}
