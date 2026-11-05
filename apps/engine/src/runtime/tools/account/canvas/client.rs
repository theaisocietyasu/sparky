//! Calendar feed client over reqwest: one GET, no redirects, body capped.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use reqwest::redirect::Policy;
use secrecy::{ExposeSecret, SecretString};

use crate::core::traits::tools::canvas::CanvasCalendar;
use crate::core::types::tools::canvas::CanvasError;
use crate::runtime::tools::http;

/// Reads calendar feeds over HTTPS.
pub struct FeedClient {
    http: Client,
    max_bytes: usize,
}

impl FeedClient {
    /// Builds the client with a request budget, a body cap and the User-Agent it sends.
    pub fn new(timeout: Duration, max_bytes: usize, user_agent: &str) -> Result<Self, CanvasError> {
        let http = Client::builder()
            .timeout(timeout)
            .user_agent(user_agent)
            .redirect(Policy::none())
            .build()
            .map_err(|e| CanvasError::Unreachable(http::failure(&e).to_owned()))?;
        Ok(Self { http, max_bytes })
    }
}

#[async_trait]
impl CanvasCalendar for FeedClient {
    async fn fetch(&self, link: &SecretString) -> Result<String, CanvasError> {
        let mut response = self
            .http
            .get(link.expose_secret())
            .send()
            .await
            .map_err(|e| CanvasError::Unreachable(http::failure(&e).to_owned()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(CanvasError::Refused(status.as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|n| usize::try_from(n).map_or(true, |n| n > self.max_bytes))
        {
            return Err(CanvasError::TooLarge(self.max_bytes));
        }
        let mut body: Vec<u8> = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| CanvasError::Unreachable(http::failure(&e).to_owned()))?
        {
            if body.len() + chunk.len() > self.max_bytes {
                return Err(CanvasError::TooLarge(self.max_bytes));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(String::from_utf8_lossy(&body).into_owned())
    }
}
