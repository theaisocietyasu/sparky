//! Wire shapes of the per-user account link routes.

use serde::{Deserialize, Serialize};

/// The caller a login belongs to, sent by the bot. The web sends an empty body.
#[derive(Deserialize, Default)]
pub struct AuthorizeRequest {
    /// The caller who started the login. Ignored for a web session.
    #[serde(default)]
    pub user: String,
}

/// Whether a web caller has an account linked for a provider.
#[derive(Serialize)]
pub struct LinkStatus {
    /// True when a grant is stored.
    pub linked: bool,
}

/// The login link the caller opens.
#[derive(Serialize)]
pub struct AuthorizeResponse {
    /// The link that starts the provider login.
    pub url: String,
}

/// Whether a disconnect removed a stored grant.
#[derive(Serialize)]
pub struct DisconnectResponse {
    /// True when a grant was there and is now gone.
    pub removed: bool,
}
