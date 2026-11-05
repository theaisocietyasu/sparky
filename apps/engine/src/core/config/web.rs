//! Discord sign-in for the web page: switches, cookie names, and the keys from the environment.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::core::config::ConfigError;

/// Length of the session key in bytes.
pub const WEB_SESSION_KEY_BYTES: usize = 32;

/// The web sign-in. Off by default; the keys and URLs come from .env.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Web {
    /// Whether the sign-in routes are served and the chat routes accept a session.
    pub enabled: bool,
    /// How long a session lasts.
    pub session_hours: u64,
    /// Name of the session cookie. The sign-in state cookie adds _state.
    pub cookie_name: String,
    /// Base of the Discord API, no trailing slash.
    pub discord_api: String,
    /// The Discord page a student signs in on.
    pub discord_authorize_url: String,
    /// Budget for one Discord request.
    pub timeout_secs: u64,
    /// Discord application id. SPARKY_WEB__DISCORD_CLIENT_ID.
    #[serde(deserialize_with = "id_text")]
    pub discord_client_id: Option<String>,
    /// Discord application secret. SPARKY_WEB__DISCORD_CLIENT_SECRET.
    pub discord_client_secret: Option<SecretString>,
    /// Base64 of 32 random bytes that seal the session cookie. SPARKY_WEB__SESSION_KEY.
    pub session_key: Option<SecretString>,
    /// Public https URL of this engine, for the Discord redirect. SPARKY_WEB__PUBLIC_URL.
    pub public_url: Option<String>,
    /// Origin of the site that serves the page. SPARKY_WEB__SITE_URL.
    pub site_url: Option<String>,
    /// Conversations the web sidebar lists, newest first.
    pub conversation_list: usize,
    /// Messages one web conversation shows.
    pub conversation_turns: usize,
    /// Characters of the first question a conversation title keeps.
    pub title_chars: usize,
}

impl Default for Web {
    fn default() -> Self {
        Self {
            enabled: false,
            session_hours: 12,
            cookie_name: "sparky_session".into(),
            discord_api: "https://discord.com/api/v10".into(),
            discord_authorize_url: "https://discord.com/oauth2/authorize".into(),
            timeout_secs: 10,
            discord_client_id: None,
            discord_client_secret: None,
            session_key: None,
            public_url: None,
            site_url: None,
            conversation_list: 50,
            conversation_turns: 200,
            title_chars: 60,
        }
    }
}

impl Web {
    /// The decoded session key.
    pub fn key(&self) -> Result<[u8; WEB_SESSION_KEY_BYTES], ConfigError> {
        let invalid = |m: &str| ConfigError::Invalid(m.to_owned());
        let Some(key) = self.session_key.as_ref() else {
            return Err(invalid(
                "set SPARKY_WEB__SESSION_KEY, base64 of 32 random bytes (openssl rand -base64 32)",
            ));
        };
        let decoded = STANDARD
            .decode(key.expose_secret().trim())
            .map_err(|_| invalid("SPARKY_WEB__SESSION_KEY is not base64"))?;
        decoded
            .try_into()
            .map_err(|_| invalid("SPARKY_WEB__SESSION_KEY must decode to 32 bytes"))
    }

    /// The public URL without a trailing slash.
    pub fn public(&self) -> &str {
        self.public_url
            .as_deref()
            .unwrap_or_default()
            .trim()
            .trim_end_matches('/')
    }

    /// The site origin without a trailing slash.
    pub fn site(&self) -> &str {
        self.site_url
            .as_deref()
            .unwrap_or_default()
            .trim()
            .trim_end_matches('/')
    }
}

/// An application id as text, read from a string or from the number an env var parses to.
fn id_text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Id {
        /// Quoted, or from the TOML file.
        Text(String),
        /// An unquoted env value of digits.
        Number(u64),
    }
    Ok(Option::<Id>::deserialize(deserializer)?.map(|id| match id {
        Id::Text(text) => text,
        Id::Number(number) => number.to_string(),
    }))
}

/// Default first message of a new student's home chat.
pub const HOME_WELCOME: &str = "Hey, I'm Sparky, your ASU sidekick. I keep an eye on what's due, find things on campus \
     worth your time, and check in before anything sneaks up on you.\n\nI'd love to get to know \
     you first so I can actually help. What are you studying, and what year are you in?";

/// Default instructions of the choice between home and a new thread.
pub const HOME_ROUTE_PROMPT: &str = "You sort one message a student sent in their home chat with Sparky, an ASU assistant. \
     Answer THREAD when it opens a new topic worth its own conversation: a question about a class, \
     a deadline, a plan, a campus service, a job, anything that needs looking up or some back and \
     forth. Answer HOME when it is a greeting, small talk, thanks, an answer to what Sparky just \
     asked (such as their major or interests), or a short reply to Sparky's last message. Answer \
     with the one word THREAD or HOME.";

/// The home chat: the first message a new student gets, and new topics moved into threads.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Home {
    /// Sparky's first message in an empty home chat. Empty sends none.
    pub welcome: String,
    /// Move a message that opens a new topic in home into a thread of its own.
    pub threads: bool,
    /// Instructions of the choice between home and a new thread.
    pub route_prompt: String,
    /// Completion budget of that choice.
    pub route_max_tokens: u32,
    /// Longest the choice may take before the message stays in home.
    pub route_timeout_secs: u64,
}

impl Default for Home {
    fn default() -> Self {
        Self {
            welcome: HOME_WELCOME.into(),
            threads: true,
            route_prompt: HOME_ROUTE_PROMPT.into(),
            route_max_tokens: 8,
            route_timeout_secs: 8,
        }
    }
}
