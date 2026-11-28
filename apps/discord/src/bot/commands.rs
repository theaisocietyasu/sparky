//! Slash command names and their registration: reset, memories, forget, canvas.

use serenity::all::{CommandOptionType, CreateCommand, CreateCommandOption};

/// The /reset command, which ends the conversation in this channel.
pub const RESET: &str = "reset";
/// The /memories command, which lists what the engine remembers about the user.
pub const MEMORIES: &str = "memories";
/// The /forget command, which removes one memory or every memory.
pub const FORGET: &str = "forget";
/// Name of the option on /forget: a number from /memories, or all.
pub const WHICH: &str = "which";
/// The /canvas command, which links or unlinks the user's Canvas calendar feed.
pub const CANVAS: &str = "canvas";
/// The /canvas subcommand that opens the form for the feed link.
pub const CANVAS_LINK: &str = "link";
/// The /canvas subcommand that removes the feed link.
pub const CANVAS_UNLINK: &str = "unlink";
/// Custom id of the form that takes the feed link, and of its one field.
pub const CANVAS_FORM: &str = "canvas_feed";

/// Every command the bot registers on its guild.
pub fn all() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new(RESET)
            .description("End the conversation in this channel and start over"),
        CreateCommand::new(MEMORIES).description("See what Sparky remembers about you"),
        CreateCommand::new(FORGET)
            .description("Make Sparky forget one memory, or every memory, about you")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    WHICH,
                    "A number from /memories, or all",
                )
                .required(true),
            ),
        CreateCommand::new(CANVAS)
            .description("Link your Canvas calendar so Sparky can list due dates in DMs")
            .add_option(CreateCommandOption::new(
                CommandOptionType::SubCommand,
                CANVAS_LINK,
                "Paste your Canvas Calendar Feed link in a private form",
            ))
            .add_option(CreateCommandOption::new(
                CommandOptionType::SubCommand,
                CANVAS_UNLINK,
                "Remove your Canvas Calendar Feed link from Sparky",
            )),
    ]
}
