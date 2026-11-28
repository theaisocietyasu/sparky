//! Messages addressed to the bot: a mention in a channel, a reply in a thread, or a direct message.

use std::collections::HashMap;

use serenity::all::{Context, Message, Permissions, RoleId, UserId};
use tracing::field::Empty;

use super::destination::Destination;
use super::{Handler, event};
use crate::access::message;
use crate::access::roles::{authorized_roles, member_permissions};
use crate::access::route::{self, Arrival, Arrived, Place, Trigger};
use crate::core::types::{Attachment, FileAttachment};
use crate::render::reply;

/// The text of the bot message msg replies to, or None when it replies to nothing of the bot's.
fn replied_to(msg: &Message, me: UserId) -> Option<String> {
    let referenced = msg.referenced_message.as_deref()?;
    message::quoted(Some(referenced.author.id), &referenced.content, me)
}

/// What a trigger is called on a span and in analytics.
fn trigger_name(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Direct => "direct",
        Trigger::Opening => "mention",
        Trigger::Reply => "reply",
    }
}

impl Handler {
    /// Answers a message addressed to the bot. Everything else is ignored.
    pub(super) async fn addressed(&self, ctx: &Context, msg: &Message) {
        let Some(me) = self.admits(msg) else {
            return;
        };
        let direct = msg.guild_id.is_none();
        let quoted = replied_to(msg, me);
        let here = Destination {
            channel: msg.channel_id,
            reply_to: Some(msg.id),
        };
        let Some((at, parent)) = self.placed(ctx, msg, direct, &here).await else {
            return;
        };
        let Some(trigger) = route::trigger(Arrival {
            at,
            mentions_bot: message::addresses_bot(
                msg.mentions_user_id(me),
                &msg.mention_roles,
                self.role.get().copied(),
            ),
            replies_to_bot: quoted.is_some(),
        }) else {
            tracing::debug!(at = ?at, "ignored: not addressed to the bot");
            return;
        };
        let in_thread = at == Arrived::Thread;
        if !direct && !self.serves(msg.channel_id, parent) {
            tracing::debug!(channel = %msg.channel_id, "ignored: channel not served");
            return;
        }
        if !self.within_cooldown(msg.author.id).await {
            here.say(&ctx.http, self.cooldown_text()).await;
            return;
        }
        let question = message::strip_mentions(&msg.content, me, self.role.get().copied());
        let (images, files) = self.attached(msg);
        if question.is_empty() && images.is_empty() && files.is_empty() {
            here.say(&ctx.http, "Ask me something.").await;
            return;
        }
        let label = route::place_name(direct, in_thread);
        self.record(
            event(
                "discord_message",
                msg.author.id,
                msg.guild_id,
                msg.channel_id,
            )
            .with("place", label)
            .with("trigger", trigger_name(trigger)),
        );
        let Some(roles) = self.roles_for(ctx, msg, direct, label, &here).await else {
            return;
        };
        let (dest, place) = match trigger {
            Trigger::Direct => (here, Place::Private(msg.channel_id)),
            Trigger::Reply => (here, Place::Thread(msg.channel_id)),
            Trigger::Opening => self.open_thread(ctx, msg, &question, here).await,
        };
        let mut req = route::chat_request(
            place,
            msg.author.id,
            self.guild_id,
            roles,
            question,
            quoted,
            images,
        );
        req.files = files;
        let span = tracing::info_span!(
            "discord.message",
            "discord.command" = trigger_name(trigger),
            "sparky.visibility" = ?req.visibility,
            "sparky.input" = %req.message,
            "sparky.output" = Empty,
            // OpenInference, read by the Phoenix trace UI.
            "openinference.span.kind" = "CHAIN",
            "input.value" = %req.message,
            "output.value" = Empty,
            "session.id" = Empty,
            "otel.status_code" = Empty,
            "otel.status_message" = Empty,
            "user.id" = %msg.author.id,
        );
        self.converse(ctx, &dest, &req, span, label).await;
    }

    /// The bot user when msg is from a person in a place the bot serves, or None to ignore it.
    fn admits(&self, msg: &Message) -> Option<UserId> {
        if msg.author.bot {
            return None;
        }
        tracing::debug!(
            guild = ?msg.guild_id,
            channel = %msg.channel_id,
            author = %msg.author.id,
            mentions = ?msg.mentions.iter().map(|u| u.id).collect::<Vec<_>>(),
            mention_roles = ?msg.mention_roles,
            bot_role = ?self.role.get(),
            chars = msg.content.chars().count(),
            "message seen"
        );
        let Some(&me) = self.me.get() else {
            tracing::debug!("ignored: gateway not ready");
            return None;
        };
        if msg.guild_id.is_none() && !self.direct_messages {
            tracing::debug!("ignored: direct messages are off");
            return None;
        }
        if let Some(guild_id) = msg.guild_id
            && guild_id != self.guild_id
        {
            tracing::debug!(guild = %guild_id, served = %self.guild_id, "ignored: another guild");
            return None;
        }
        Some(me)
    }

    /// Role names for the turn. None means the lookup failed and the caller was told so.
    async fn roles_for(
        &self,
        ctx: &Context,
        msg: &Message,
        direct: bool,
        label: &'static str,
        here: &Destination,
    ) -> Option<Vec<String>> {
        if direct {
            return Some(Vec::new());
        }
        match self.message_roles(ctx, msg).await {
            Ok(roles) => Some(roles),
            Err(e) => {
                tracing::error!(error = %e, user = %msg.author.id, "role lookup failed");
                self.record(
                    event("discord_error", msg.author.id, msg.guild_id, msg.channel_id)
                        .with("stage", "roles")
                        .with("kind", "discord")
                        .with("place", label),
                );
                here.say(
                    &ctx.http,
                    "I could not verify your roles, so I did not run that.",
                )
                .await;
                None
            }
        }
    }

    /// The kind of place msg arrived in, and the channel the allowlist checks. None stops the turn.
    async fn placed(
        &self,
        ctx: &Context,
        msg: &Message,
        direct: bool,
        here: &Destination,
    ) -> Option<(Arrived, Option<serenity::all::ChannelId>)> {
        if direct {
            return Some((Arrived::Direct, None));
        }
        let channel = match msg.channel_id.to_channel(ctx).await {
            Ok(channel) => channel.guild(),
            Err(e) => {
                tracing::warn!(error = %e, channel = %msg.channel_id, "channel lookup failed");
                here.say(&ctx.http, reply::UNAVAILABLE).await;
                return None;
            }
        };
        let Some(channel) = channel else {
            tracing::debug!(channel = %msg.channel_id, "ignored: not a guild channel");
            return None;
        };
        let at = if route::is_thread(channel.kind) {
            Arrived::Thread
        } else {
            Arrived::Channel
        };
        Some((
            at,
            route::thread_parent(Some(channel.kind), channel.parent_id),
        ))
    }

    /// Opens a thread from msg for question. Falls back to here when the thread cannot be made.
    async fn open_thread(
        &self,
        ctx: &Context,
        msg: &Message,
        question: &str,
        here: Destination,
    ) -> (Destination, Place) {
        match msg
            .channel_id
            .create_thread_from_message(&ctx.http, msg.id, self.thread_for(question))
            .await
        {
            Ok(thread) => (
                Destination {
                    channel: thread.id,
                    reply_to: None,
                },
                Place::Thread(thread.id),
            ),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    channel = %msg.channel_id,
                    "could not open a thread; answering inline"
                );
                (here, Place::Inline(msg.channel_id))
            }
        }
    }

    /// Role names the author holds plus the capability their guild roles or ownership grant.
    async fn message_roles(
        &self,
        ctx: &Context,
        msg: &Message,
    ) -> Result<Vec<String>, serenity::Error> {
        let held: Vec<RoleId> = msg.member.as_ref().map_or_default(|m| m.roles.clone());
        let owner = self.owner(ctx).await?;
        let roles = self.guild_id.roles(&ctx.http).await?;
        let bits: HashMap<RoleId, Permissions> =
            roles.iter().map(|(id, r)| (*id, r.permissions)).collect();
        let everyone = RoleId::new(self.guild_id.get());
        let permissions = member_permissions(everyone, &held, &bits, msg.author.id == owner);
        let names = held
            .iter()
            .filter_map(|id| roles.get(id).map(|r| r.name.clone()));
        Ok(authorized_roles(
            names,
            Some(permissions),
            &self.write_capability,
        ))
    }

    /// The guild owner, from the cache when it holds the guild.
    async fn owner(&self, ctx: &Context) -> Result<UserId, serenity::Error> {
        let cached = ctx.cache.guild(self.guild_id).map(|g| g.owner_id);
        if let Some(owner) = cached {
            return Ok(owner);
        }
        Ok(self.guild_id.to_partial_guild(&ctx.http).await?.owner_id)
    }

    /// The images of msg the model is sent, and the other files the engine opens.
    fn attached(&self, msg: &Message) -> (Vec<Attachment>, Vec<FileAttachment>) {
        let images = message::images(
            msg.attachments
                .iter()
                .map(|a| (a.url.as_str(), a.content_type.as_deref())),
            self.max_images,
        );
        let files = message::files(
            msg.attachments.iter().map(|a| {
                (
                    a.url.as_str(),
                    a.filename.as_str(),
                    a.content_type.as_deref(),
                    u64::from(a.size),
                )
            }),
            self.max_files,
            self.max_file_bytes,
        );
        (images, files)
    }
}
