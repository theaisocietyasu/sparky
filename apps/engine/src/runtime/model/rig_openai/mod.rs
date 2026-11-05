//! Chat via Rig's OpenAI client at llama-server. Maps Rig types to core types.

mod chat;
pub(crate) mod convert;

use ::rig_core::client::BearerAuth;
use ::rig_core::providers::openai::CompletionsClient;
use secrecy::{ExposeSecret, SecretString};

pub use self::chat::RigChat;

/// Builds a Rig client for one OpenAI-compatible base URL (ending in /v1).
pub fn client(base_url: &str, api_key: &SecretString) -> Result<CompletionsClient, String> {
    CompletionsClient::builder()
        .api_key(BearerAuth::from(api_key.expose_secret().to_owned()))
        .base_url(base_url.trim_end_matches('/'))
        .build()
        .map_err(|e| e.to_string())
}
