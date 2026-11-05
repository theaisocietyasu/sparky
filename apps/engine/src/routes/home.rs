//! The home chat: the welcome a new student gets, and new topics moved into threads of their own.

use std::sync::Arc;

use uuid::Uuid;

use crate::core::traits::conversation::ConversationStore;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::conversation::{Parent, WEB_CHANNEL};
use crate::core::types::store::StoreError;
use crate::runtime::harness::agent::task::Task;

/// Messages read back to find what Sparky said last in home.
const LOOK_BACK: usize = 4;

/// What the home chat does beyond holding messages.
#[derive(Clone, Default)]
pub struct HomeRules {
    /// Sparky's first message in an empty home chat. Empty sends none.
    pub welcome: String,
    /// Judges whether a message opens a new topic. None keeps every message in home.
    pub router: Option<Arc<Task>>,
}

/// Writes the welcome into the home chat ctx names when it holds no message yet.
pub async fn welcome(
    store: &dyn ConversationStore,
    ctx: &RequestContext,
    text: &str,
) -> Result<(), StoreError> {
    let text = text.trim();
    if text.is_empty() || !store.load(ctx, 1).await?.is_empty() {
        return Ok(());
    }
    store.append(ctx, &[Message::assistant(text)]).await
}

/// Whether message opens a new topic, judged against what Sparky said last. A failure keeps it in home.
pub async fn opens_topic(
    router: &Task,
    store: &dyn ConversationStore,
    ctx: &RequestContext,
    message: &str,
) -> bool {
    let last = store
        .load(ctx, LOOK_BACK)
        .await
        .unwrap_or_default()
        .into_iter()
        .rev()
        .find(|s| s.message.role == Role::Assistant)
        .map(|s| s.message.content)
        .unwrap_or_default();
    let input = format!("Sparky's last message:\n{last}\n\nThe student's message:\n{message}");
    match router.run(ctx, &input).await {
        Ok(answer) => answer.trim().to_uppercase().starts_with("THREAD"),
        Err(error) => {
            tracing::warn!(%error, "could not sort a home message; it stays in home");
            false
        }
    }
}

/// Writes message into the home chat ctx names and opens a thread off it. Returns the thread.
pub async fn branch(
    store: &dyn ConversationStore,
    ctx: &RequestContext,
    message: &str,
) -> Result<RequestContext, StoreError> {
    store.append(ctx, &[Message::user(message)]).await?;
    let position = store
        .load(ctx, 1)
        .await?
        .last()
        .map(|s| s.position)
        .ok_or_else(|| StoreError::Database("the home message was not kept".into()))?;
    let thread = ctx.clone().with_conversation(Uuid::new_v4());
    store.ensure(&thread, WEB_CHANNEL).await?;
    store
        .link(
            &thread,
            &Parent {
                conversation_id: ctx.conversation_id,
                position,
            },
        )
        .await?;
    Ok(thread)
}
