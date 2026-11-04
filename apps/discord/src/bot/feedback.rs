//! Helpful and Not helpful on an answer: recorded as a product event with the request id.

use serenity::all::{
    ComponentInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use uuid::Uuid;

use super::{Handler, event};

impl Handler {
    /// Records the rating of the user who asked, and tells only them it was noted.
    pub(super) async fn feedback(
        &self,
        ctx: &Context,
        press: &ComponentInteraction,
        helpful: bool,
        request: Uuid,
        asker: u64,
    ) {
        let text = if press.user.id.get() == asker {
            self.record(
                event(
                    "discord_feedback",
                    press.user.id,
                    press.guild_id,
                    press.channel_id,
                )
                .with("request_id", request.to_string())
                .with("helpful", helpful),
            );
            tracing::info!(request_id = %request, helpful, "answer rated");
            "Thanks, noted."
        } else {
            "Only the person who asked can rate this answer."
        };
        let msg = CreateInteractionResponseMessage::new()
            .content(text)
            .ephemeral(true);
        if let Err(e) = press
            .create_response(&ctx.http, CreateInteractionResponse::Message(msg))
            .await
        {
            tracing::warn!(error = %e, "feedback response failed");
        }
    }
}
