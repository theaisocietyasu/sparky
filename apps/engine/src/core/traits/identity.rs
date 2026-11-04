//! Discord sign-in for the web page.

use async_trait::async_trait;

use crate::core::types::http::web::{DiscordUser, IdentityError};

/// Turns a Discord authorization code into the account and its guilds.
#[async_trait]
pub trait DiscordIdentity: Send + Sync {
    /// The account behind code, which Discord issued for redirect_uri.
    async fn sign_in(&self, code: &str, redirect_uri: &str) -> Result<DiscordUser, IdentityError>;
}
