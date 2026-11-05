//! Typed HTTP client for the engine endpoints the bot calls. Service token auth.

use std::time::Duration;

use opentelemetry::trace::TraceContextExt;
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use futures::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

use crate::core::types::{
    AuthorizeRequest, AuthorizeResponse, ChatRequest, ChatResponse, ConfirmRequest,
    DisconnectResponse, ERROR_BODY_CHARS, EngineError, ForgetRequest, ForgetResponse, MemoryList,
    MemoryRequest, OutgoingList, ResetRequest, ResetResponse, SaveFeedRequest, SaveFeedResponse,
    SentRequest, Update,
};
use crate::engine::sse::{decode, drain_frames, take_complete};

/// HTTP client bound to one engine.
#[derive(Debug, Clone)]
pub struct EngineClient {
    http: reqwest::Client,
    base_url: String,
    token: SecretString,
}

impl EngineClient {
    /// Builds a client for base_url. request_timeout must exceed the engine request budget.
    pub fn new(
        base_url: &str,
        token: SecretString,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, EngineError> {
        let http = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|e| EngineError::Transport(e.to_string()))?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
            token,
        })
    }

    /// Answers a held action. The engine runs it and carries on, or drops it.
    pub async fn confirm(&self, req: &ConfirmRequest) -> Result<ChatResponse, EngineError> {
        self.post("/confirm", req).await
    }

    /// Ends the open conversations of a user in one channel.
    pub async fn reset(&self, req: &ResetRequest) -> Result<ResetResponse, EngineError> {
        self.post("/conversation/reset", req).await
    }

    /// Lists what the engine remembers about a user, numbered from 1.
    pub async fn memories(&self, req: &MemoryRequest) -> Result<MemoryList, EngineError> {
        self.post("/memories/list", req).await
    }

    /// Forgets one memory by number, or every memory without one. 404 for an unknown number.
    pub async fn forget(&self, req: &ForgetRequest) -> Result<ForgetResponse, EngineError> {
        self.post("/memories/forget", req).await
    }

    /// Mints a consent URL for the caller and a provider. 503 when that login is not enabled.
    pub async fn oauth_login(
        &self,
        provider: &str,
        req: &AuthorizeRequest,
    ) -> Result<AuthorizeResponse, EngineError> {
        self.post(&format!("/oauth/{provider}/authorize"), req)
            .await
    }

    /// Disconnects the caller's grant for a provider.
    pub async fn oauth_logout(
        &self,
        provider: &str,
        req: &AuthorizeRequest,
    ) -> Result<DisconnectResponse, EngineError> {
        self.post(&format!("/oauth/{provider}/logout"), req).await
    }

    /// Keeps the caller's Canvas calendar feed link.
    pub async fn canvas_save(
        &self,
        req: &SaveFeedRequest,
    ) -> Result<SaveFeedResponse, EngineError> {
        self.post("/canvas/feed", req).await
    }

    /// Removes the caller's Canvas calendar feed link.
    pub async fn canvas_remove(
        &self,
        req: &AuthorizeRequest,
    ) -> Result<DisconnectResponse, EngineError> {
        self.post("/canvas/feed/remove", req).await
    }

    /// Notifications waiting to be sent as direct messages.
    pub async fn outgoing(&self) -> Result<OutgoingList, EngineError> {
        let response = self
            .http
            .get(format!("{}/notifications/outgoing", self.base_url))
            .bearer_auth(self.token.expose_secret())
            .send()
            .await
            .map_err(|e| EngineError::Transport(e.to_string()))?;
        read(response).await
    }

    /// Tells the engine these notifications are done with.
    pub async fn sent(&self, req: &SentRequest) -> Result<(), EngineError> {
        let response = self
            .request("/notifications/sent", req)
            .send()
            .await
            .map_err(|e| EngineError::Transport(e.to_string()))?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        Err(EngineError::Status {
            status: status.as_u16(),
            body: body.chars().take(ERROR_BODY_CHARS).collect(),
        })
    }

    /// A POST of a JSON body to path, with the service token and the current traceparent.
    fn request<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> reqwest::RequestBuilder {
        let request = self
            .http
            .post(format!("{}{path}", self.base_url))
            .bearer_auth(self.token.expose_secret())
            .json(body);
        match current_traceparent() {
            Some(traceparent) => request.header("traceparent", traceparent),
            None => request,
        }
    }

    /// Posts a JSON body and reads a JSON reply.
    async fn post<B: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, EngineError> {
        let response = self
            .request(path, body)
            .send()
            .await
            .map_err(|e| EngineError::Transport(e.to_string()))?;
        read(response).await
    }

    /// Runs one chat turn, reporting progress on tx. Sends exactly one Answer or Failed last.
    pub async fn chat_stream(&self, req: &ChatRequest, tx: UnboundedSender<Update>) {
        // A send error means the watcher has dropped the receiver.
        let response = match self.request("/chat/stream", req).send().await {
            Ok(response) => response,
            Err(e) => {
                let _ = tx.send(Update::Failed(EngineError::Transport(e.to_string())));
                return;
            }
        };
        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|e| format!("unreadable body: {e}"));
            let _ = tx.send(Update::Failed(EngineError::Status {
                status: status.as_u16(),
                body: body.chars().take(ERROR_BODY_CHARS).collect(),
            }));
            return;
        }

        let mut pending = Vec::new();
        let mut body = response.bytes_stream();
        let mut answered = false;
        while let Some(chunk) = body.next().await {
            let chunk = match chunk {
                Ok(bytes) => bytes,
                Err(e) => {
                    if !answered {
                        let _ = tx.send(Update::Failed(EngineError::Transport(e.to_string())));
                    }
                    return;
                }
            };
            pending.extend_from_slice(&chunk);
            let mut complete = take_complete(&mut pending);
            for (name, data) in drain_frames(&mut complete) {
                let Some(update) = decode(&name, &data) else {
                    continue;
                };
                if !matches!(update, Update::Progress(_)) {
                    answered = true;
                }
                let _ = tx.send(update);
            }
        }
        if !answered {
            let _ = tx.send(Update::Failed(EngineError::Transport(
                "the engine closed the stream without answering".into(),
            )));
        }
    }
}

/// The JSON body of a reply, or the status and the start of the body when it failed.
async fn read<R: DeserializeOwned>(response: reqwest::Response) -> Result<R, EngineError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| EngineError::Transport(e.to_string()))?;
    if !status.is_success() {
        return Err(EngineError::Status {
            status: status.as_u16(),
            body: body.chars().take(ERROR_BODY_CHARS).collect(),
        });
    }
    serde_json::from_str(&body).map_err(|e| EngineError::Transport(format!("bad body: {e}")))
}

/// W3C traceparent for the current span, if tracing is exporting.
pub fn current_traceparent() -> Option<String> {
    let cx = tracing::Span::current().context();
    let span = cx.span();
    let sc = span.span_context();
    sc.is_valid().then(|| {
        format!(
            "00-{}-{}-{:02x}",
            sc.trace_id(),
            sc.span_id(),
            sc.trace_flags().to_u8()
        )
    })
}
