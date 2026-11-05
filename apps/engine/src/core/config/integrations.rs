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
    /// Local weekday the openings check runs, 1 for Monday to 7 for Sunday.
    pub opportunity_weekday: u32,
    /// Local hour from which the openings check runs.
    pub opportunity_hour: u32,
    /// Instruction for the openings check. {nothing} and {already} are replaced.
    pub opportunity_prompt: String,
    /// Hours after a big item was due that the check in may still go out.
    pub after_due_hours: i64,
    /// Lines of the after due check in. {title} is replaced.
    pub after_due_lines: Vec<String>,
    /// Words in a title that make an item an exam.
    pub exam_words: Vec<String>,
    /// Exams in the next 7 days that make an exam week. 0 turns it off.
    pub exam_week_min: usize,
    /// Lines of the exam week nudge. {count} and {list} are replaced.
    pub exam_week_lines: Vec<String>,
    /// Local hour the Friday wins go out from.
    pub wins_hour: u32,
    /// Items done this week that make wins worth sending. 0 turns it off.
    pub wins_min: usize,
    /// Lines of the Friday wins. {count} and {list} are replaced.
    pub wins_lines: Vec<String>,
    /// Days after the first chat that tips go out.
    pub tips_days: i64,
    /// Local hour tips go out from.
    pub tip_hour: u32,
    /// The tips, in order, one a day.
    pub tips: Vec<String>,
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
            opportunity_weekday: 3,
            opportunity_hour: 12,
            opportunity_prompt: OPPORTUNITY_PROMPT.into(),
            after_due_hours: 8,
            after_due_lines: lines(AFTER_DUE_LINES),
            exam_words: lines(&["exam", "midterm", "final", "quiz", "test"]),
            exam_week_min: 2,
            exam_week_lines: lines(EXAM_WEEK_LINES),
            wins_hour: 15,
            wins_min: 2,
            wins_lines: lines(WINS_LINES),
            tips_days: 14,
            tip_hour: 11,
            tips: lines(TIPS),
        }
    }
}

/// Owned copies of default lines.
fn lines(all: &[&str]) -> Vec<String> {
    all.iter().map(|l| (*l).to_owned()).collect()
}

/// Default lines of the after due check in.
pub const AFTER_DUE_LINES: &[&str] = &[
    "How did {title} go? Whatever happened, it is done now. Want to jot down anything for next time?",
    "{title} is behind you. How do you feel it went?",
    "You made it through {title}. Anything you want to carry into the next one?",
];

/// Default lines of the exam week nudge.
pub const EXAM_WEEK_LINES: &[&str] = &[
    "Exam week: {count} coming up.{list}\nI will keep the other check-ins quiet. Want me to block out study time for each?",
    "Big week ahead, {count} exams:{list}\nI will stay out of your way. Want a study plan, one day at a time?",
    "{count} exams in the next few days:{list}\nWant me to set a reminder to start on the first one?",
];

/// Default lines of the Friday wins.
pub const WINS_LINES: &[&str] = &[
    "Friday check: you got through {count} things this week.{list}\nNice work. Take the evening off.",
    "Week wrapped, {count} done:{list}\nThat is real progress. Enjoy the weekend.",
    "Look at this week: {count} off your plate.{list}\nYou earned a break.",
];

/// Default tips for a student's first weeks.
pub const TIPS: &[&str] = &[
    "Tip: send me a syllabus PDF and I will pull out every due date for you.",
    "Tip: link your Canvas calendar in Settings and I will keep an eye on your deadlines.",
    "Tip: ask me to remind you of anything, like: remind me Sunday at 7 to plan my week.",
    "Tip: tell me what you are into and I will point you to events and clubs that fit.",
    "Tip: tap Thread under any of my messages to dig into it without cluttering this chat.",
];

/// Default instruction for the openings check.
pub const OPPORTUNITY_PROMPT: &str = "Background check, not a question from the student. Search for internships, research positions, hackathons and other openings that are open now and fit what you remember about this student: their major, year, interests and campus. Prefer ASU and Arizona openings and ones with a deadline in the next 30 days. If one clearly fits, write a short friendly message to them, two sentences at most, naming it, the deadline and the link, and why it fits them. You already told them: {already}. Do not repeat those. If nothing clearly fits, answer exactly {nothing}.";

/// Default instruction for the event match.
pub const EVENT_PROMPT: &str = "Background check, not a question from the student. Search ASU events in the next 7 days that fit what you remember about this student: their major, interests, clubs and campus. If one clearly fits, write a short friendly message to them, two sentences at most, naming the event, when and where, with its link, and why it fits them. If nothing clearly fits, or you know too little about them, answer exactly {nothing}.";

/// Default instruction for the academic calendar check.
pub const CALENDAR_PROMPT: &str = "Background check, not a question from the student. Search the ASU academic calendar for dates in the next 14 days that matter to a student: the last day to add or drop, the withdrawal deadline, registration opening, tuition due, financial aid and scholarship deadlines, and days with no classes. Use what you remember about them, such as their year and campus. If one is coming up, write a short friendly message, two sentences at most, with the date and the link. You already told them: {already}. Do not repeat those. If nothing new is coming up, answer exactly {nothing}.";

/// Default instruction for the late study check.
pub const STUDY_PROMPT: &str = "Background check, not a question from the student. They have {item} due {due}. Check ASU library hours tonight and the study rooms. If the main libraries near their campus close before midnight tonight, write a short friendly message, two sentences at most, naming a place that stays open later and its hours, with a link. If they are open late anyway, or you cannot tell, answer exactly {nothing}.";

/// Searching the student's earlier private chats. Needs store.backend sqlite.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ChatSearch {
    /// Whether the search_chats tool is registered at boot.
    pub enabled: bool,
    /// Most messages one search returns.
    pub max_hits: usize,
}

impl Default for ChatSearch {
    fn default() -> Self {
        Self {
            enabled: true,
            max_hits: 6,
        }
    }
}

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
    /// Whether each page read is sent to the platform to save and refresh.
    pub save_pages: bool,
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
            save_pages: true,
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
