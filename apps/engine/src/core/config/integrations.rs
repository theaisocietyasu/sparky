//! Settings for the per-user and public integrations the tools reach.

use serde::Deserialize;

/// Outlook (Microsoft Graph) read-only tools. Off by default; reads the caller's Microsoft account linked on the platform.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Outlook {
    /// Whether the Outlook tools are registered at boot.
    pub enabled: bool,
    /// Base URL of Microsoft Graph, no trailing path.
    pub base_url: String,
    /// Budget for one Graph request.
    pub timeout_secs: u64,
    /// Most rows one list tool returns to the model.
    pub max_items: usize,
}

impl Default for Outlook {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://graph.microsoft.com/v1.0".into(),
            timeout_secs: 30,
            max_items: 15,
        }
    }
}

/// Canvas due dates and events from each student's calendar feed link. Off with store.backend platform.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Canvas {
    /// Whether the canvas_deadlines tool and the feed link routes are on.
    pub enabled: bool,
    /// Hosts a feed link may point at.
    pub hosts: Vec<String>,
    /// Budget for one feed request.
    pub timeout_secs: u64,
    /// Largest feed read, in bytes.
    pub max_bytes: usize,
    /// Days ahead a call covers when the model gives none.
    pub default_days: u32,
    /// Most days ahead one call may cover.
    pub max_days: u32,
    /// Most items one call returns to the model.
    pub max_items: usize,
    /// User-Agent header sent with each feed request.
    pub user_agent: String,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            enabled: true,
            hosts: vec!["canvas.asu.edu".into()],
            timeout_secs: 15,
            max_bytes: 2_000_000,
            default_days: 14,
            max_days: 60,
            max_items: 25,
            user_agent: "SparkyAI/1.0 (+https://github.com/theaisocietyasu/sparky)".into(),
        }
    }
}

/// One dated list of the caller's tasks, Canvas due dates and calendar events.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Upcoming {
    /// Whether the upcoming tool is registered when at least one source is.
    pub enabled: bool,
    /// Days ahead a call covers when the model gives none.
    pub default_days: u32,
    /// Most days ahead one call may cover.
    pub max_days: u32,
    /// Most items one call returns to the model.
    pub max_items: usize,
}

impl Default for Upcoming {
    fn default() -> Self {
        Self {
            enabled: true,
            default_days: 7,
            max_days: 30,
            max_items: 30,
        }
    }
}

/// Reminders the student sets with the remind tool, and the notifications they become.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Reminders {
    /// Whether the remind tools, the notification routes and the reminder loop are on. Needs store.backend sqlite.
    pub enabled: bool,
    /// Seconds between checks for due reminders.
    pub tick_secs: u64,
    /// Most reminders one student keeps at once.
    pub max_per_user: usize,
    /// Longest reminder text, in characters.
    pub max_chars: usize,
    /// Most days ahead a reminder may be set.
    pub max_days_ahead: u32,
    /// Days a notification stays after it is made.
    pub keep_days: u32,
    /// Most notifications the web list or one bot poll returns.
    pub list_limit: usize,
}

impl Default for Reminders {
    fn default() -> Self {
        Self {
            enabled: true,
            tick_secs: 30,
            max_per_user: 50,
            max_chars: 200,
            max_days_ahead: 365,
            keep_days: 30,
            list_limit: 50,
        }
    }
}

/// Google Calendar read-only tool. Off by default; reads the caller's Google account linked on the platform.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Gcal {
    /// Whether the Google Calendar tool is registered at boot.
    pub enabled: bool,
    /// Base URL of the Google Calendar API, no trailing path.
    pub base_url: String,
    /// Budget for one request.
    pub timeout_secs: u64,
    /// Most events one call returns to the model.
    pub max_items: usize,
}

impl Default for Gcal {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "https://www.googleapis.com/calendar/v3".into(),
            timeout_secs: 30,
            max_items: 15,
        }
    }
}

/// Academic paper search over Semantic Scholar. No key; a public read.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Papers {
    /// Whether the paper search tool is registered at boot.
    pub enabled: bool,
    /// Base URL of the Semantic Scholar graph API, no trailing path.
    pub base_url: String,
    /// Budget for one request.
    pub timeout_secs: u64,
    /// Most papers returned to the model.
    pub max_items: usize,
}

impl Default for Papers {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: "https://api.semanticscholar.org/graph/v1".into(),
            timeout_secs: 20,
            max_items: 5,
        }
    }
}

/// Nudges: notifications Sparky sends on its own, from deadlines, memories and live sources.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Nudges {
    /// Whether the nudge loop and its routes are on. Needs reminders on.
    pub enabled: bool,
    /// Seconds between passes over every student.
    pub tick_secs: u64,
    /// Students with a private conversation in this many days get nudges.
    pub active_days: u32,
    /// Most nudges one student gets in a local day.
    pub daily_cap: usize,
    /// Local hour quiet time starts, until a student sets their own.
    pub quiet_start: u8,
    /// Local hour quiet time ends.
    pub quiet_end: u8,
    /// Local hour the weekday brief goes out from.
    pub brief_hour: u32,
    /// Days ahead the brief and the Sunday plan cover.
    pub brief_days: i64,
    /// Local hour the Sunday plan goes out from.
    pub week_plan_hour: u32,
    /// Hours before a deadline the due soon nudge goes out.
    pub due_soon_hours: i64,
    /// Days before a big assignment the start early nudge goes out.
    pub start_early_days: i64,
    /// Words in a title that make an assignment big.
    pub big_work: Vec<String>,
    /// Days ahead a new assignment is looked for.
    pub new_item_days: i64,
    /// Days after a plan is saved that the follow up goes out.
    pub follow_up_days: i64,
    /// A plan older than this many days gets no follow up.
    pub follow_up_max_days: i64,
    /// Local hour from which the daily event match runs.
    pub event_hour: u32,
    /// Local hour from which the late study check runs, the evening before a deadline.
    pub study_hour: u32,
    /// Local weekday the academic calendar check runs, 1 for Monday to 7 for Sunday.
    pub calendar_weekday: u32,
    /// Local hour from which the academic calendar check runs.
    pub calendar_hour: u32,
    /// Seconds one background run may take.
    pub ask_timeout_secs: u64,
    /// Days a sent key is kept so it is not said again.
    pub keep_days: u32,
    /// What the background agent answers when there is nothing worth sending.
    pub nothing: String,
    /// Instruction for the event match. {nothing} is replaced.
    pub event_prompt: String,
    /// Instruction for the academic calendar check. {nothing} and {already} are replaced.
    pub calendar_prompt: String,
    /// Instruction for the late study check. {nothing}, {item} and {due} are replaced.
    pub study_prompt: String,
}

impl Default for Nudges {
    fn default() -> Self {
        Self {
            enabled: true,
            tick_secs: 600,
            active_days: 30,
            daily_cap: 4,
            quiet_start: 22,
            quiet_end: 8,
            brief_hour: 8,
            brief_days: 7,
            week_plan_hour: 18,
            due_soon_hours: 24,
            start_early_days: 3,
            big_work: [
                "exam",
                "midterm",
                "final",
                "project",
                "paper",
                "essay",
                "report",
                "presentation",
            ]
            .map(String::from)
            .to_vec(),
            new_item_days: 21,
            follow_up_days: 3,
            follow_up_max_days: 10,
            event_hour: 12,
            study_hour: 17,
            calendar_weekday: 1,
            calendar_hour: 10,
            ask_timeout_secs: 120,
            keep_days: 120,
            nothing: "NONE".into(),
            event_prompt: EVENT_PROMPT.into(),
            calendar_prompt: CALENDAR_PROMPT.into(),
            study_prompt: STUDY_PROMPT.into(),
        }
    }
}

/// Default instruction for the event match.
pub const EVENT_PROMPT: &str = "Background check, not a question from the student. Search ASU events in the next 7 days that fit what you remember about this student: their major, interests, clubs and campus. If one clearly fits, write a short friendly message to them, two sentences at most, naming the event, when and where, with its link, and why it fits them. If nothing clearly fits, or you know too little about them, answer exactly {nothing}.";

/// Default instruction for the academic calendar check.
pub const CALENDAR_PROMPT: &str = "Background check, not a question from the student. Search the ASU academic calendar for dates in the next 14 days that matter to a student: the last day to add or drop, the withdrawal deadline, registration opening, tuition due, financial aid deadlines, and days with no classes. Use what you remember about them, such as their year and campus. If one is coming up, write a short friendly message, two sentences at most, with the date and the link. You already told them: {already}. Do not repeat those. If nothing new is coming up, answer exactly {nothing}.";

/// Default instruction for the late study check.
pub const STUDY_PROMPT: &str = "Background check, not a question from the student. They have {item} due {due}. Check ASU library hours tonight and the study rooms. If the main libraries near their campus close before midnight tonight, write a short friendly message, two sentences at most, naming a place that stays open later and its hours, with a link. If they are open late anyway, or you cannot tell, answer exactly {nothing}.";

/// Arithmetic and date math the model hands off instead of working out itself.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Calculate {
    /// Whether the calculate tool is registered at boot.
    pub enabled: bool,
}

impl Default for Calculate {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Reading one public web page by URL. Private and local addresses are refused.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ReadPage {
    /// Whether the read_page tool is registered at boot.
    pub enabled: bool,
    /// Budget for one request.
    pub timeout_secs: u64,
    /// Most bytes of a page downloaded.
    pub max_bytes: usize,
    /// Most characters of page text returned to the model.
    pub max_chars: usize,
    /// Redirects followed, each checked to be public.
    pub max_redirects: usize,
    /// User agent sent with each request.
    pub user_agent: String,
}

impl Default for ReadPage {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_secs: 15,
            max_bytes: 2_000_000,
            max_chars: 6_000,
            max_redirects: 5,
            user_agent: "Mozilla/5.0 (compatible; SparkyAI/2.0; ASU student copilot)".into(),
        }
    }
}

/// Wikipedia lookups over the MediaWiki API. No key; a public read.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Wikipedia {
    /// Whether the Wikipedia tool is registered at boot.
    pub enabled: bool,
    /// The MediaWiki API endpoint.
    pub base_url: String,
    /// Budget for one request.
    pub timeout_secs: u64,
    /// Most characters of the summary returned to the model.
    pub max_chars: usize,
}

impl Default for Wikipedia {
    fn default() -> Self {
        Self {
            enabled: true,
            base_url: "https://en.wikipedia.org/w/api.php".into(),
            timeout_secs: 20,
            max_chars: 1500,
        }
    }
}
