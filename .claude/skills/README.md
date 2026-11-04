# Skills

| Skill | Purpose | Origin |
|---|---|---|
| `check` | the gate: fmt, clippy, test, check-deps for every unit | ours |
| `roadmap` | progress against ROADMAP.md | ours |
| `code-quality` | refactor and cleanup pass: naming, workspace deps, registries over enumeration, dead code and silent fallbacks. User-invoked only. | ours (adapted, Rust-specific) |
| `rust-skills` | 265 Rust rules across 26 categories; the Rust style reference | [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills), MIT |
| `postgres-strict` | schema, indexing, migration safety, RLS, pgvector | [0xMassi/claude-skills](https://github.com/0xMassi/claude-skills), MIT |
| `security-audit-standard` | secret scanning, input validation, authz, deps, OWASP checklist | [0xMassi/claude-skills](https://github.com/0xMassi/claude-skills), MIT |
| `test-driven-development` | red, green, refactor; test-quality anti-patterns | [obra/superpowers](https://github.com/obra/superpowers), MIT |
| `systematic-debugging` | root cause before fixes | [obra/superpowers](https://github.com/obra/superpowers), MIT |
| `receiving-code-review` | check review feedback before acting on it | [obra/superpowers](https://github.com/obra/superpowers), MIT |
| `verification-before-completion` | run the command, read the output, then claim done | [obra/superpowers](https://github.com/obra/superpowers), MIT |

## From ash-fedora

Copied from [ashworks1706/ash-fedora](https://github.com/ashworks1706/ash-fedora) (`home/.claude/skills`). The license holder of each is in its LICENSE file.

| Skill | Purpose | License |
|---|---|---|
| `karpathy-guidelines` | rules against common coding mistakes while writing or reviewing code | MIT, Forrest Chang |
| `codebase-design` | vocabulary for deep modules and their interfaces | MIT, Matt Pocock |
| `technical-writing` | docs, comments, errors and PR text in ASD-STE100 plain English | ash |
| `write-report` | report results and findings with every number traced | ash |
| `writing-for-agents` | skills, AGENTS.md and CLAUDE.md files | MIT, Matt Pocock |
| `pr` | pull request bodies: diagram of the change, then evidence | MIT, Matt Pocock |
| `unslop` | remove AI tells from prose | MIT, Lauren Tan |
| `mermaid-diagrams` | correct Mermaid in Markdown | MIT, robust-skills contributors |
| `show-me` | visual explanations: diagrams, call trees, file trees | MIT, HumanLayer |
| `grilling` | stress-test a plan when asked | MIT, Matt Pocock |
| `discernment-nudge` | a check after advice the user will act on | Apache-2.0, Anthropic |
| `frontend-design` | visual direction for new UI in `apps/web` | Apache-2.0, Anthropic |
| `emil-design-eng` | UI polish, components and animation | MIT, Emil Kowalski |
| `better-accessibility`, `better-colors`, `better-layout`, `better-typography`, `better-ui`, `better-writing` | focused passes on one quality of an interface | MIT, Jakub Krehel |
| `better-interface`, `interface-review` | review a screen or a branch for interface problems | MIT, Jakub Krehel |

`code-quality` here is the Rust adaptation and stays; ash-fedora has a generic copy. `systematic-debugging` and `verification-before-completion` are the ash-fedora copies of obra/superpowers.

Third-party skills are vendored with their LICENSE files. Update by re-copying from the source; do not edit in place.
