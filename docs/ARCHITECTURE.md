# Architecture

SparkyAI is a Discord copilot for the AI Society at ASU. It answers questions from public ASU sources, keeps conversation history and a profile of each user, and gates moderator actions behind policy and confirmation.

This document describes the current shape of the system and its rules. Order of work is in the master issue [#79](https://github.com/theaisocietyasu/sparky/issues/79); direction and scope are in [ROADMAP.md](ROADMAP.md). Decisions are recorded in the commits that settle them.

## Stack

| Layer | Choice | Where |
|---|---|---|
| Engine, Discord bot, console | tokio, axum, serenity, ratatui, serde, thiserror, figment | `apps/engine`, `apps/discord`, `apps/cli` |
| Model and embed clients | Rig (`rig-core`) OpenAI-compatible client | `apps/engine/src/runtime/model/rig_openai/` |
| MCP | `rmcp` | `apps/engine/src/runtime/tools/mcp.rs` |
| Conversations, memories, confirmations | one SQLite file through `sqlx`, opened by the engine only | `apps/engine/src/stores/sqlite/` |
| Knowledge, live ASU queries, linked accounts | Platform HTTP API and MCP | [asusoda/platform](https://github.com/asusoda/platform), a separate repo |
| Web | Vite, React, TypeScript, shadcn | `apps/web` |
| Fine-tuning | lives outside this repo | [loupe](https://github.com/ashworks1706/loupe), a separate repo |
| Evals | golden cases against the engine, baseline gate | `apps/evals` |
| Chat model | Qwen3 GGUF on `llama-server` | compose service `chat`, profile `model` |
| Embeddings | Qwen3-Embedding-0.6B, 1024 dimensions, on `llama-server` | compose service `embed`, profile `model` |
| Observability | OpenTelemetry spans to Phoenix, product events as spans, JSONL traces and logs under `.sparky/` | profiles `phoenix`, `metrics` |
| Config | `sparky.toml`, then `SPARKY_*` env vars | `sparky.toml`, `.env.example` |
| Build, gate | `just` recipes, pre-commit hook, CI | `justfile`, `.githooks`, `.github/workflows` |
| Deploy | Docker Compose, prod pulls GHCR images; one RunPod GPU pod under supervisord, without run_sandbox | `deploy/` |

## Rules

- Open models only, served by `llama-server` behind an OpenAI-compatible HTTP API.
- Facts come from retrieval or from a live source query, never from model weights.
- The engine never fetches a page for the index. Platform crawls ASU pages, writes the retrieval index, and runs live queries.
- The engine opens one SQLite file for its own conversations, memories and confirmations; every other store is the Platform HTTP API.
- Every request carries its own `RequestContext`. There is no global mutable state.
- Every replaceable dependency is a trait in `engine/src/core/traits` with a test double in `core/tests/support`.
- The harness owns the loop, policy, context assembly, memory, and tracing. Provider JSON never leaves `runtime/model`.
- Model output is never written back as retrieval evidence.
- An action at or above `policy.confirm_from` is held for the caller's approval immediately before it runs.
- Credentials, cookies, and authenticated page content never enter the retrieval index, memory, or traces.

## Layout

```
apps/
  engine/         Rust bin. The agent, its HTTP surface, and the store adapters.
    src/core/       config (services, integrations, http, harness by domain), telemetry, types, traits, tests
    src/runtime/    harness (loop, prompt, uploads, memory, safety, compaction, trace), model (Rig client, slot limit, server props), tools (knowledge/search with sources/, account for per-user Outlook and Gcal, papers, wiki, files, mcp, sandbox, shared http)
    src/stores/     platform/: client, conversation, confirmation, memory, profile, retrieval, query, accounts, health over the Platform HTTP API
    src/routes/     chat (JSON and SSE), confirm, conversation, profile, oauth, sandbox, health, rate limit, auth, failure
    src/wiring/     builds every dependency from config and serves: platform stores, tools, agent, http, boot checks, system prompt
  discord/        Rust bin. serenity bot and HTTP client of the engine. Never links it.
    src/core/       config, telemetry, types, tests
    src/bot/        client, addressed messages, turn destinations, memory and login commands, approvals, replies, the streamed turn
    src/engine/     HTTP client of the engine and SSE frame parsing
    src/render/     the turn card, reply text, buttons and their custom ids
    src/access/     roles, permissions, where a turn is answered, and what a message carries
    src/analytics/  product events, one exported span each
  cli/            Rust bin sparky. Developer console: runs just recipes and compose services and tails them.
    src/core/       config, types, tests
    src/app/        console state, key map, control, rendering
    src/units/      unit catalog, process runner, output parsing, log buffers, health probes, engine sandbox
  evals/          Python. Evals with a baseline gate.
    core/           settings, types, tests
    cli.py          eval run, baseline, compare
    runner.py       loads cases and runs them against the engine /chat
    suites/         one scorer per suite, SUITES in __init__.py
    cases/          golden cases, one JSON object per line
  web/            Vite and React frontend and admin UI
deploy/           compose (dev and prod), Dockerfiles, inference, monitoring, sandbox, runpod
docs/             ROADMAP.md, this file
.sparky/          ignored local state: traces, logs, eval reports
```

`apps/evals` is its own importable package (`evals`) with no `src/` layer. `pyproject.toml` maps the package to `.` and lists its subpackages, so a new subpackage is added there.

Every app has a `core/` holding config, telemetry, data types, interfaces, and tests. Domain modules import from it; it imports nothing from them. Language is never a folder, and neither is ASU domain (library, events): that is a module in `runtime/tools/knowledge/search/sources/` and an entry in the Platform query registry.

## System context

```mermaid
flowchart LR
    U["Student or moderator"] --> DC["Discord"]
    DEV["Developer"] --> CLI["sparky console"]

    subgraph apps ["apps"]
        BOT["discord"]
        ENG["engine"]
    end

    DC --> BOT
    BOT -->|"HTTP and SSE"| ENG
    CLI -->|"just, docker compose"| apps

    ENG -->|"completions"| CHAT["llama-server chat"]
    ENG -->|"embeddings"| EMB["llama-server embed"]
    ENG <-->|"stores, knowledge, live queries, accounts"| PLAT["Platform HTTP API"]
    ENG -.->|"platform tools"| PMCP["Platform MCP"]
    ENG -.->|"optional"| MCP["MCP servers"]
    ENG -.->|"run_sandbox"| SBX["sandboxd<br/>sandbox containers"]

    PLAT --> SITES["ASU sites and APIs"]

    BOT -->|"spans, product events"| OBS["Phoenix"]
    ENG -->|"spans"| OBS
```

Processes talk only at these edges. Platform ([asusoda/platform](https://github.com/asusoda/platform)) is a separate service: it holds the knowledge index, the live query registry, and linked accounts, and it alone crawls and queries ASU sites. Beyond Platform, the engine calls only the Microsoft Graph, Google Calendar, Semantic Scholar, and MediaWiki APIs and the attachment hosts in `agent.file_hosts`; with `sandbox.egress` on, a sandbox command reads public pages through the egress proxy. Other MCP servers are optional and none is configured by default.

The engine serves `/chat` and `/chat/stream` for the bot; both run the same loop. Every route except health and `/web/*` requires the bearer token in `SPARKY_ENGINE__SERVICE_TOKEN` or, where the web sign-in below allows it, a session cookie; and every route that carries a user applies the per-user `http.rate_limit_per_min`. The sandbox routes carry no user and are gated by the token alone, so any holder of it, `apps/discord` included, can read and stop the sandbox: `GET /sandbox` reports what is running, `DELETE /sandbox/{name}` removes one session container, and `POST /sandbox/enabled` stops the agent being offered `run_sandbox`. `apps/cli` shows and drives the containers through them, since the engine starts the containers outside compose.

### Web sign-in

With `[web] enabled`, a browser signs in with Discord instead of holding the token. `GET /web/login` sets a short state cookie and sends the browser to Discord with the scopes `identify guilds`. `GET /web/callback` checks the state, trades the code through `DiscordIdentity` (`stores/discord.rs`), and admits only members of `discord.guild_id`; a member whose guild permissions include Manage Server or Administrator gets the first `policy.write_roles` entry as a role. The session is the user id, display name, roles and expiry, sealed with XChaCha20-Poly1305 under `SPARKY_WEB__SESSION_KEY` into an `HttpOnly; Secure; SameSite=None` cookie that lasts `web.session_hours`. `GET /web/me` returns it and `POST /web/logout` clears it.

`/chat`, `/chat/stream`, `/chat/confirm`, `/conversation/reset` and `/memories/*` take either the bearer token or the session cookie. A cookie counts only when the request `Origin` is `SPARKY_WEB__SITE_URL`, which stands in for a CSRF token. With a session, the engine replaces the user, roles and tenant in the body with the session's, and runs the turn as private on channel `web`. `apps/web` serves the client: `/chat` streams `/chat/stream` and shows the progress lines, the answer with its sources and used memories, and approval buttons; `/memories` lists and forgets. Both call the engine at `VITE_ENGINE_URL` with `credentials: include`. CORS allows credentials only while the web sign-in is on, and `Config::validate` then requires the site in `http.cors_origins` and refuses `*`.

### Admin surface

`/admin/*` (`routes/admin.rs`) takes the bearer token or a web session whose roles hold the first `policy.write_roles` entry; any other session gets 403. `GET /admin/tools` lists every registered tool and whether it is offered, and `POST /admin/tools/{name}` with `{on}` switches one for the next turn: `ToolSet` keeps the switched-off names in a set its clones share, so `get` and the definitions leave the tool out. `GET` and `PUT /admin/instructions` read and replace the system instructions (`text: null` returns to `prompt.system`), at most `admin.max_instructions_chars`. Each write is a row in `admin_changes` (migration 0004) with the actor's user id; `GET /admin/history` lists the newest `admin.history_limit`, and `POST /admin/rollback/{id}` writes that row's value again. The newest row per kind and key is in force, and the engine applies them at boot. `GET /admin/limits` shows the settings that bound load. `GET /admin/approvals` lists held actions by user, tool and expiry without their arguments, and `POST /admin/approvals/{token}/deny` denies one. The store is `AdminStore` (`stores/sqlite/admin.rs`); with `store.backend = "platform"` the routes that need it answer 501.

The console starts and stops the other units and tails their output. It does not start a process whose port is already served, and says so instead.

## Inside engine

```mermaid
flowchart TD
    ROUTES["routes and wiring<br/>compose everything, own main"]
    HARNESS["runtime::harness<br/>agent: run, step, inputs, execute, conclude, task, uploads<br/>agent/call: thinking, relay, draft, thought, retry, spans<br/>agent/prompt: assemble, capability<br/>safety: guardrail, policy<br/>compact, tools, trace"]
    MODEL["runtime::model<br/>rig_openai: chat, embed, convert<br/>limit, props"]
    TOOLS["runtime::tools<br/>knowledge/search: live, stored, sources/ one file per source<br/>account: outlook, gcal, grant<br/>papers, wiki, files, mcp, http<br/>sandbox: container, session, egress, output"]
    STORES["stores<br/>platform: client, conversation, confirmation, memory, retrieval, query, accounts, health"]
    CORE["core<br/>config, telemetry, types, traits, tests"]

    ROUTES --> HARNESS
    ROUTES --> MODEL
    ROUTES --> TOOLS
    ROUTES --> STORES
    ROUTES --> CORE
    HARNESS --> CORE
    MODEL --> CORE
    TOOLS --> CORE
    STORES --> CORE
```

`core` imports nothing else in the crate. `runtime::harness`, `runtime::model`, `runtime::tools`, and `stores` import only `core`. `routes` and `wiring` compose them. `scripts/check-deps.sh` enforces that the three Rust apps never depend on each other.

Folders nest by domain, and the same domain names repeat across `core/types`, `core/traits`, `core/tests`, `runtime`, and `stores`: agent, conversation, http, knowledge, memory, model, safety, tools, trace. A domain with one file at a level keeps that file flat. Data lives in `core/types`, interfaces in `core/traits`, and stateful objects beside their implementations.

`runtime::model::limit` wraps the chat client in a semaphore of `agent.model_slots` permits, matching `llama-server --parallel`. A call waits up to `agent.model_queue_wait_secs` for a slot, then fails as busy.

## Platform stores

`wiring/platform.rs` builds a `Stores` value of trait objects over `stores/platform/`, and `wiring/store.rs` moves conversations, memories and confirmations to the local store; the harness, tools, routes, and `core` see the traits alone. `SPARKY_PLATFORM__URL` and `SPARKY_PLATFORM__TOKEN` are required.

| Store | Platform |
|---|---|
| `ConversationStore`, `MemoryStore`, `ConfirmationStore`, only with `store.backend = "platform"` | `/api/agents/members/{user_id}/...` |
| the one-time import | `GET /api/agents/export/conversations` and `/memories` |
| `Retriever` | `POST /api/knowledge/search`, with the query vector from the engine `Embedder` when `retrieval.dense` is on |
| `SourceQueries` | `GET /api/asu/queries`, `POST /api/asu/query`, run while the call waits, bounded by the request deadline |
| `OAuthStore` | `/api/accounts/members/{user_id}/{provider}/token` and `/login`; Platform runs the login and refreshes tokens |
| readiness `Probe` | `GET /health` on Platform |

The engine needs the chat and embedding servers and Platform. Platform takes the organization from the machine token (`Authorization: Bearer plat_...`), so `tenant_id` is not sent, and the member is `user_id`, which Platform requires to be a numeric Discord user id. One engine serves one guild, and its token belongs to the Platform organization of that guild. The token needs the scopes `agents:read`, `agents:write` (the import and `store.backend = "platform"`), `knowledge:read`, `accounts:link`, `accounts:token`, and `asu:read`.

With `platform.mcp_url` set, the engine also registers the Platform MCP tools with the same token (`runtime/tools/mcp.rs`, `connect_platform`). Each tool is named `platform_<name>` with dots as underscores; `platform.mcp_tools` limits the list, and the token scopes decide which tools Platform offers. The risk comes from the MCP annotations: `readOnlyHint` is `ReadPublic`, `destructiveHint` is `Destructive`, and any other tool is `ExternalWrite`, so `Policy` requires `policy.write_roles` and a confirmation for every write. The Platform `confirm` argument is removed from the schema the model sees; the tool sends `confirm=true` after `Policy` confirmed the call.

A Platform tool whose name starts with a prefix in `platform.mcp_skip` (default `canvas.`) is not registered; the engine serves Canvas itself (see Canvas). A Platform tool whose name starts with a prefix in `platform.mcp_private` (default empty) returns one member's own data. A read-only one is `ReadAuthenticated`, the tool refuses outside a direct message, and its `discord_id` argument is removed from the schema the model sees; the call sends the caller's `user_id` in its place, so a member reads only their own account (`mcp::Fill`).

## Local store

`stores/sqlite/` holds `ConversationStore`, `MemoryStore`, `ConfirmationStore`, `CanvasFeeds` and `AdminStore` over one SQLite file (`store.path`), with the migrations in `apps/engine/migrations/` embedded and run at boot. Every row has `tenant_id` and `user_id`, and every query binds both from the `RequestContext`. A memory marked sensitive is sealed with XChaCha20-Poly1305 under `SPARKY_MEMORY__KEY`; a row that does not open is left out of recall and logged by id. A claim is one `UPDATE ... RETURNING`, so a confirmation is answered once. With `store.backup_dir` set, a `VACUUM INTO` copy is written every `store.backup_every_hours` and the newest `store.backups_kept` are kept. A readiness `Probe` named `store` runs `SELECT 1`. Every `store.prune_every_hours` the store deletes expired memories, answered or expired pending actions, and, when `store.conversation_idle_days` is above 0, conversations idle that long with their messages.

`engine import-platform` copies the Platform export once: conversations updated in the last `store.import_days` days with their messages, each summary's `covers` rewritten to the new positions, and every memory, sealed when Platform marked it sensitive. The `imports` table refuses a second run unless `--again` is passed.

What Platform serves beside the stores:

| Part | Platform |
|---|---|
| ASU knowledge and live queries | the `asu` pack: crawling of ASU pages, SearXNG web search, and a shared Playwright browser for JS pages; `search_live` offers only the sources the Platform registry publishes, plus `tools.live_default_source` |
| Sun Devil Central clubs and events behind an ASU sign-in | `platform_asu_clubs` and `platform_asu_events` (scope `asu:read`), over the organization's ASU sign-in |
| `/login` and `/logout` | the Platform accounts link (`accounts:link`) and grant removal |
| Outlook and Google Calendar | engine tools over tokens from Platform (`accounts:token`) |

## Types

```rust
pub struct RequestContext {
    pub request_id: Uuid,
    pub tenant_id: String,
    pub user_id: String,
    pub roles: Vec<String>,
    pub conversation_id: Uuid,
    /// Public when anyone but the caller can read the answer.
    pub visibility: Visibility,
    pub deadline: Instant,
    pub cancel: CancellationToken,
    /// Live progress goes here while a caller is watching; None for a plain request.
    pub progress: Option<UnboundedSender<Progress>>,
}

pub struct Evidence {
    pub source_id: Uuid,
    pub chunk_id: Uuid,
    pub key: String,
    pub title: String,
    pub content: String,
    pub url: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub score: f32,
}
```

Citations are built from the pages the tools returned (`Answer.sources`), never parsed out of generated text. A `Citation` has a `title` a client shows (the stored page title, or the site name from `LiveSource::label`) and a `key` naming the source, which is what a caller matches on.

## Traits

All in `engine/src/core/traits`. Inputs and outputs are owned types from `core/types`. The main ones:

```rust
#[async_trait]
pub trait ModelProvider {
    async fn generate(&self, ctx: &RequestContext, req: ModelRequest) -> Result<ModelResponse, ModelError>;
    // The same completion, sending each reasoning and text piece to deltas as it arrives.
    async fn stream(&self, ctx: &RequestContext, req: ModelRequest, deltas: UnboundedSender<ModelDelta>) -> Result<ModelResponse, ModelError>;
}

#[async_trait]
pub trait Tool {
    fn definition(&self) -> ToolDefinition;   // name, description, JSON schema, risk, sequential, timeout
    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError>;
}

#[async_trait]
pub trait Retriever {
    async fn retrieve(&self, ctx: &RequestContext, query: &RetrievalQuery) -> Result<Vec<Evidence>, RetrievalError>;
}

#[async_trait]
pub trait Embedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RetrievalError>;
    fn dim(&self) -> usize;
}

#[async_trait]
pub trait SourceQueries {
    async fn sources(&self) -> Result<Vec<QuerySourceInfo>, QueryError>;
    async fn run(&self, ctx: &RequestContext, request: &QueryRequest) -> Result<QueryOutcome, QueryError>;
}

#[async_trait]
pub trait ConversationStore {
    async fn ensure(&self, ctx: &RequestContext, channel_id: &str) -> Result<(), StoreError>;
    async fn owns(&self, ctx: &RequestContext) -> Result<bool, StoreError>;
    async fn load(&self, ctx: &RequestContext, limit: usize) -> Result<Vec<Message>, StoreError>;
    async fn append(&self, ctx: &RequestContext, turns: &[Message]) -> Result<(), StoreError>;
    async fn latest(&self, ctx: &RequestContext, channel_id: &str) -> Result<Option<Uuid>, StoreError>;
    async fn end(&self, ctx: &RequestContext, channel_id: &str) -> Result<u64, StoreError>;
}

#[async_trait]
pub trait MemoryStore {
    async fn recall(&self, ctx: &RequestContext) -> Result<Vec<Memory>, StoreError>;
    async fn remember(&self, ctx: &RequestContext, memory: &NewMemory) -> Result<Memory, StoreError>;
    async fn forget(&self, ctx: &RequestContext, id: Uuid) -> Result<bool, StoreError>;
    async fn forget_all(&self, ctx: &RequestContext) -> Result<u64, StoreError>;
}

#[async_trait]
pub trait Policy {
    async fn authorize(&self, ctx: &RequestContext, action: &ProposedAction) -> Decision;
    // Decision: Allow | Deny { reason } | Confirm(ConfirmationRequest)
}

#[async_trait]
pub trait Guardrail {
    async fn check(&self, ctx: &RequestContext, stage: Stage, text: &str) -> Verdict;
}

pub trait TraceSink {
    fn emit(&self, ctx: &RequestContext, event: TraceEvent);
}
```

The rest follow the same pattern: `Compactor`, `ConfirmationStore`, `Sandbox`, `FileSource`, `Outlook`, `GoogleCalendar`, `OAuthStore`, `Probe`. `OAuthStore` loads a caller's linked grant, removes it, and hands out the Platform `login_link`.

## Request lifecycle

```mermaid
sequenceDiagram
    actor U as Student
    participant B as discord
    participant E as engine
    participant P as Platform
    participant M as llama-server

    U->>B: @mention, a reply in the thread, or a direct message
    B->>B: resolve roles, destination, visibility
    B->>E: POST /chat/stream with bearer token and traceparent
    E->>E: ensure conversation, load recent turns from the SQLite file
    opt history over its budget
        E->>M: chat agent compacts the oldest turns
    end
    E->>E: recall memory from the SQLite file, private turns only
    loop one step per model call
        E->>M: streamed completion with tool schemas
        M-->>E: reasoning, text, tool calls
        E-->>B: SSE progress, thinking line, answer draft
        opt search_knowledge call
            E->>M: embed the query
            E->>P: knowledge search
        end
        opt search_live call
            E->>P: live query
            P-->>E: result text
        end
    end
    E->>E: append the new turns to the SQLite file
    E-->>B: SSE answer, then done
    B->>U: final card with answer, sources, memories
```

1. `discord` posts an addressed message to the engine with the Discord identity, role names, the answering channel, its visibility, `continue_channel`, and the bot message a reply answers. A question is never a slash command; the remaining commands only read and clear memory.
2. The engine checks the bearer token and the per-user rate limit, then builds a `RequestContext`. A given `conversation_id` must belong to the caller. Without one, `continue_channel` continues the caller's newest open conversation in that channel at that visibility; otherwise a new conversation starts.
3. The loop loads inputs once: the last `agent.history_turns` turns, compacted if they overflow the history budget, and, in a private turn only, memories. A failed memory read is logged and the turn runs without it. Nothing is retrieved before the first model call: the model calls `search_knowledge` and `search_live` as the question needs, several in one step when it words the subject more than one way.
4. The loop runs steps until it stops. On `/chat/stream` each progress event goes out as an SSE frame. A turn about to answer after a tool failed, with `run_sandbox` registered and untried, is handed back once with `prompt.sandbox_retry_line`.
5. Every exit appends what was said and builds the answer with citations, tool runs, memories (empty for a public answer), usage, and cost. Tool calls and results are not stored in history.
6. An answer with status `AwaitingConfirmation` carries a token. `POST /confirm` with that token runs the held action and resumes the loop from its result.

## Prompt assembly

Each step assembles the prompt in a fixed order:

1. System instructions, the role line, and today's date.
2. The capabilities section, if it fits `agent.capabilities_budget_tokens`.
3. Memory, within `agent.memory_budget_tokens`.
4. History, newest first within `agent.history_budget_tokens`, never starting on an orphaned tool result.
5. The user's input, then the model and tool turns of this request.

Tool schemas are sent beside the messages and are charged to `agent.prompt_budget_tokens` first; history is capped by what remains. The system prompt, the input, and this request's tool exchange are never dropped. When this request's tool results outgrow the room left after the fixed sections and the input, each is cut to a share of it, smallest first, ending in `prompt.result_cut_line`. Token counts are estimates from `agent.chars_per_token`. MCP schemas are compacted and by default show only required properties.

## Agent loop

```mermaid
flowchart TD
    TOP(["next step"]) --> LIMIT{"cancelled, past deadline,<br/>or max_steps reached?"}
    LIMIT -->|yes| STOP(["Cancelled, Deadline, or StepLimit"])
    LIMIT -->|no| ASM["assemble prompt"]
    ASM --> THINK{"thinking decision"}
    THINK --> CALL["model call<br/>no tools when an answer is forced"]
    CALL --> RELAY["relay while streaming<br/>reasoning line, answer draft blocks"]
    RELAY --> SPENT{"thought, but no answer<br/>and no tool call?"}
    SPENT -->|yes| PLAIN["same call without thinking"]
    SPENT -->|no| GUARD
    PLAIN --> GUARD{"guardrail passes?"}
    GUARD -->|no| BLOCKED(["Blocked, replacement text"])
    GUARD -->|yes| CALLS{"tool calls?"}
    CALLS -->|no| ANSWER(["Answered"])
    CALLS -->|yes| POLICY{"Policy::authorize<br/>each call"}
    POLICY -->|Confirm| HOLD(["AwaitingConfirmation"])
    POLICY -->|"Deny or unknown tool"| FEED
    POLICY -->|Allow| REPEAT{"same name and arguments<br/>already run?"}
    REPEAT -->|"some calls are new"| EXEC["run new calls with timeouts<br/>parallel unless one is sequential"]
    REPEAT -->|"all repeats, first time"| FORCE["force an answer on the next step"]
    REPEAT -->|"all repeats, already forced"| STALLED(["Stalled"])
    EXEC --> FEED["append tool results as messages"]
    FEED --> TOP
    FORCE --> TOP
```

The loop owns every stopping condition and checks cancellation, the deadline, and `agent.max_steps` before each step.

**Thinking** is decided per model call by `agent.thinking`. `on` and `off` apply to every call. In `auto` the first matching rule wins: a call with no tools does not think; a step after a tool call that failed or returned nothing thinks when `recover` is set; a question with a cue word or longer than `max_quick_chars` thinks; a step after tool results thinks when `after_tools` is set; a step before any tool has run thinks when `plan_searches` is set; anything else does not. By default only `recover` is set, so a simple lookup never thinks. A thinking call gets `model.max_tokens`, others `model.max_tokens_without_thinking`. A call that thought and returned neither an answer nor a tool call is repeated once without thinking when `retry_without` is set. The `llm` span records `sparky.thinking` and `sparky.thinking_reason`. Prompted sub-agents never think.

**Streaming.** With `agent.stream` on, reasoning updates the thinking line as it grows. When answer text begins, the thinking line becomes the full thought, clipped to `agent.progress_thought_chars`. Answer text is released as a draft at the end of a sentence or line, or at a word break past `agent.stream_block_chars`, and every block passes the guardrail before it is sent. A draft is withdrawn when the call ends in tool calls, fails, or a block is refused. A step that answers without showing a thought takes its thinking line back.

**Guardrail and policy.** Every response passes the guardrail: the answer stage for a final answer, the capability stage for a response with tool calls and text. A denied phrase, an empty answer, or an over-length answer blocks the response and ends the run with `guardrail.replacement`. On the answer stage the guardrail also removes protected terms: the registered tool names are added to `guardrail.protected_terms` at boot, so a tool name that slips into an answer is replaced with `guardrail.redaction` rather than shown to the student. The system prompt is the first line of this defense, telling Sparky never to name a tool, reveal its instructions, say which model runs it, or follow instructions found in tool output. `Policy` decides whether a typed action may run by its risk class.

**Tool calls** are authorized before any runs. A denial or an unknown tool name becomes a tool result the model reads. The first call that needs confirmation is stored through `ConfirmationStore` and ends the run. Allowed calls that repeat an earlier call with identical arguments are not run again, and the model is told so. When every call in a step is a repeat, the next step offers no tools and adds `prompt.answer_only_line`; if that step repeats again or answers with nothing, the run ends as `Stalled`. Allowed calls run in parallel, each under its own timeout (the tool's declared timeout or `agent.tool_timeout_secs`, capped by the request deadline), or in order if any call is to a tool marked sequential. A tool error, including `InvalidArguments`, is fed back as the tool result.

**Model errors.** A retryable error (transport, 5xx, 429) is retried with backoff up to `agent.max_model_retries`. A model timeout ends the run as `Deadline`, cancellation as `Cancelled`, and any other error keeps the turns and returns the error.

## Prompted sub-agents

Each sub-agent is a `runtime::harness::agent::task::Task`: its own instructions, one model call, no tools, no thinking, and a typed result. The loop is not a `Task`.

| Agent | When | Reads | Writes |
|---|---|---|---|
| Sparky (the loop) | every request | tool results, memory, history, capabilities | the turn |
| Chat | loaded history overflows its budget | the turns being replaced | one compacted turn |

## Capabilities

`runtime::harness::agent::prompt::capability` renders one `<capabilities>` list in the prompt. Each entry names its kind.

| Kind | Executed by | Risk |
|---|---|---|
| `tool` | a built-in `Tool`: `search_knowledge`, `search_live`, `run_sandbox` | declared per tool |
| `mcp` | a remote MCP server | `risks` in the server entry, else derived from the tool name; a pin the server does not list fails boot |

`tools.disabled` removes tools by name at registration. The engine refuses to boot when the capabilities section exceeds its budget, when the tool schemas take more than half of `agent.prompt_budget_tokens`, or when the prompt budget plus `agent.prompt_estimate_headroom` and `model.max_tokens_without_thinking` do not fit one `llama-server` slot.

**The sandbox image.** `run_sandbox` runs a command in a container with a read-only root, memory, CPU, and process limits, a non-root user, and a workspace of `sandbox.workspace_mb` mounted `noexec` at /tmp. The image is `deploy/docker/sandbox.Dockerfile`: python3 with requests, bs4, lxml, pandas, numpy, pypdf, python-docx, openpyxl, xlrd, Pillow, chardet, yaml and markdown; pdftotext, pdftoppm and tesseract; curl, wget, jq, ripgrep, sqlite3 and the usual text tools. The live test `the_image_carries_python_jq_and_the_document_readers` checks every one.

**Network.** With `sandbox.egress` off the container runs with `--network none`. With it on, the container joins `sandbox.egress_network`, which the engine creates `--internal` at boot (and refuses to use if it exists and is not internal), with `HTTP_PROXY` and `HTTPS_PROXY` pointing at `sandbox.egress_proxy_name`. That proxy (`deploy/docker/sandbox-proxy.Dockerfile`, `deploy/sandbox/squid.conf`) is the only container on both that network and the outside. It allows ports 80 and 443 and refuses private, loopback, link-local and reserved destinations, so a command reaches public sites and never the engine, the host, or cloud metadata. HTTPS passes through as a tunnel. What the sandbox reads is never indexed.

**Sessions.** A call that names a session reuses its container, so workspace files persist between calls. Session containers are named from a hash of the tenant and user, so one caller cannot reach another's session. A container idle for `sandbox.session_idle_secs` is removed; a caller holding `sandbox.max_sessions` loses their least recently used one; `sandbox.max_sessions_total` caps session containers across callers, reaping the least recently used of all. `sandbox.max_running` caps commands running at once; a command past it waits for a slot within its budget. Every command runs under `timeout -s KILL` inside the container. stdout and stderr are read as they arrive and only a head and tail of each is held.

**Lifecycle.** Every container the engine starts, and the egress proxy, carries the `sparky.sandbox` label. `just sandbox-down` removes them by that label, and `just down` runs it first. The recipe reads `.env` so `DOCKER_HOST` points at the runtime the engine drives (`sandboxd` under compose). Containers also carry `sparky.sandbox.instance` (`sandbox.instance`); at boot and on every idle sweep the engine removes session containers of its instance it is not tracking. The engine keeps the last `sandbox.recent_commands` commands, the running one included. Each carries an id, is redacted like a traced tool argument, and is reported once while it runs and once when it ends; a call the loop cancelled is recorded as ended with no exit status. `POST /sandbox/enabled` takes the tool out of `ToolSet::definitions`, so the schema, the capabilities section, and the sandbox hand-back all stop naming it.

**Runtime.** Under compose the runtime is the `sandboxd` service, a daemon of its own reached over `DOCKER_HOST`; on a developer host it is the local `docker`. It is probed once at boot: `sandbox.required` decides whether an unreachable one fails boot or leaves `run_sandbox` unregistered.

A tool result longer than `agent.tool_result_to_file_chars` is written to the workspace and replaced by its head plus the path; `0` turns this off. The full result still reaches the trace, and a long live listing reaches the workspace whole.

### Resource limits

Everything that could grow with load has a ceiling in `sparky.toml`:

| What | Bound |
|---|---|
| Turns in flight | `agent.max_turns`; a turn waits up to `agent.turn_queue_wait_secs`, then gets 503 busy. The permit is held for the whole turn, detached streaming turns included, so attachments and tool results held in memory are bounded with it |
| Model calls | `agent.model_slots` |
| Sandbox | `sandbox.max_sessions` per user, `sandbox.max_sessions_total` overall, `sandbox.max_running` commands, `sandbox.memory` and `sandbox.workspace_mb` (a tmpfs) each; sandboxd's `mem_limit` bounds them together |
| Attachments | `agent.max_files` of `agent.max_file_bytes`, streamed with the cap enforced |
| Live results | `query.timeout_secs` per query; past `agent.tool_result_to_file_chars` the rest goes to the workspace |
| Containers | a `mem_limit` on every compose service, overridable with `SPARKY_MEM_<SERVICE>` |
| Store | `store.max_connections` pooled connections to the SQLite file; a write waits up to `store.busy_timeout_ms` for a lock |
| Memories | `memory.max_per_user` per student, each at most `memory.max_chars`; at the cap the oldest fact goes, then the oldest preference, then the oldest task |

What is kept on disk is pruned: JSONL traces after `trace.retention_hours`, checked hourly, each capped at `trace.max_file_bytes`; CLI unit logs rotated at `cli.log_file_max_mb` to one `.1` file, lines cut at `cli.log_line_chars`. Memories stay until a student forgets them or a task passes its due date. Conversations stay until `store.conversation_idle_days` of idleness when that is above 0.

### Attached files

`discord` forwards every non-image attachment, up to `bot.max_files` of at most `bot.max_file_bytes`, as `files` on the chat request. The engine takes up to `agent.max_files` of at most `agent.max_file_bytes` into `RequestContext.files`.

Before the first model call, `uploads.rs` downloads each through `FileSource` (`HttpFiles`: HTTPS only, hosts in `agent.file_hosts`, no redirects, the byte cap enforced while streaming) and writes it into the conversation's sandbox session as `upload-<name>`. One command there turns it into text at `<path>.txt`: pdftotext for a PDF, tesseract over its first `agent.upload_ocr_pages` pages when it has no text layer, python-docx, openpyxl and pandas for Word, Excel and CSV. The same command ranks the passages by how many words of the question each holds.

The prompt gets one `prompt.upload_line` per file, just before the question: name, size, paths, session, the first `agent.upload_preview_chars` of its text, and the best passages up to `agent.upload_match_chars`. A file with no text gets `prompt.upload_raw_line`; one that could not be downloaded or written gets `prompt.upload_failed_line`. Each emits `TraceEvent::FileAttached`. The file stays in the session until the session idles out.

## Compaction

Without compaction, history is trimmed to its budget by dropping the oldest turns. With `compaction.enabled`, compaction runs when the loaded history, a stored summary included, is over `agent.history_budget_tokens`. It keeps the newest turns within `compaction.keep_share` of that budget, moved forward to start on a user turn, and the Chat agent replaces everything older, the previous summary included, with one stored compacted turn. Boot rejects a `keep_share` whose kept turns plus `compaction.max_tokens` exceed the history budget. Assembly places a leading summary before trimming the turns after it. A failed compaction falls back to trimming.

A compacted turn is stored with role `summary`. It is never retrieval evidence, and the turns it replaced stay stored. A summary records `covers`, the `seq` of the last message it replaces; loading history reads the newest summary and then up to `agent.history_turns` messages with a greater `seq`. The kept turns are stored before the summary, so they load again. A summary without `covers` covers every message before its own `seq`.

The system prompt is not a stored message; it is rebuilt on every request and always comes first.

## Memory

| Kind | Content | Where |
|---|---|---|
| Working | the current request | not persisted |
| Conversation | what was said, the question and the answer | Platform conversation messages |
| Fact, preference, task | one short line a student stated about themselves | the engine store, written by `remember` |

The model writes memories in the normal loop, so a turn costs no extra model call. `remember` takes a kind (`fact`, `preference`, or `task` with a due date), one line of at most `memory.max_chars`, and an optional sensitive flag. It refuses text with a term in `memory.refuse_terms` and seals text with a term in `memory.sensitive_terms`. `forget_memory` takes the short id the memory section shows. Both are `PrepareWrite`, private and sequential, and are registered when `memory.tools` is on and `store.backend` is `sqlite`. `prompt.memory_rules` tells the model what to keep and what never to store; it is added to the system prompt only in a private turn where the tools are registered. `prompt.discovery_rules` tells the model to use the student's memories (major, interests, clubs, campus) as the words of a `search_live` query on events or clubs when they ask for something to do or join, and to say which memory it used; it is added only in a private turn where `search_live` is registered. Sparky sends nothing on its own.

Recall reads every unexpired memory: tasks due within `memory.task_soon_days` first, soonest first, then the rest newest first. The prompt lists them as `[short id] kind: text (date)` until `agent.memory_budget_tokens` is spent. A task expires at the end of its due day in `prompt.utc_offset_hours`.

A public request recalls no memory and is offered no private tool. Only an answer no one else can read is private. A public answer never lists the memories its prompt carried.

Students see their memories with `/memories` (`POST /memories/list`, numbered in recall order) and remove them with `/forget <number>` or `/forget all` (`POST /memories/forget`).

## Tool risk classes

| Class | Examples | Default behavior |
|---|---|---|
| `ReadPublic` | `search_knowledge`, `search_live` | run |
| `ReadAuthenticated` | `canvas_deadlines`, `outlook_mail`, `google_calendar`, a page inside the user's own session | allowed in a direct message; elsewhere deny unless `policy.allow_authenticated_reads` |
| `PrepareWrite` | `run_sandbox`, `remember`, `forget_memory`, a draft, a form filled without submitting | run |
| `ExternalWrite` | post, create a ticket, book, submit | require a `policy.write_roles` role, then confirm |
| `Destructive` | delete, cancel | require a `policy.write_roles` role, then confirm |
| `Forbidden` | another user's session, bypassing policy | deny |

The classes are ordered as listed. `policy.write_roles` gates `ExternalWrite` and above. `policy.confirm_from` names the lowest class held for approval. `Forbidden` is denied regardless of settings. Defaults are `["MANAGE_GUILD"]`, `external_write`, and authenticated reads off. `ReadAuthenticated` is also allowed whenever the exchange is private (a direct message), so a user's own data is read only in their own channel, never in a shared server.

A tool with `private` set in its definition is personal to the user. A public turn is not offered it, and the policy denies a call to it outside a private turn whatever the other settings allow.

## Canvas

Canvas is one engine tool, `canvas_deadlines` (`runtime/tools/account/canvas/`), over each student's Canvas calendar feed, on while `[canvas] enabled` and `store.backend` is `sqlite`. It lists assignment due dates and course events for the next `days` (default `canvas.default_days`, at most `canvas.max_days`), soonest first, at most `canvas.max_items`. It reads no grades, logs in nowhere, and runs no browser.

A student runs `/canvas link` in Discord, which opens a private form, and pastes the Calendar Feed link from Canvas (Calendar, then Calendar Feed). The bot posts it to `POST /canvas/feed`; the link never passes through the model, a conversation, or a trace. `feed_link` (`core/types/tools/canvas.rs`) accepts only an https link on a `canvas.hosts` host, with no user name, password or port, and a path under `/feeds/calendars/user_` ending in `.ics`. `CanvasFeeds` keeps one link per Discord user in the `canvas_feeds` table, sealed with XChaCha20-Poly1305 under `SPARKY_MEMORY__KEY`. `/canvas unlink` calls `POST /canvas/feed/remove`.

On a call, the tool opens the caller's link and reads the feed once through `CanvasCalendar` (no redirects, `canvas.timeout_secs`, at most `canvas.max_bytes`). `parse_ics` reads each event's title, course, start and link. Nothing read from the feed is stored, indexed or memorized. The tool is `ReadAuthenticated` and `private`, so it is offered and allowed only in a direct message, and traces and Phoenix record only the length of its result (see Tracing).

## Linked accounts and integrations

Platform runs every per-user login and holds the grants. Canvas is the exception (see Canvas). `/login <service>` in Discord calls `POST /oauth/{provider}/authorize`, which returns the Platform login link the bot shows privately. The user signs in through the provider's own SSO; SparkyAI never sees a password. `/logout <service>` calls `POST /oauth/{provider}/logout`, which removes the Platform-linked grant. `Credentials` in `runtime::tools::account::grant` reads a caller's token for a provider from Platform, which refreshes an expired one.

`runtime/tools/account/outlook` offers read-only Outlook tools over Microsoft Graph, off until `[outlook] enabled`: `outlook_calendar` and `outlook_mail`, both `ReadAuthenticated` and direct-message only, keyed to the `microsoft` provider. Reading ASU accounts needs ASU admin consent on the Platform Microsoft app.

`runtime/tools/account/gcal` offers a read-only `google_calendar` tool over the Google Calendar API, off until `[gcal] enabled`: it lists the caller's upcoming events, is `ReadAuthenticated` and direct-message only, and is keyed to the `google` provider. The Calendar REST client sits behind the `GoogleCalendar` trait in `core/traits/tools/gcal.rs`.

`upcoming` (`runtime/tools/upcoming.rs`) lists what the caller has coming up in one call: remembered tasks, Canvas due dates, and Google and Outlook events, merged and sorted by date, at most `upcoming.max_items`, for the next `days` (default `upcoming.default_days`, at most `upcoming.max_days`). Each integration that has dated items adds an `UpcomingSource` (`core/traits/tools/upcoming.rs`) beside its tools; a source whose tool is in `tools.disabled` is left out, and the tool is registered only when `[upcoming] enabled` and at least one source is. The sources are read in parallel within the request deadline. A source that is not linked, fails, or runs out of time adds one line naming it. `upcoming` is `ReadAuthenticated` and `private`.

Two public, no-auth tools sit beside the ASU sources: `search_papers` (Semantic Scholar) and `wikipedia_lookup` (MediaWiki). They are `ReadPublic` and take a query; their parse functions are tested on canned bodies.

A confirmation is bound to a hash of the exact arguments, belongs to the caller who was asked, is single use, and expires after `agent.confirmation_ttl_secs`. Its summary names the tool, the arguments, and whether the action can be undone. The policy decision is recorded in the trace.

## Knowledge

Platform crawls ASU pages, chunks and embeds them, and serves the retrieval index. The engine reads it when the model calls `search_knowledge`.

```mermaid
flowchart TD
    Q["question"] --> DENSE{"retrieval.dense?"}
    DENSE -->|yes| EMB["embed with the engine Embedder"]
    DENSE -->|no| REQ
    EMB --> REQ["POST /api/knowledge/search<br/>query, vector, category,<br/>retrieval.top_k, retrieval.window"]
    REQ --> HITS{"any hits?"}
    HITS -->|"none in the category"| BROAD["the same search across every category"]
    BROAD --> HITS2{"any hits?"}
    HITS2 -->|no| EMPTY(["tools.nothing_stored"])
    HITS2 -->|yes| EV
    HITS -->|yes| EV["Evidence"]
```

A call that names a source filters by that source's category; when that finds nothing, the query runs again across every category. When nothing matches at all, the tool returns `tools.nothing_stored`. Each hit comes back with the `retrieval.window` chunks either side of it joined into one passage; `retrieval.window = 0` hands each hit back alone. The query vector is sent with `platform.embedding_model`, or `embedding.name` when that is empty, so the engine and Platform must embed with the same model and dimension. A `Citation` is the source title and URL, not an offset.

## Live source queries

A live query answers from a source now, with parameters the model picks: a term, subject and level of the class catalog, a scholarship search, study room slots on a date, the next shuttle at each stop, a building on the campus map, a web search, or this week's library hours.

The engine offers two search tools: `search_knowledge(query, source?)` reads the stored index and `search_live(query, source?)` runs a live query on Platform. `source` is an optional filter; each variant carries a few words from `LiveSource::hint`. `search_knowledge` offers only the sources Platform indexes and maps the key to the category it filters on. `search_live` with no source uses `tools.live_default_source`, which is `web`.

The query string is the whole request, and `tools.query_description` shows in a bad-then-good pair what a self-contained keyword query looks like. `search::params_for` turns the string into the source's parameters: the query fills the source's text parameter, a choice the query names fills a choice parameter, a `YYYY-MM-DD` fills a date, a required date falls back to today at `prompt.utc_offset_hours`, and `Courses::derived` reads the term out of the query or takes the one running today. A required parameter nothing filled comes back to the model naming what the query has to say. `LiveSource::narrow` cuts what a source's site cannot match out of the query first: `Courses` sends the class search the course code alone.

```mermaid
sequenceDiagram
    participant M as model
    participant T as engine search tool
    participant P as Platform
    participant W as source site or SearXNG

    M->>T: search_live with a query and a source
    T->>T: turn the query into the parameters the source takes
    T->>P: POST /api/asu/query
    P->>W: fetch the page or read the endpoints
    P-->>T: result text
    T-->>M: live result text and citation
    Note over T,P: the call is bounded by query.timeout_secs and the request deadline
```

Each source is one engine file, `runtime/tools/knowledge/search/sources/<source>.rs`. It declares the hint, the citation label, the category, the parameters, what each accepts (text, one of fixed choices, any of fixed choices, a date, a flag), whether its answers are stored (`LiveSource::freshness`), and any extra check. The query is turned into parameters and checked there, so a bad call is corrected without a Platform call. The two tools live in `live.rs` and `stored.rs`.

At boot the engine reads the Platform registry (`GET /api/asu/queries`) and fails to boot if it cannot. `search::conforms` logs a warning when a catalog source and the published one disagree on a parameter name, whether it is required, whether it takes a list, a choice, or whether it is stored. A source Platform does not publish is not offered to `search_live`. Platform checks every query's parameters again before running it.

A Platform refusal (unknown source, missing parameter, unreadable page) reaches the model as `InvalidArguments` with the reason. A caller that is cancelled or runs out of time drops the call. The page is cited in `Answer.sources`.

The `web` source is Platform's SearXNG search; it returns titles, links, dates, and snippets. `clubs` and the Sun Devil Central half of `events` are read over the organization's ASU sign-in on Platform and are never indexed. Not covered: X and Instagram posts, and anything behind an individual student's login (Workday jobs, personal MyASU data), which waits on the per-user sessions of Phase 8.

## Discord surface

| Entry | Where the answer goes | Visibility |
|---|---|---|
| mention in a text channel | a thread opened from the message, or inline if the thread cannot be made | public |
| reply to a Sparky message in a thread | that thread | public |
| any other message in a thread | nowhere; the bot stays quiet | |
| direct message | the direct message channel | private |

A thread is the conversation. Inside one, only a reply to something Sparky said continues it. The message replied to rides the request as `reply_to` and is quoted into the prompt under `prompt.reply_header`, capped by `agent.reply_budget_tokens`. A reply to a person or another bot is not a turn.

A direct message is private: only there are memories recalled and the memory tools offered. `bot.direct_messages` turns the direct channel off.

Up to `bot.max_images` image attachments ride the request as image blocks on the user turn. The engine filters them again in `Attachment::accepted`. Only the link travels, the model server fetches it, and history stores none of them. Whether the model reads images depends on the model; see `deploy/inference/README.md`.

Reading a reply whose author turned its ping off needs the privileged Message Content intent, enabled on the application at discord.com/developers. Without it the reply goes unanswered.

A turn is one message, edited in place. It opens with a spinner header and gains one line per progress event. The engine writes each line's text; the bot never keeps its own copy of the event enum. A line with a `slot` overwrites the earlier line in that slot. A `clear` event removes a line. A `draft` event sets or withdraws the answer shown under the steps. Edits are paced to one per `bot.edit_every_ms`.

The finished card shows the steps (the oldest fold into a count when space runs out), the answer, the memories the prompt carried (`ChatResponse.memories`), and sources without a URL. Sources with a URL are link buttons. An answer that does not fit spills into further messages. An approval adds buttons to the card, and the resumed answer replaces the prompt on the same card.

A conversation belongs to one tenant, user, channel, and visibility. The bot holds no conversation state. `/reset` ends the caller's open conversations in the channel through `POST /conversation/reset`. `/memories` lists what Sparky remembers, numbered; `/forget <n>` removes one, and `/forget all` asks with buttons, then removes every memory.

## Storage

| Data | Store |
|---|---|
| conversations, messages, memories, confirmations | the engine SQLite file (`store.path`) and its backups |
| knowledge index, live query registry, linked account grants | Platform |
| local traces, console logs, eval reports | `.sparky/`, ignored |

The engine keeps nothing else. Its schema is `apps/engine/migrations/`; the Platform schema lives in the Platform repo.

## Failure behavior

| Failure | Response |
|---|---|
| model returns a retryable error | retry with backoff up to `agent.max_model_retries` within the deadline, then 502 |
| all model slots busy past `agent.model_queue_wait_secs` | 503, which the bot reports as temporary |
| model call outlives the request deadline | run ends as `Deadline` |
| tool argument error or Platform refusal | the reason becomes the tool result and the model corrects the call |
| tool timeout | `ToolError::Timeout` fed back as the tool result |
| live query outlives `query.timeout_secs` or the deadline | `QueryError::Timeout` fed back as the tool result |
| model repeats an identical tool call | repeat refused, next step offers no tools, `Stalled` if it repeats again |
| thinking spends the whole completion | the call is made again without thinking |
| prompt would exceed the budget | history is trimmed and tool results are cut; boot checks keep tool schemas under half |
| `search_knowledge` finds nothing | `tools.nothing_stored` sends the model to another search |
| guardrail blocks a response | run ends as `Blocked` with `guardrail.replacement` |
| guardrail finds a protected term in an answer | the term is replaced with `guardrail.redaction` and the answer proceeds |
| confirmation denied, expired, or answered by someone else | nothing runs |
| Platform unavailable | the engine does not boot, the readiness probe fails, and a request that needs a store gets 503 |
| JSONL trace over `trace.max_file_bytes` | later events for that request are dropped |

## Tracing

Every request produces one trace covering model calls, tool calls, memory recall, policy decisions, guardrail blocks and redactions, and the outcome. The live pieces of a streaming call (reasoning so far, answer drafts, a withdrawn draft) go only to the watcher; the recorded trace keeps the finished call. A guardrail block or redaction is also recorded as an event on the `agent.run` span. Each `tool` span carries the tool's risk class.

```mermaid
flowchart TD
    A["discord.ask<br/>bot"] --> B["http.chat<br/>engine, parented by traceparent"]
    B --> C["agent.run<br/>invoke_agent"]
    C --> E["llm<br/>full prompt and reply, thinking"]
    C --> F["tool<br/>redacted arguments and result"]
```

Traces go to two places:

- JSONL at `.sparky/traces/<request_id>.jsonl`: the complete local record of trace events.
- Phoenix: spans exported over OTLP/HTTP to `telemetry.phoenix_url` plus `/v1/traces`, read as a tree per conversation. They carry `gen_ai.*` attributes beside the OpenInference ones the Phoenix UI reads: `user.id` for the user, `session.id` for the conversation. The resource attribute `openinference.project.name` puts them in the project named by `telemetry.project_name`.

An empty `phoenix_url` turns export off. `telemetry.phoenix_api_key` is sent as a bearer token when set. The bot records each product event as one span under the interaction (`[analytics]`). A finished answer carries Helpful and Not helpful buttons; only the person who asked can press them, and a press records `discord_feedback` with the `request_id` of the answer and `helpful`, so Phoenix joins the rating to the engine trace.

Secrets, credentials, cookies, and sensitive form values are redacted from tool arguments and results before they reach any trace. The result of a `ReadAuthenticated` tool never reaches a trace in clear: the JSONL trace, the `tool` span and the prompt recorded on the `llm` span carry `[authenticated result, <n> chars]` in its place. The answer built from it stays in the trace. The developer console mirrors followed stdout into `.sparky/logs/<unit>.log`. Deployments keep stdout with the platform log driver.

## Metrics

`llama-server` runs with `--metrics` and exports Prometheus format on its own port. Prometheus scrapes it and Grafana reads Prometheus. Both are opt-in (`just metrics`) and bind to loopback.

```mermaid
flowchart LR
    subgraph inference ["llama-server"]
        CH["chat :8000/metrics"]
        EM["embed :8001/metrics"]
    end
    GX["gpu-exporter :9835<br/>nvidia-smi, gpu-metrics profile"]
    CH --> P[("prometheus :9090<br/>15s scrape, 15d retention")]
    EM --> P
    GX --> P
    P --> G["grafana :3000<br/>throughput, queue, batching"]
    EN["engine"] -->|"inference"| CH
    EN -->|"OTLP spans"| PX["phoenix :6006<br/>prompt, reply, tokens, latency"]
```

Phoenix holds spans and events; Prometheus holds time series. Dashboard panels and their metric names are in `deploy/README.md`.

## Authenticated tasks (Phase 8)

Not built; nothing drives a page on a user's behalf. A design must meet these rules: the user completes login and MFA themselves, SparkyAI never asks for or stores a password, authenticated page content is never indexed or memorized, and any consequential submission is confirmed by the user. This is per-user, each user's own session for their own data, and distinct from the organization ASU sign-in Platform uses for Sun Devil Central.

## Configuration

Two layers, lowest first: `sparky.toml`, then `SPARKY_<SECTION>__<KEY>` environment variables, which win. `sparky.toml` is committed and holds every tunable value. `.env` is not committed and holds only secrets, per-machine URLs, and what docker compose and the justfile read. The image copies `sparky.toml` in, and compose overrides the service URLs through the environment.

Rust reads the file with figment, `apps/evals` with tomllib through pydantic-settings. `SPARKY_CONFIG_FILE` points at a different file, such as an eval profile. A missing file is not an error.

Sections in `sparky.toml`: `app`, `agent` (with `agent.thinking`), `prompt`, `model` (with `model.sampling`), `embedding`, `retrieval`, `policy`, `tools`, `sandbox`, `guardrail`, `compaction`, `query`, `mcp`, `outlook`, `gcal`, `canvas`, `upcoming`, `papers`, `wikipedia`, `trace`, `telemetry`, `analytics`, `http`, `web`, `admin`, `bot`, `platform`, `store`, `memory`, `cli`, `evals`. The `engine` and `discord` sections hold only env values: the service token and the guild id; `platform.url` and `platform.token` come from the environment too.

A default belongs to exactly one settings struct; adapters declare no defaults of their own. `Config::validate` rejects at boot any combination the engine cannot serve, for example both retrieval legs off, a section budget above the prompt budget, a sample ratio out of range, two MCP servers with the same name, a missing or malformed `SPARKY_PLATFORM__URL`, a missing `SPARKY_PLATFORM__TOKEN`, an empty `platform.mcp_private` or `platform.mcp_skip` prefix, a `[canvas]` window with `default_days` above `max_days`, a `[web]` sign-in without its Discord app, session key, https URLs, or its site in `http.cors_origins`, or with `retrieval.top_k`, `retrieval.window`, or `agent.confirmation_ttl_secs` past what the platform accepts. An unreadable `prompt.system_file` also stops the boot. Nothing is clamped at runtime.

`prompt` holds the wording the harness writes around every section. `system_file` takes precedence over `system`, which takes precedence over the built-in prompt.

## Deployment

One service image, `sparkyai-rust` (engine and discord, selected by entrypoint). `sparkyai-sandbox` and `sparkyai-sandbox-proxy` are the images the engine starts for `run_sandbox`. `sparkyai-runpod` holds every service and both models for one RunPod GPU pod, where containers cannot start, so `run_sandbox` is off there. CD rebuilds only the images whose inputs changed. Platform runs outside this repo. `llama-server` runs as the compose services `chat` and `embed` under the `model` profile, configured in `deploy/inference`. Details are in `deploy/README.md`.

## Open decisions

- Chat model size and quantization.
- Whether a reranker earns its place once the eval set exists.
- Parallel slots per `llama-server` under load.
- Default `chars_per_token` once the tokenizer is measured.
- Retention periods for conversations and traces. `store.conversation_idle_days` is 0 (keep) until this is settled; #82 proposes 30.
- Moderator access to user conversations and traces.
- App server host.

Record each in the commit that settles it.
