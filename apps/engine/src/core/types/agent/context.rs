//! RequestContext holds per-request state threaded through every call.

use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::core::types::conversation::Visibility;
use crate::core::types::conversation::file::FileAttachment;
use crate::core::types::conversation::image::Attachment;
use crate::core::types::trace::progress::Progress;

/// Per-request state, created at the edge and threaded through every model call, tool call, event.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique id for this request; the trace id.
    pub request_id: Uuid,
    /// The Discord guild or other scope the data belongs to.
    pub tenant_id: String,
    /// Caller identity as known to the edge adapter.
    pub user_id: String,
    /// Role names granted to the caller.
    pub roles: Vec<String>,
    /// Conversation this request continues.
    pub conversation_id: Uuid,
    /// Who can read the exchange this request belongs to.
    pub visibility: Visibility,
    /// The message of ours the caller replied to, when they replied to one.
    pub reply_to: Option<String>,
    /// Images attached to the caller's message.
    pub images: Vec<Attachment>,
    /// Other files attached to the caller's message, opened in the sandbox.
    pub files: Vec<FileAttachment>,
    /// Hard stop for the whole request.
    pub deadline: Instant,
    /// Cancelled by the caller or by the deadline.
    pub cancel: CancellationToken,
    /// Whether the client renders tap-to-answer choices.
    pub widgets: bool,
    /// Where to send live progress, when the caller is watching.
    pub progress: Option<UnboundedSender<Progress>>,
}

impl RequestContext {
    /// Creates a context with a fresh request_id and conversation.
    pub fn new(tenant_id: impl Into<String>, user_id: impl Into<String>, budget: Duration) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            tenant_id: tenant_id.into(),
            user_id: user_id.into(),
            roles: Vec::new(),
            conversation_id: Uuid::new_v4(),
            visibility: Visibility::Public,
            reply_to: None,
            images: Vec::new(),
            files: Vec::new(),
            deadline: Instant::now() + budget,
            cancel: CancellationToken::new(),
            widgets: false,
            progress: None,
        }
    }

    /// Sends live progress to tx for the length of this request.
    pub fn listening_to(mut self, tx: UnboundedSender<Progress>) -> Self {
        self.progress = Some(tx);
        self
    }

    /// Says whether the client renders tap-to-answer choices.
    pub fn with_widgets(mut self, widgets: bool) -> Self {
        self.widgets = widgets;
        self
    }

    /// Continues an existing conversation.
    pub fn with_conversation(mut self, conversation_id: Uuid) -> Self {
        self.conversation_id = conversation_id;
        self
    }

    /// Sets who can read the exchange.
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// Names the message of ours the caller replied to.
    pub fn replying_to(mut self, quoted: Option<String>) -> Self {
        self.reply_to = quoted;
        self
    }

    /// Attaches images to the caller's message. More than most are dropped by the caller.
    pub fn with_images(mut self, images: Vec<Attachment>) -> Self {
        self.images = images;
        self
    }

    /// Attaches files to the caller's message. More than most are dropped by the caller.
    pub fn with_files(mut self, files: Vec<FileAttachment>) -> Self {
        self.files = files;
        self
    }

    /// Grants roles to the caller.
    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    /// Whether the caller holds role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// Time left before the deadline; zero once passed.
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }
}
