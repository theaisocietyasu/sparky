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
