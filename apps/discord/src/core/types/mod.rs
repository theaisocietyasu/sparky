//! Wire types mirrored from the engine HTTP contract, plus the client error, by domain.

pub mod analytics;
pub mod chat;
pub mod conversation;
pub mod error;
pub mod memory;
pub mod oauth;
pub mod stream;

pub use analytics::AnalyticsEvent;
pub use chat::{ChatRequest, ChatResponse, ConfirmRequest};
pub use conversation::{Attachment, FileAttachment, ResetRequest, ResetResponse, Visibility};
pub use error::{ERROR_BODY_CHARS, EngineError};
pub use memory::{ForgetRequest, ForgetResponse, MemoryList, MemoryRequest};
pub use oauth::{
    AuthorizeRequest, AuthorizeResponse, DisconnectResponse, SaveFeedRequest, SaveFeedResponse,
};
pub use stream::{ErrorFrame, Progress, Update};
