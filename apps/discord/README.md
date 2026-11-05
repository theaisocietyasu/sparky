# apps/discord

The Discord bot: a serenity gateway client and an HTTP client of the engine. It never links the engine crate and holds no agent logic. It decides who may ask, relays a turn's progress, and renders the answer.

```bash
just discord                        # needs SPARKY_DISCORD__TOKEN and SPARKY_DISCORD__GUILD_ID in .env
cargo test -p discord
```

## What it does

- Decides whether a message is for the bot: a mention in a channel, a reply to the bot inside a thread, or a direct message when `bot.direct_messages` is on, within `bot.channels` when that is set.
- Sends one `POST /chat/stream` per turn with the asker, the channel, their roles, the message they replied to, images, and files.
- Renders the stream into one card edited as the turn runs: the thinking line, the steps, then the answer and its sources. `bot.edit_every_ms` paces the edits; a message longer than `bot.max_message_chars` is split on line boundaries.
- Shows a held action as buttons and calls `POST /confirm` with the token when someone presses one.
- Polls `GET /notifications/outgoing` and DMs each reminder and nudge, then marks it with `POST /notifications/sent`.
- Exports one span per interaction and one span per product event.

Entry points, visibility, and the card are described under Discord surface in [docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md).

## Layout

```
src/core/       config, telemetry, types, tests. Imports nothing else in the crate.
src/bot/        the serenity handler: addressing, turns, threads, memory commands, confirmations
src/engine/     the HTTP client of apps/engine, including the SSE stream
src/render/     the card, the step list, splitting, buttons
src/access/     who may ask, and where the answer goes and who can see it
src/analytics/  product events, one exported span each
```

## Configuration

`[bot]` in `sparky.toml` holds behaviour: channels, edit pacing, cooldown, thread archiving, image and file limits, and `bot.write_capability`, the role name the engine's policy reads for write-side tools. It must match `policy.write_roles` or a write tool is refused for everyone.

`.env` holds `SPARKY_DISCORD__TOKEN`, `SPARKY_DISCORD__GUILD_ID`, and `SPARKY_ENGINE__SERVICE_TOKEN`, which must match the engine's.
