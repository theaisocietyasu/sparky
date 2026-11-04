//! Slash command names and their registration: reset, memories, forget, login, logout.

use serenity::all::{CommandOptionType, CreateCommand, CreateCommandOption};

/// The /reset command, which ends the conversation in this channel.
pub const RESET: &str = "reset";
/// The /memories command, which lists what the engine remembers about the user.
pub const MEMORIES: &str = "memories";
/// The /forget command, which removes one memory or every memory.
pub const FORGET: &str = "forget";
/// Name of the option on /forget: a number from /memories, or all.
pub const WHICH: &str = "which";
/// The /login command, which connects one of the user's accounts.
pub const LOGIN: &str = "login";
/// The /logout command, which disconnects one of the user's accounts.
pub const LOGOUT: &str = "logout";
/// Name of the service option on /login and /logout.
pub const SERVICE: &str = "service";
/// The services a login connects: the option value is the provider key the engine expects.
pub const SERVICES: [(&str, &str); 3] = [
    ("Canvas", "canvas"),
    ("Outlook", "microsoft"),
    ("Google Calendar", "google"),
];

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
        CreateCommand::new(LOGIN)
            .description("Connect an account (Canvas, Outlook) so Sparky can check it in DMs")
            .add_option(service_option()),
        CreateCommand::new(LOGOUT)
            .description("Disconnect an account (Canvas, Outlook) from Sparky")
            .add_option(service_option()),
    ]
}

/// The required service choice shared by /login and /logout.
fn service_option() -> CreateCommandOption {
    let mut option = CreateCommandOption::new(
        CommandOptionType::String,
        SERVICE,
        "Which account to connect or disconnect",
    )
    .required(true);
    for (label, value) in SERVICES {
        option = option.add_string_choice(label, value);
    }
    option
}
