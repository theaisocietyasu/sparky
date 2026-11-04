# apps/evals

Deterministic engine evals with a baseline gate.

```bash
just eval run | baseline | compare
```

| Module | Holds |
|---|---|
| `runner.py` | posts each golden case to `/chat` and reads the engine's JSONL trace for that request |
| `suites/` | deterministic scorers: tool selection and arguments, grounding, refusal, permissions, clarification, memory, personal, latency, voice |
| `cases/` | hand-written ASU questions with expectations; add a line and it runs |
| `baseline.json` | accepted suite rates, once one has been promoted |
| `core/` | settings (the `[evals]` section of `sparky.toml`) and shared types |

Reports go under `.sparky/evals/`. Engine traces are read from `.sparky/traces/`. Golden cases and the promoted baseline are committed here. Evals need a live engine.

## Cases

Every `*.jsonl` under `cases/` is loaded.

- `asu_golden.jsonl` is the original set: grounding, refusal, permissions, memory, latency.
- `tool_routing.jsonl` is the regression set for tool choice and answer voice: which source
  answers a question, and that the answer never names a tool or a source key.
- `personal.jsonl` is the direct-message memory set. A case sets `visibility`, and
  `new_conversation` sends the follow-up as a fresh conversation, so only a stored memory
  carries over. The deadline and discovery cases expect `upcoming` for what is due, a
  `search_live` query shaped by a remembered interest, and the `/canvas` hint for a student
  with no Canvas link. The last one passes only with `[canvas] enabled` in the eval profile.

## Suites

`tool_selection` and `tool_args` read the trace: which tool ran and what it was given.
`grounding` reads the answer and its citations, matching a citation by its source key rather
than its label. `voice` fails an answer that names its own machinery, which is what a student
can neither act on nor verify. `memory`, `permissions`, `clarification`, `refusal` and
`latency` are unchanged. `personal` checks whether the first turn wrote a memory
(`writes_memory`), that the last answer uses what was stored (`remembers`), and that it leaves
out what must never be kept (`not_mentions`).

Run one with `just eval run --suite voice`.
