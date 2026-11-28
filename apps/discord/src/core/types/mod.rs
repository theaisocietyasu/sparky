//! Wire types mirrored from the engine HTTP contract, plus the client error, by domain.

pub mod analytics;
pub mod canvas;
pub mod chat;
pub mod conversation;
pub mod error;
pub mod memory;
pub mod notify;
pub mod stream;

pub use analytics::AnalyticsEvent;
pub use canvas::{RemoveFeedRequest, RemoveFeedResponse, SaveFeedRequest, SaveFeedResponse};
pub use chat::{ChatRequest, ChatResponse, ConfirmRequest};
pub use conversation::{Attachment, FileAttachment, ResetRequest, ResetResponse, Visibility};
pub use error::{ERROR_BODY_CHARS, EngineError};
pub use memory::{ForgetRequest, ForgetResponse, MemoryList, MemoryRequest};
pub use notify::{Outgoing, OutgoingList, SentRequest};
pub use stream::{ErrorFrame, Progress, Update};
