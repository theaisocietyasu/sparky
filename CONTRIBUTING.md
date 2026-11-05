# Contributing

[AGENTS.md](AGENTS.md) has the commands, layering rules, and conventions every change follows. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before changing a boundary. Both apply to people and coding agents.

<!-- shared: start -->
## Principles

This section is the same in asusoda/platform, theaisocietyasu/sparky and theaisocietyasu/godfather. Change it in all three.

- One name for each thing, in code, UI and docs. Use the term the glossary uses.
- No ambiguity. A name, a route or a message has one meaning.
- Clean code: small functions, no dead code, no commented-out code, no TODO without an issue link.
- Small pull requests with one logical change.
- Tests and docs change in the same pull request as the code.
- A change is done when the gate passes.
- Comments are plain ASCII and say what the code does. The why goes in the commit body.
- Commit subjects are imperative and 72 characters or fewer.
<!-- shared: end -->

## Setup

Needs [just](https://just.systems), a Rust toolchain, [uv](https://docs.astral.sh/uv), Node, and Docker. `just doctor` names anything missing and how to install it.

```
just doctor        # what is missing
just bootstrap     # .env, hooks, dependencies
just check         # the gate: every unit
```

`bootstrap` copies `.env` from `.env.example`, points `core.hooksPath` at `.githooks`, and installs each unit's dependencies. The engine also needs a Platform URL and machine token in `.env` (`SPARKY_PLATFORM__URL`, `SPARKY_PLATFORM__TOKEN`). The pre-commit hook and CI run the gate only for the units a change touches.

### A model

`bootstrap` does not start one, and the engine answers nothing without it.

| | |
|---|---|
| `just model` | `llama-server` on CUDA. Needs an NVIDIA GPU and the container toolkit. |
| `just model-cpu` | the same GGUFs on the processor. Slow; fit for a smoke test. |
| hosted | set `SPARKY_MODEL__BASE_URL` and `SPARKY_MODEL__API_KEY` in `.env` to any OpenAI-compatible endpoint. |

Without one the engine still starts: `/health/live` answers, `/health/ready` reports `{"platform":true,"model":false}`, and `/chat` returns 502 `the model is unavailable`.

### The sandbox

`run_sandbox` needs a container runtime. With Docker running it works; without one the engine logs a warning at boot and does not offer the tool. `SPARKY_SANDBOX__REQUIRED=true` makes a missing runtime a boot failure, as in production.

## Branches and pull requests

Branch from `main` and name the branch for the change (`reply-threads`, `fix-login-link`). One logical change per pull request, with a test for new behaviour that fails without the change.

Commit subjects are imperative, at most 72 characters, with no trailing period. The body explains why.

A change is not done until `just check` passes: `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace` and `scripts/check-deps.sh` for Rust, ruff and pytest for `apps/evals`, and eslint, tsc, vitest and a build for the website.

CI clippy runs on current stable. If a lint fails in CI but not locally, run `rustup update stable`.

### A stacked branch

CI runs on pull requests into `main` only. For a pull request into another branch, run **CI** by hand from the Actions tab (`workflow_dispatch`); a manual run gates every unit.

## Rules that are not style

- Every tunable value goes in `sparky.toml` at its default, in the same change. `.env` holds only secrets, per-machine URLs, and what compose and the justfile read.
- The engine never writes the retrieval index; Platform crawls and indexes. Model output is never written back as evidence.
- Write-side tools go through `Policy`; consequential actions need a confirmation.
- Comments are plain ASCII and say what the code does. The why belongs in the commit message. See Conventions in AGENTS.md.

## Rust

- The `[workspace.lints]` in `Cargo.toml` deny `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `dbg!`, `println!` and wildcard imports, in tests too, and require docs on every public item.
- `cargo clippy --workspace --all-targets -D warnings`: a warning fails the gate.
- Errors are a `thiserror` enum per crate. No `anyhow` in library code.
- Async traits use `async_trait`.
- Dependencies are declared in `[workspace.dependencies]` and referenced with `.workspace = true`.
- `core` imports nothing else in its crate; `engine`, `discord` and `cli` never depend on each other (`scripts/check-deps.sh`).

`just check-rust` runs all of it.

## Python (apps/evals)

- `ruff` and `pytest`, through `just check-evals`.
- Settings load through `apps/evals/core/settings.py`; shared types are in `apps/evals/core/types.py`.

## TypeScript (apps/web)

- `eslint`, `tsc --noEmit` with `strict` on, `vitest`, and a `vite build`, through `just check-web`.

## CI and CD

| Workflow | Runs on | Does |
|---|---|---|
| CI | pull requests into main, main, manual | the gate for each unit that changed, or every unit on a manual run |
| CD | main, manual | builds and publishes the images whose inputs changed; a manual run builds all |

## Reporting

Bugs, wrong answers and feature requests: the issue templates. Vulnerabilities: privately, see [SECURITY.md](SECURITY.md).
