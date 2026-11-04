//! Discord sign-in over the OAuth2 API: one code exchange, then the account and its guilds.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use reqwest::redirect::Policy;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::core::traits::identity::DiscordIdentity;
use crate::core::types::http::web::{DiscordUser, GuildAccess, IdentityError};

/// The Discord OAuth2 client of one application.
pub struct DiscordOAuth {
    http: Client,
    api: String,
    client_id: String,
    client_secret: SecretString,
}

/// The token answer of a code exchange.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// The account a token reads.
#[derive(Deserialize)]
struct RawUser {
    id: String,
    username: String,
    #[serde(default)]
    global_name: Option<String>,
}

/// One guild of the account, with its permission bits as a decimal string.
#[derive(Deserialize)]
struct RawGuild {
    id: String,
    #[serde(default)]
    permissions: String,
}

/// The short reason a request failed, without the URL or body.
fn failure(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "timed out".to_owned()
    } else if error.is_connect() {
        "could not connect".to_owned()
    } else {
        "request failed".to_owned()
    }
}

impl DiscordOAuth {
    /// Builds the client over the API base, the application id and secret, and a request budget.
    pub fn new(
        api: &str,
        client_id: &str,
        client_secret: SecretString,
        timeout: Duration,
    ) -> Result<Self, IdentityError> {
        let http = Client::builder()
            .timeout(timeout)
            .redirect(Policy::none())
            .build()
            .map_err(|e| IdentityError::Unreachable(failure(&e)))?;
        Ok(Self {
            http,
            api: api.trim_end_matches('/').to_owned(),
            client_id: client_id.to_owned(),
            client_secret,
        })
    }

    /// A GET with the access token, read as T.
    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        token: &str,
    ) -> Result<T, IdentityError> {
        let response = self
            .http
            .get(format!("{}{path}", self.api))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| IdentityError::Unreachable(failure(&e)))?;
        if !response.status().is_success() {
            return Err(IdentityError::Refused(response.status().as_u16()));
        }
        response
            .json()
            .await
            .map_err(|e| IdentityError::Malformed(failure(&e)))
    }
}

#[async_trait]
impl DiscordIdentity for DiscordOAuth {
    async fn sign_in(&self, code: &str, redirect_uri: &str) -> Result<DiscordUser, IdentityError> {
        let form = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &self.client_id)
            .append_pair("client_secret", self.client_secret.expose_secret())
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", code)
            .append_pair("redirect_uri", redirect_uri)
            .finish();
        let response = self
            .http
            .post(format!("{}/oauth2/token", self.api))
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form)
            .send()
            .await
            .map_err(|e| IdentityError::Unreachable(failure(&e)))?;
        if !response.status().is_success() {
            return Err(IdentityError::Refused(response.status().as_u16()));
        }
        let token: TokenResponse = response
            .json()
            .await
            .map_err(|e| IdentityError::Malformed(failure(&e)))?;
        let user: RawUser = self.get("/users/@me", &token.access_token).await?;
        let guilds: Vec<RawGuild> = self.get("/users/@me/guilds", &token.access_token).await?;
        Ok(DiscordUser {
            id: user.id,
            name: user.global_name.unwrap_or(user.username),
            guilds: guilds
                .into_iter()
                .map(|g| GuildAccess {
                    id: g.id,
                    permissions: g.permissions.parse().unwrap_or(0),
                })
                .collect(),
        })
    }
}
