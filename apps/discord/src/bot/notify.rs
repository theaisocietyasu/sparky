//! Sends due reminders as direct messages: polls the engine, messages each student, says which went.

use std::sync::Arc;
use std::time::Duration;

use serenity::all::{CreateMessage, Http, UserId};

use crate::core::types::{EngineError, Outgoing, SentRequest};
use crate::engine::client::EngineClient;

/// Discord error code for a user who does not accept direct messages from the bot.
const CANNOT_MESSAGE: isize = 50_007;

/// The direct message one notification becomes.
pub fn message_text(text: &str) -> String {
    format!("\u{23f0} Reminder: {text}")
}

/// Polls every tick until the process ends.
pub async fn run(engine: EngineClient, http: Arc<Http>, every: Duration) {
    let mut tick = tokio::time::interval(every);
    loop {
        tick.tick().await;
        if let Err(error) = once(&engine, &http).await {
            tracing::warn!(error = %error, "could not deliver reminders");
        }
    }
}

/// Sends what is waiting and reports the ids that are done.
async fn once(engine: &EngineClient, http: &Http) -> Result<(), EngineError> {
    let waiting = engine.outgoing().await?.notifications;
    if waiting.is_empty() {
        return Ok(());
    }
    let mut done = Vec::new();
    for item in &waiting {
        if deliver(http, item).await {
            done.push(item.id.clone());
        }
    }
    tracing::info!(
        waiting = waiting.len(),
        done = done.len(),
        "reminders delivered"
    );
    engine.sent(&SentRequest { ids: done }).await
}

/// Whether the notification is done with: sent, or never sendable to this user.
async fn deliver(http: &Http, item: &Outgoing) -> bool {
    let Ok(id) = item.user_id.parse::<u64>() else {
        tracing::warn!("a notification has no Discord user");
        return true;
    };
    let channel = match UserId::new(id).create_dm_channel(http).await {
        Ok(channel) => channel,
        Err(error) => return gave_up(&error),
    };
    match channel
        .send_message(http, CreateMessage::new().content(message_text(&item.text)))
        .await
    {
        Ok(_) => true,
        Err(error) => gave_up(&error),
    }
}

/// Whether a failed send will never succeed, so it is not tried again.
fn gave_up(error: &serenity::Error) -> bool {
    if let serenity::Error::Http(serenity::all::HttpError::UnsuccessfulRequest(reply)) = error
        && reply.error.code == CANNOT_MESSAGE
    {
        tracing::info!("a student does not accept direct messages; reminder dropped");
        return true;
    }
    tracing::warn!(error = %error, "could not send a reminder; it is tried again");
    false
}
