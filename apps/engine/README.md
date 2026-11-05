# apps/engine

The agent and the HTTP surface in front of it. One loop: assemble a prompt, call the model, run the tools it asks for, repeat until it answers or hits a limit.

```bash
just engine                         # needs .env; see CONTRIBUTING.md for the first run
cargo test -p engine                # unit tests
cargo test -p engine -- --ignored   # tests that need a container runtime
```

## Routes

| Route | Does |
|---|---|
| `POST /chat`, `/chat/stream` | one turn, whole or as server-sent events |
| `POST /confirm` | approves or refuses an action the policy held |
| `POST /conversation/reset` | ends the caller's conversations in a channel |
| `POST /memories/list`, `/memories/forget` | the caller's memories |
| `/oauth/{provider}/...` | Platform login link, unlink, status |
| `/canvas/feed`, `/canvas/feed/remove` | the caller's Canvas calendar feed link |
| `GET /notifications/outgoing`, `POST /notifications/sent` | reminders and nudges the bot sends as DMs |
| `/sandbox...` | sandbox containers and the `run_sandbox` switch |
| `/web/...` | web sign-in, home chat, chats and threads, reminders, notifications, nudge settings and nudge thumbs |
| `/admin/...` | traces, tools, sources, limits, guardrails, instructions, approvals |
| `GET /health/live`, `/health/ready` | up; `{"platform":bool,"model":bool}`, 503 until both are true |

Bot routes need the bearer `SPARKY_ENGINE__SERVICE_TOKEN`. `/web` and `/admin` use the web session. An unreachable model is not a crash: `/health/ready` reports `model:false` and `/chat` answers 502.

## Layout

```
src/core/       config, telemetry, types, traits, tests. Imports nothing else in the crate.
src/runtime/    harness (loop, prompt assembly, safety, compaction, tracing),
                model (Rig client, slot limit),
                tools (knowledge, account, memory, remind, chats, calculate, page, papers, wiki, mcp, sandbox),
                nudge (the background loop and its rules)
src/stores/     sqlite/: conversations, memories, confirmations, reminders, nudges, canvas, backups, prune, import;
                platform/: knowledge search, live queries, linked accounts, the export the
                import reads, and the platform fallback store over the Platform HTTP API
src/routes/     axum handlers
src/wiring/     builds every adapter from config and composes them; the boot checks live here
```

`runtime::harness`, `runtime::model`, `runtime::tools`, `runtime::nudge` and `stores` each import only `core`.

## The loop

`Agent::run` in `runtime/harness/agent/`. History and memory are loaded once before the first step; nothing is retrieved until the model calls a tool. A step calls the model, passes the response through the guardrail, authorizes the requested tool calls against `Policy`, runs the allowed ones, and continues. It stops on an answer, a confirmation, `agent.max_steps`, the request deadline, or cancellation.

Tools are `search` (described and run by Platform), `read_page`, `calculate`, `search_chats`, `remember`, `forget_memory`, `remind`, `cancel_reminder`, `upcoming`, `canvas_deadlines`, `run_sandbox`, `search_papers`, `wikipedia_lookup`, the Outlook and Google Calendar tools when enabled, the Platform MCP tools, and any configured MCP servers. Each declares a `RiskClass`; anything at `policy.confirm_from` or above waits for the caller to approve.

## Nudges

`runtime/nudge/` runs every `nudges.tick_secs`. For each student active in the last `nudges.active_days` it reads their dated items and memories, then sends what is due: morning brief, due soon, start early, new Canvas item, after-deadline check-in, exam week, follow-up, Sunday plan, Friday wins, first-weeks tips, and from a background agent with read-only tools, event matches, internships and hackathons, the academic calendar and late study spots. Quiet hours, a daily cap, per-kind toggles and thumbs on each nudge limit them. Nudges and reminders go out as notifications: the bot DMs them, and each one is also a Sparky message in the student's home chat (`GET /web/home`), where the student can reply or start a thread off it. A new student's home opens with `home.welcome`, and a new topic asked in home is answered in a thread off the question (`[home]`).

Details: Request lifecycle, Agent loop, Capabilities, and Search in [docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md).

## Configuration

`sparky.toml` sections `agent`, `prompt`, `model`, `policy`, `tools`, `sandbox`, `guardrail`, `compaction`, `query`, `mcp`, `outlook`, `gcal`, `canvas`, `web`, `home`, `admin`, `upcoming`, `reminders`, `nudges`, `chat_search`, `calculate`, `read_page`, `papers`, `wikipedia`, `trace`, `telemetry`, `analytics`, `http`, `platform`, `store`, and `memory`. `.env` holds only secrets and per-machine URLs, including the required `SPARKY_PLATFORM__URL` and `SPARKY_PLATFORM__TOKEN`. `Config::validate` rejects a bad combination at boot. `wiring/` adds the checks that need a live dependency: the prompt against one model slot, the capabilities against their budget, the search tool Platform offers the token, and the container runtime behind `run_sandbox`.
