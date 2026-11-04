//! Wire shapes of the web sign-in: the session a cookie holds, the Discord account behind it.

use serde::{Deserialize, Serialize};

/// Discord permission bit that lets a member manage the server.
pub const MANAGE_GUILD: u64 = 1 << 5;

/// Discord permission bit that grants every permission.
pub const ADMINISTRATOR: u64 = 1 << 3;

/// What a sealed session cookie holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// Discord user id.
    pub user_id: String,
    /// Discord user name, for the page.
    pub name: String,
    /// Role names the policy reads, as the bot sends them.
    pub roles: Vec<String>,
    /// Unix seconds after which the session is refused.
    pub expires_at: i64,
}

/// The Discord account a sign-in returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscordUser {
    /// Discord user id.
    pub id: String,
    /// Discord user name.
    pub name: String,
    /// Every guild the user is in, with their permission bits there.
    pub guilds: Vec<GuildAccess>,
}

/// One guild of a signed-in user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildAccess {
    /// Guild id.
    pub id: String,
    /// The permission bits of the user in that guild.
    pub permissions: u64,
}

/// A Discord sign-in that could not be completed.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    /// Discord could not be reached.
    #[error("Discord could not be reached: {0}")]
    Unreachable(String),
    /// Discord refused the code or the token.
    #[error("Discord refused the sign-in ({0})")]
    Refused(u16),
    /// Discord answered with an unexpected shape.
    #[error("Discord sent an unexpected answer: {0}")]
    Malformed(String),
}

/// The answer to GET /web/me.
#[derive(Debug, Serialize)]
pub struct MeResponse {
    /// Discord user id.
    pub user_id: String,
    /// Discord user name.
    pub name: String,
    /// Role names the policy reads.
    pub roles: Vec<String>,
    /// Unix seconds when the session ends.
    pub expires_at: i64,
}
