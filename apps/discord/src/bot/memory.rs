//! The /reset, /memories, and /forget commands, and the forget everything buttons.

use serenity::all::{
    CommandInteraction, ComponentInteraction, Context, CreateInteractionResponse,
    CreateInteractionResponseMessage, EditInteractionResponse,
};
use tracing::Instrument;

use super::commands;
use super::respond::{defer_private, finish, tell};
use super::{Handler, error_kind, event, option_str};
use crate::core::types::{ForgetRequest, MemoryRequest, ResetRequest};
use crate::render::components::{self, CustomId};
use crate::render::reply;

/// What the which option of /forget asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Which {
    /// Every memory.
    All,
    /// The memory with this number in /memories.
    One(usize),
    /// Neither a number nor all.
    Unclear,
}

/// Reads the which option: a number from 1, or all.
pub fn parse_which(text: &str) -> Which {
    let text = text.trim().trim_start_matches('#');
    if text.eq_ignore_ascii_case("all") {
        return Which::All;
    }
    match text.parse::<usize>() {
        Ok(n) if n > 0 => Which::One(n),
        _ => Which::Unclear,
    }
}

/// Asked before erasing everything.
const FORGET_ALL_PROMPT: &str = "Forget everything I remember about you? This cannot be undone.";

impl Handler {
    /// Ends the conversation of the caller in this channel or thread.
    pub(super) async fn reset(&self, ctx: &Context, cmd: &CommandInteraction) {
        let Some(guild_id) = cmd.guild_id else {
            tell(ctx, cmd, "Use this in the server.").await;
            return;
        };
        if !defer_private(ctx, cmd).await {
            return;
        }
        let req = ResetRequest {
            user: cmd.user.id.to_string(),
            tenant: guild_id.to_string(),
            channel: cmd.channel_id.to_string(),
        };
        let span = tracing::info_span!(
            "discord.reset",
            "discord.command" = "reset",
            "user.id" = %cmd.user.id,
        );
        let text = match self.engine.reset(&req).instrument(span).await {
            Ok(done) => {
                tracing::info!(user = %cmd.user.id, ended = done.ended, "conversation reset");
                self.record(
                    event("discord_reset", cmd.user.id, cmd.guild_id, cmd.channel_id)
                        .with("ended", done.ended),
                );
                "Fresh start. Ask away.".to_owned()
            }
            Err(e) => {
                tracing::error!(error = %e, user = %cmd.user.id, "reset failed");
                self.record(
                    event("discord_error", cmd.user.id, cmd.guild_id, cmd.channel_id)
                        .with("stage", "reset")
                        .with("kind", error_kind(&e)),
                );
                reply::failure(&e)
            }
        };
        finish(ctx, cmd, vec![text]).await;
    }

    /// Lists what the engine remembers about the caller, numbered.
    pub(super) async fn memories(&self, ctx: &Context, cmd: &CommandInteraction) {
        let Some(guild_id) = cmd.guild_id else {
            tell(ctx, cmd, "Use this in the server.").await;
            return;
        };
        if !defer_private(ctx, cmd).await {
            return;
        }
        let req = MemoryRequest {
            user_id: cmd.user.id.to_string(),
            tenant_id: guild_id.to_string(),
        };
        let span = tracing::info_span!(
            "discord.memories",
            "discord.command" = "memories",
            "user.id" = %cmd.user.id,
        );
        let messages = match self.engine.memories(&req).instrument(span).await {
            Ok(list) => {
                tracing::info!(user = %cmd.user.id, memories = list.memories.len(), "memories listed");
                self.record(
                    event("discord_memory", cmd.user.id, cmd.guild_id, cmd.channel_id)
                        .with("memories", list.memories.len()),
                );
                reply::render_memories(&list, self.max_message_chars)
            }
            Err(e) => {
                tracing::warn!(error = %e, user = %cmd.user.id, "memory list failed");
                self.record(
                    event("discord_error", cmd.user.id, cmd.guild_id, cmd.channel_id)
                        .with("stage", "memory")
                        .with("kind", error_kind(&e)),
                );
                vec![reply::memory_failure(&e)]
            }
        };
        finish(ctx, cmd, messages).await;
    }

    /// Forgets one memory by its number, or asks before forgetting every memory.
    pub(super) async fn forget(&self, ctx: &Context, cmd: &CommandInteraction) {
        let Some(guild_id) = cmd.guild_id else {
            tell(ctx, cmd, "Use this in the server.").await;
            return;
        };
        let which = option_str(cmd, commands::WHICH).unwrap_or_default();
        let number = match parse_which(&which) {
            Which::All => {
                let rows = components::forget_rows(cmd.user.id.get());
                let msg = CreateInteractionResponseMessage::new()
                    .content(FORGET_ALL_PROMPT)
                    .components(components::to_action_rows(&rows))
                    .ephemeral(true);
                if let Err(e) = cmd
                    .create_response(&ctx.http, CreateInteractionResponse::Message(msg))
                    .await
                {
                    tracing::warn!(error = %e, "forget prompt failed");
                }
                return;
            }
            Which::Unclear => {
                tell(ctx, cmd, "Give a number from /memories, or all.").await;
                return;
            }
            Which::One(number) => number,
        };
        if !defer_private(ctx, cmd).await {
            return;
        }
        let req = ForgetRequest {
            user_id: cmd.user.id.to_string(),
            tenant_id: guild_id.to_string(),
            number: Some(number),
        };
        let span = tracing::info_span!(
            "discord.forget",
            "discord.command" = "forget",
            "user.id" = %cmd.user.id,
            "sparky.everything" = false,
        );
        let text = match self.engine.forget(&req).instrument(span).await {
            Ok(done) => {
                tracing::info!(user = %cmd.user.id, removed = done.removed, "forgot one memory");
                self.record(
                    event("discord_forget", cmd.user.id, cmd.guild_id, cmd.channel_id)
                        .with("removed", done.removed)
                        .with("everything", false),
                );
                reply::forgot(done.removed, Some(number))
            }
            Err(e) => {
                tracing::warn!(error = %e, user = %cmd.user.id, "forget failed");
                self.record(
                    event("discord_error", cmd.user.id, cmd.guild_id, cmd.channel_id)
                        .with("stage", "forget")
                        .with("kind", error_kind(&e)),
                );
                reply::forget_failure(&e, number)
            }
        };
        finish(ctx, cmd, vec![text]).await;
    }

    /// Answers the forget everything prompt. Only the user who asked may press it.
    pub(super) async fn forget_pressed(
        &self,
        ctx: &Context,
        press: &ComponentInteraction,
        id: CustomId,
    ) {
        let erase = matches!(id, CustomId::ForgetAll { .. });
        if !id.may_press(press.user.id.get()) {
            let msg = CreateInteractionResponseMessage::new()
                .content("That button is not yours.")
                .ephemeral(true);
            if let Err(e) = press
                .create_response(&ctx.http, CreateInteractionResponse::Message(msg))
                .await
            {
                tracing::warn!(error = %e, "refusal response failed");
            }
            return;
        }
        if let Err(e) = press.defer(&ctx.http).await {
            tracing::warn!(error = %e, "defer failed");
            return;
        }
        let text = match (erase, press.guild_id) {
            (false, _) => "Kept everything.".to_owned(),
            (true, None) => "Use this in the server.".to_owned(),
            (true, Some(guild_id)) => {
                let req = ForgetRequest {
                    user_id: press.user.id.to_string(),
                    tenant_id: guild_id.to_string(),
                    number: None,
                };
                let span = tracing::info_span!(
                    "discord.forget",
                    "discord.command" = "forget",
                    "user.id" = %press.user.id,
                    "sparky.everything" = true,
                );
                match self.engine.forget(&req).instrument(span).await {
                    Ok(done) => {
                        tracing::info!(user = %press.user.id, removed = done.removed, "forgot everything");
                        self.record(
                            event(
                                "discord_forget",
                                press.user.id,
                                press.guild_id,
                                press.channel_id,
                            )
                            .with("removed", done.removed)
                            .with("everything", true),
                        );
                        reply::forgot(done.removed, None)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, user = %press.user.id, "forget failed");
                        self.record(
                            event(
                                "discord_error",
                                press.user.id,
                                press.guild_id,
                                press.channel_id,
                            )
                            .with("stage", "forget")
                            .with("kind", error_kind(&e)),
                        );
                        reply::memory_failure(&e)
                    }
                }
            }
        };
        let edit = EditInteractionResponse::new()
            .content(text)
            .components(Vec::new());
        if let Err(e) = press.edit_response(&ctx.http, edit).await {
            tracing::warn!(error = %e, "could not close the forget prompt");
        }
    }
}
