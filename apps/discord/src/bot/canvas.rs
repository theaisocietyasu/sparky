//! The /canvas command: a private form takes the Calendar Feed link; unlink removes it.

use serenity::all::{
    ActionRowComponent, CommandInteraction, Context, CreateActionRow, CreateInputText,
    CreateInteractionResponse, CreateModal, InputTextStyle, ModalInteraction, ResolvedValue,
};
use tracing::Instrument;

use super::commands;
use super::respond::{defer_private, fill, finish};
use super::{Handler, error_kind, event};
use crate::core::types::{AuthorizeRequest, EngineError, SaveFeedRequest};

/// Longest link the form accepts, as the engine does.
const MAX_LINK_CHARS: u16 = 512;

/// Where the student finds the link.
const WHERE: &str =
    "In Canvas, open Calendar, then Calendar Feed at the bottom right, and copy the link.";

impl Handler {
    /// Opens the link form, or removes the link, by subcommand.
    pub(super) async fn canvas(&self, ctx: &Context, cmd: &CommandInteraction) {
        let unlink = cmd.data.options().into_iter().any(|o| {
            o.name == commands::CANVAS_UNLINK && matches!(o.value, ResolvedValue::SubCommand(_))
        });
        if unlink {
            self.canvas_unlink(ctx, cmd).await;
            return;
        }
        let field = CreateInputText::new(
            InputTextStyle::Short,
            "Calendar Feed link",
            commands::CANVAS_FORM,
        )
        .placeholder("https://canvas.asu.edu/feeds/calendars/user_....ics")
        .max_length(MAX_LINK_CHARS)
        .required(true);
        let form = CreateModal::new(commands::CANVAS_FORM, "Link your Canvas calendar")
            .components(vec![CreateActionRow::InputText(field)]);
        if let Err(e) = cmd
            .create_response(&ctx.http, CreateInteractionResponse::Modal(form))
            .await
        {
            tracing::warn!(error = %e, "canvas form failed to open");
        }
    }

    /// Sends the link from the form to the engine and tells the caller the result.
    pub(super) async fn canvas_form(&self, ctx: &Context, form: &ModalInteraction) {
        if let Err(e) = form.defer_ephemeral(&ctx.http).await {
            tracing::warn!(error = %e, "defer failed");
            return;
        }
        let link = form
            .data
            .components
            .iter()
            .flat_map(|row| row.components.iter())
            .find_map(|c| match c {
                ActionRowComponent::InputText(input)
                    if input.custom_id == commands::CANVAS_FORM =>
                {
                    input.value.clone()
                }
                _ => None,
            })
            .unwrap_or_default();
        let req = SaveFeedRequest {
            user: form.user.id.to_string(),
            link,
        };
        let span = tracing::info_span!(
            "discord.canvas_link",
            "discord.command" = "canvas",
            "user.id" = %form.user.id,
        );
        let text = match self.engine.canvas_save(&req).instrument(span).await {
            Ok(done) if done.saved => {
                tracing::info!(user = %form.user.id, "canvas feed linked");
                self.record(
                    event(
                        "discord_canvas_link",
                        form.user.id,
                        form.guild_id,
                        form.channel_id,
                    )
                    .with("linked", true),
                );
                "Canvas is linked. Send me a direct message and ask what is due. I read your \
                 calendar only when you ask and keep none of it. Send /canvas unlink to remove it."
                    .to_owned()
            }
            Err(EngineError::Status { status: 422, body }) => {
                format!("That link was not accepted: {body}. {WHERE}")
            }
            Ok(_) => "Canvas linking is not available right now.".to_owned(),
            Err(e) => {
                tracing::warn!(error = %e, user = %form.user.id, "canvas link failed");
                self.record(
                    event(
                        "discord_error",
                        form.user.id,
                        form.guild_id,
                        form.channel_id,
                    )
                    .with("stage", "canvas_link")
                    .with("kind", error_kind(&e)),
                );
                "Canvas linking is not available right now.".to_owned()
            }
        };
        fill(ctx, &form.token, vec![text], None, true).await;
    }

    /// Removes the caller's feed link.
    async fn canvas_unlink(&self, ctx: &Context, cmd: &CommandInteraction) {
        if !defer_private(ctx, cmd).await {
            return;
        }
        let req = AuthorizeRequest {
            user: cmd.user.id.to_string(),
        };
        let span = tracing::info_span!(
            "discord.canvas_unlink",
            "discord.command" = "canvas",
            "user.id" = %cmd.user.id,
        );
        let text = match self.engine.canvas_remove(&req).instrument(span).await {
            Ok(done) => {
                self.record(
                    event(
                        "discord_canvas_unlink",
                        cmd.user.id,
                        cmd.guild_id,
                        cmd.channel_id,
                    )
                    .with("removed", done.removed),
                );
                if done.removed {
                    "Your Canvas calendar is unlinked.".to_owned()
                } else {
                    "You had no Canvas calendar linked.".to_owned()
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, user = %cmd.user.id, "canvas unlink failed");
                self.record(
                    event("discord_error", cmd.user.id, cmd.guild_id, cmd.channel_id)
                        .with("stage", "canvas_unlink")
                        .with("kind", error_kind(&e)),
                );
                "Could not unlink Canvas right now.".to_owned()
            }
        };
        finish(ctx, cmd, vec![text]).await;
    }
}
