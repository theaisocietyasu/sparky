//! ConversationStore trait.

pub mod compaction;

use async_trait::async_trait;
use uuid::Uuid;

use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::Message;
use crate::core::types::conversation::{ChatHit, ConversationSummary, Parent, Stored};
use crate::core::types::store::StoreError;

/// Durable conversation history, scoped by tenant, user, and conversation.
#[async_trait]
pub trait ConversationStore: Send + Sync {
    /// Ensures the conversation row exists for this user; errors if the id belongs to another.
    async fn ensure(&self, ctx: &RequestContext, channel_id: &str) -> Result<(), StoreError>;
    /// Whether the conversation exists and belongs to the user of this request.
    async fn owns(&self, ctx: &RequestContext) -> Result<bool, StoreError>;
    /// Loads history oldest first: the newest summary, then up to limit messages after it.
    async fn load(&self, ctx: &RequestContext, limit: usize) -> Result<Vec<Stored>, StoreError>;
    /// Appends turns in order.
    async fn append(&self, ctx: &RequestContext, turns: &[Message]) -> Result<(), StoreError>;
    /// Stores a summary that stands in for every message up to and including position covers.
    async fn append_summary(
        &self,
        ctx: &RequestContext,
        summary: &Message,
        covers: i64,
    ) -> Result<(), StoreError>;
    /// The caller's most recently updated open conversation in channel_id, at request visibility.
    async fn latest(
        &self,
        ctx: &RequestContext,
        channel_id: &str,
    ) -> Result<Option<Uuid>, StoreError>;
    /// Ends every open conversation of the caller in channel_id, any visibility; returns the count.
    async fn end(&self, ctx: &RequestContext, channel_id: &str) -> Result<u64, StoreError>;
    /// The caller's open conversations in channel_id that hold a question, newest first.
    async fn list(
        &self,
        _ctx: &RequestContext,
        _channel_id: &str,
        _limit: usize,
    ) -> Result<Vec<ConversationSummary>, StoreError> {
        Ok(Vec::new())
    }
    /// The caller's private messages in other conversations that match query, best first.
    async fn search(
        &self,
        _ctx: &RequestContext,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<ChatHit>, StoreError> {
        Ok(Vec::new())
    }
    /// Makes the request conversation a thread of parent, when both are the caller's and it is none yet.
    async fn link(&self, _ctx: &RequestContext, _parent: &Parent) -> Result<(), StoreError> {
        Ok(())
    }
    /// Deletes the request conversation and its messages when the caller owns it.
    async fn delete(&self, _ctx: &RequestContext) -> Result<bool, StoreError> {
        Ok(false)
    }
}
