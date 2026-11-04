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
    pub discord_client_id: Option<String>,
    /// Discord application secret. SPARKY_WEB__DISCORD_CLIENT_SECRET.
    pub discord_client_secret: Option<SecretString>,
    /// Base64 of 32 random bytes that seal the session cookie. SPARKY_WEB__SESSION_KEY.
    pub session_key: Option<SecretString>,
    /// Public https URL of this engine, for the Discord redirect. SPARKY_WEB__PUBLIC_URL.
    pub public_url: Option<String>,
    /// Origin of the site that serves the page. SPARKY_WEB__SITE_URL.
    pub site_url: Option<String>,
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
