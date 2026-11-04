//! serenity client setup, the guards every question passes, and event dispatch.

mod address;
mod canvas;
mod commands;
mod confirm;
mod destination;
mod feedback;
mod login;
pub(crate) mod memory;
mod respond;
mod turn;

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use secrecy::ExposeSecret;
use serenity::all::{
    AutoArchiveDuration, ChannelId, Client, Command, CommandInteraction, Context, CreateThread,
    EventHandler, GatewayIntents, GuildId, Interaction, Message, Ready, ResolvedValue, RoleId,
    UserId,
};
use serenity::async_trait;
use tokio::sync::Mutex;

use crate::access::{message, route};
use crate::analytics::Analytics;
use crate::core::config::Config;
use crate::core::types::{AnalyticsEvent, EngineError};
use crate::engine::client::EngineClient;
use crate::render::components::CustomId;

/// Per-process bot state and the serenity event handler.
struct Handler {
    engine: EngineClient,
    /// Product events, exported as spans.
    analytics: Analytics,
    guild_id: GuildId,
    /// Channels the bot answers in. Empty answers everywhere it can see.
    channels: Vec<ChannelId>,
    /// Shortest gap between edits of the progress message.
    edit_every: Duration,
    /// Longest message posted before a reply is split.
    max_message_chars: usize,
    /// Shortest gap between the questions of one user. Zero removes the limit.
    cooldown: Duration,
    /// Role name the engine policy reads to allow write-side tools.
    write_capability: String,
    /// How long a thread the bot opens stays active without messages.
    thread_archive: AutoArchiveDuration,
    /// Whether a direct message is answered.
    direct_messages: bool,
    /// Images of one message sent to the model.
    max_images: usize,
    /// Other files of one message sent to the engine.
    max_files: usize,
    /// Largest file sent to the engine, in bytes.
    max_file_bytes: u64,
    /// The bot user, set once the gateway is ready.
    me: OnceLock<UserId>,
    /// The role Discord manages for the bot, set once the gateway is ready.
    role: OnceLock<RoleId>,
    /// When each user last asked, for the cooldown.
    last_ask: Mutex<HashMap<UserId, Instant>>,
}

/// Connects to Discord and runs until shutdown.
pub async fn run(cfg: Config) -> anyhow::Result<()> {
    if cfg.discord.guild_id == 0 {
        anyhow::bail!("SPARKY_DISCORD__GUILD_ID is unset; set it to the guild the bot serves");
    }
    if cfg.discord.token.expose_secret().trim().is_empty() {
        anyhow::bail!("SPARKY_DISCORD__TOKEN is unset; create a bot at discord.com/developers");
    }
    let engine = EngineClient::new(
        &cfg.engine.base_url,
        cfg.engine.service_token.clone(),
        Duration::from_secs(cfg.engine.connect_timeout_secs),
        Duration::from_secs(cfg.engine.request_timeout_secs),
    )?;
    let analytics = Analytics::start(&cfg.analytics);
    let handler = Handler {
        engine,
        analytics,
        guild_id: GuildId::new(cfg.discord.guild_id),
        channels: cfg
            .bot
            .channels
            .iter()
            .copied()
            .map(ChannelId::new)
            .collect(),
        edit_every: Duration::from_millis(cfg.bot.edit_every_ms),
        max_message_chars: cfg.bot.max_message_chars,
        cooldown: Duration::from_secs(cfg.bot.cooldown_secs),
        write_capability: cfg.bot.write_capability.clone(),
        thread_archive: archive_after(cfg.bot.thread_auto_archive_minutes),
        direct_messages: cfg.bot.direct_messages,
        max_images: cfg.bot.max_images,
        max_files: cfg.bot.max_files,
        max_file_bytes: cfg.bot.max_file_bytes,
        me: OnceLock::new(),
        role: OnceLock::new(),
        last_ask: Mutex::new(HashMap::new()),
    };
    // MESSAGE_CONTENT is privileged and must be enabled on the application at discord.com/developers.
    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(cfg.discord.token.expose_secret(), intents)
        .event_handler(handler)
        .await?;
    let shards = client.shard_manager.clone();
    let result = tokio::select! {
        started = client.start() => started.map_err(anyhow::Error::from),
        () = interrupted() => {
            tracing::info!("shutting down");
            shards.shutdown_all().await;
            Ok(())
        }
    };
    result
}

/// Resolves on ctrl-c. Never resolves when the signal cannot be watched.
async fn interrupted() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::warn!(error = %e, "cannot watch ctrl-c");
        std::future::pending::<()>().await;
    }
}

/// An analytics event by user in channel of guild.
fn event(
    name: &'static str,
    user: UserId,
    guild: Option<GuildId>,
    channel: ChannelId,
) -> AnalyticsEvent {
    AnalyticsEvent::new(name, &user)
        .with("guild_id", guild.map(|g| g.to_string()).unwrap_or_default())
        .with("channel_id", channel.to_string())
}

/// A short name for an engine failure: transport or status_NNN.
fn error_kind(e: &EngineError) -> String {
    match e {
        EngineError::Transport(_) => "transport".to_owned(),
        EngineError::Status { status, .. } => format!("status_{status}"),
    }
}

/// The Discord duration for a validated number of minutes.
fn archive_after(minutes: u16) -> AutoArchiveDuration {
    match minutes {
        60 => AutoArchiveDuration::OneHour,
        4_320 => AutoArchiveDuration::ThreeDays,
        10_080 => AutoArchiveDuration::OneWeek,
        _ => AutoArchiveDuration::OneDay,
    }
}

impl Handler {
    /// Whether the bot answers in this channel, or in the channel a thread hangs off.
    fn serves(&self, channel: ChannelId, parent: Option<ChannelId>) -> bool {
        route::serves(&self.channels, channel, parent)
    }

    /// Counts one question from user and says whether it may run.
    async fn within_cooldown(&self, user: UserId) -> bool {
        if self.cooldown.is_zero() {
            return true;
        }
        let mut last = self.last_ask.lock().await;
        if last
            .get(&user)
            .is_some_and(|at| at.elapsed() < self.cooldown)
        {
            return false;
        }
        // Entries past the cooldown decide nothing, so the map holds only recent askers.
        let cooldown = self.cooldown;
        last.retain(|_, at| at.elapsed() < cooldown);
        last.insert(user, Instant::now());
        true
    }

    /// What to tell a user who asked again inside the cooldown.
    fn cooldown_text(&self) -> String {
        format!(
            "Give me {} seconds between questions.",
            self.cooldown.as_secs()
        )
    }

    /// Queues a product event without waiting.
    fn record(&self, event: AnalyticsEvent) {
        self.analytics.record(event);
    }

    /// The builder for a thread opened from a question.
    fn thread_for(&self, question: &str) -> CreateThread<'static> {
        CreateThread::new(message::thread_name(question)).auto_archive_duration(self.thread_archive)
    }
}

/// The string value of a command option, if given.
fn option_str(cmd: &CommandInteraction, name: &str) -> Option<String> {
    cmd.data
        .options()
        .into_iter()
        .find(|o| o.name == name)
        .and_then(|o| match o.value {
            ResolvedValue::String(s) => Some(s.to_owned()),
            _ => None,
        })
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        if self.me.set(ready.user.id).is_err() {
            tracing::debug!("gateway ready again after a reconnect");
        }
        tracing::info!(user = %ready.user.name, guild = %self.guild_id, "connected");
        match self.guild_id.set_commands(&ctx.http, commands::all()).await {
            Ok(cmds) => tracing::info!(count = cmds.len(), "commands registered"),
            Err(e) => tracing::error!(error = %e, "command registration failed"),
        }
        // Every command is registered on the guild; global commands are removed.
        match Command::set_global_commands(&ctx.http, Vec::new()).await {
            Ok(_) => tracing::debug!("global commands cleared"),
            Err(e) => tracing::warn!(error = %e, "clearing global commands failed"),
        }
        match self.guild_id.roles(&ctx.http).await {
            Ok(roles) => {
                let tagged = roles
                    .values()
                    .map(|r| (r.id, r.tags.bot_id))
                    .collect::<Vec<_>>();
                if let Some(role) = message::bot_role(tagged, ready.user.id)
                    && self.role.set(role).is_err()
                {
                    tracing::debug!("bot role already known");
                }
            }
            Err(e) => tracing::warn!(error = %e, "bot role lookup failed"),
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        self.addressed(&ctx, &msg).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Component(press) => match CustomId::parse(&press.data.custom_id) {
                Some(CustomId::Confirm {
                    action,
                    token,
                    conversation,
                }) => {
                    self.confirm(&ctx, &press, action, token, conversation)
                        .await;
                }
                Some(id @ (CustomId::ForgetAll { .. } | CustomId::KeepAll { .. })) => {
                    self.forget_pressed(&ctx, &press, id).await;
                }
                Some(CustomId::Feedback {
                    helpful,
                    request,
                    user,
                }) => {
                    self.feedback(&ctx, &press, helpful, request, user).await;
                }
                None => {
                    tracing::debug!(custom_id = %press.data.custom_id, "component is not ours");
                }
            },
            Interaction::Command(cmd) => match cmd.data.name.as_str() {
                commands::RESET => self.reset(&ctx, &cmd).await,
                commands::MEMORIES => self.memories(&ctx, &cmd).await,
                commands::FORGET => self.forget(&ctx, &cmd).await,
                commands::LOGIN => self.login(&ctx, &cmd).await,
                commands::LOGOUT => self.logout(&ctx, &cmd).await,
                commands::CANVAS => self.canvas(&ctx, &cmd).await,
                other => tracing::warn!(command = other, "unknown command"),
            },
            Interaction::Modal(form) if form.data.custom_id == commands::CANVAS_FORM => {
                self.canvas_form(&ctx, &form).await;
            }
            _ => {}
        }
    }
}
