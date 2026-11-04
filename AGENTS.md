# SparkyAI agent guide

Rust rebuild of an ASU student copilot. The master issue [#79](https://github.com/theaisocietyasu/sparky/issues/79) and its sub-issues say what we build and in what order; `docs/ROADMAP.md` gives the direction and what is out of scope; `docs/ARCHITECTURE.md` gives crate boundaries, traits, and invariants. Do not contradict either doc; propose an edit to the doc instead.

Principles and per-language rules for every change: [CONTRIBUTING.md](CONTRIBUTING.md).

## Commands

`just` is the entrypoint for every unit; `just` alone lists recipes. Install: https://just.systems.

```
just doctor | env | hooks | bootstrap   # first run
just check            # the gate: fmt-check, lint, test every unit. CI and the pre-commit hook run the same recipes for the units a change touches
just check-rust       # cargo fmt --check, clippy -D warnings, test, scripts/check-deps.sh
just check-evals      # ruff + pytest in apps/evals
just check-web        # eslint + tsc --noEmit + vitest + vite build in apps/web
just fmt              # format every unit in place
just setup            # install every unit's deps
just clean            # remove build artifacts and virtualenvs
just engine | discord # run a Rust app (needs .env, see .env.example)
just cli              # developer console (TUI): every unit, its logs, and tasks
just eval ...         # run, baseline, compare: golden cases against the engine
just phoenix          # Phoenix on :6006: traces, LLM generations, product events
just model            # llama-server chat and embed (CUDA)
just model-cpu        # the same on the processor, no GPU
just metrics          # prometheus (:9090) + grafana (:3000)
just gpu-metrics      # nvidia-smi exporter into prometheus; needs a GPU
just web              # Vite dev server on :5173
just sandbox-images   # build the run_sandbox image and its egress proxy
just sandbox-down     # remove the sandbox containers and egress proxy the engine started
just images           # build the rust image locally
just ps               # what compose has, across every profile
just up | down | logs # full compose stack (dev, builds locally)
just prod-up | prod-down | prod-logs   # GHCR images, SPARKY_IMAGE_TAG
just diagrams         # render ARCHITECTURE.md mermaid to verify syntax
```

A change is not done until `just check` passes.

## Layout

Everything that runs is under `apps/`. Language is never a folder; ASU domain is never a folder.

```
apps/engine/      Rust bin: the agent and HTTP surface. core/{config,telemetry,types,traits,tests}, runtime/{harness,model,tools}, stores (over the Platform HTTP API), routes. One concern per file; split a module that grows past that.
apps/discord/     Rust bin: serenity bot, HTTP client of engine, never links it. core/{config,telemetry,types,tests}, bot, engine, render, access, analytics. One span per interaction and per product event.
apps/cli/         Rust bin sparky: developer console (ratatui). Drives just recipes and docker compose and tails their output. Links nothing in-repo. app/{control,keys,ui}, units/{catalog,health,logs,output,runner,sandbox}, core/{config,types,tests}.
apps/web/         static frontend + admin UI (Vite + React)
apps/evals/       Python: eval runners, suites, cases, and the baseline gate. core/{settings,types,tests}, cli, runner, suites, cases.
deploy/           compose, one Dockerfile per image, inference/ (model serving config)
docs/             ROADMAP.md, ARCHITECTURE.md
```

Processes talk only via: discord to engine; engine to llama-server and the Platform HTTP API and MCP (see Platform stores in ARCHITECTURE.md); every app to Phoenix for spans and events. Platform ([asusoda/platform](https://github.com/asusoda/platform), a separate repo) holds conversations, memories, the profile graph, confirmations, knowledge, live ASU queries, and linked accounts. The engine opens no database connection.

## Dependencies we build on

- **Rig** (`rig-core`, crate name `rig_core`): the OpenAI-compatible client for chat and embeddings (`runtime/model/rig_openai/`) and the only inference path. Never `rig::Agent`; the loop is ours.
- **rmcp**: MCP. Never hand-roll MCP.
- The rest of the harness (loop, policy, context assembly, memory, tracing, replay) is written here.

## Config

Two layers, lowest first: `sparky.toml` (committed), then `SPARKY_<SECTION>__<KEY>` env vars from `.env`, which win. Both load into `apps/engine/src/core/config/mod.rs`, `apps/discord/src/core/config.rs`, `apps/cli/src/core/config.rs` (Rust, figment) and `apps/evals/core/settings.py` (Python, tomllib through pydantic-settings). `SPARKY_CONFIG_FILE` points elsewhere.

**Every tunable value goes in `sparky.toml`**, at its default, in the same change that adds it. That covers every harness knob: loop limits, prompt budgets and their wording, sampling, retrieval tuning, policy, which tools register, tracing, and the HTTP surface. `.env` holds only secrets, per-machine URLs, and what docker compose and the justfile read; a new secret goes in `.env.example` too. Secrets are `SecretString`; never log one. Reject a bad combination in `Config::validate` at boot; never clamp at runtime.

## Rules

- Inside `apps/engine`: `core` imports nothing else in the crate; `runtime::harness`, `runtime::model`, `runtime::tools`, and `stores` import only `core`, never each other; `routes`/`wiring` compose them. Checked in review. Between apps: `engine`, `discord`, and `cli` never depend on each other, enforced by `scripts/check-deps.sh`.
- Workspace lints (`[workspace.lints]` in `Cargo.toml`): no `unwrap`/`expect`/`panic`/`todo!`/`unimplemented!`/`dbg!`/`println!`, no wildcard imports, docs on every public item.
- A crate's public surface is its constructors and the `harness` traits it implements. Nothing reaches into another adapter.
- No global mutable state. Per-request data goes in `RequestContext`.
- Every replaceable dependency sits behind a trait in `engine/src/core/traits` with a test double in `core/tests/support`.
- The engine never writes the retrieval index or fetches pages for it; Platform crawls, indexes, and runs live queries. With `sandbox.egress` on, `run_sandbox` may read public pages through the egress proxy; nothing it reads is indexed.
- Model output is never written back as retrieval evidence.
- Write-side tools go through `Policy`; consequential actions require confirmation.
- Live progress is a `TraceEvent`: give a new variant a line in `TraceEvent::progress` (or `None`) and every watching client shows it. Clients render the `text` the engine sends, never their own copy of the enum.
- Errors: `thiserror` enums per crate, no `anyhow` in library crates, no `unwrap` outside tests.
- Async: tokio. Traits use `async_trait` until native async traits cover our needs.
- Logging: `tracing` macros with structured fields.
- Dependencies are declared in `[workspace.dependencies]` and referenced with `.workspace = true`.
- Edition 2024, rustfmt defaults, clippy warnings are errors.

## Skills

Listed in `.claude/skills/README.md`. Use `rust-skills` when writing Rust, `postgres-strict` for schema and migrations, `test-driven-development` for features and fixes, `systematic-debugging` for bugs, `verification-before-completion` before saying anything is done, `security-audit-standard` before a release, `/code-quality` for cleanup and refactor passes. `/check` is the gate.

## Conventions

- Every app has a `core/` (`src/core/` in Rust) holding what the rest of the app builds on and nothing that does work. Rust: `core/{config,telemetry,types,traits,tests}`; Python: `core/{settings.py,types.py,tests/}`. **Data** (derives serde, or crosses a module as a value: messages, config, errors, wire shapes) goes in `core/types`; **interfaces** (traits) in `core/traits`; **objects** (state plus the methods that own it: `Agent`, `ToolSet`, sinks, clients, stores, handlers) beside their `impl` with private fields. Domain code imports from `core`; `core` imports nothing from the app.
- Public items have a one-line doc comment saying what, not how.
- **Comments are plain ASCII and monotone.** Applies to `//`, `///`, `//!`, `#`, `"""` and `--`. No backticks, quotation marks around terms, em dashes, arrows, or other non-ASCII. State what the code does; do not justify a decision or compare alternatives. No openers (Note that, Simply, Basically, Crucially, Importantly) and no closing summary. Present tense, declarative, one line where one line does. An invariant the code depends on is stated as a fact. The why belongs in the commit message.
- Commit messages: imperative subject of at most 72 chars, body explains why.
- The tree is scaffolded ahead of code. Fill a stub in place; don't create parallel files or rename stubs without updating ARCHITECTURE.md.
- Keep docs lean. No filler prose.

## Out of scope

See "Out of scope" in `docs/ROADMAP.md`. Don't build toward those without an explicit decision.
