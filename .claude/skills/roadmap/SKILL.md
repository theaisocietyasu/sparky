---
name: roadmap
description: Report progress against the master issue #79 and its sub-issues, or record a completed item. Use when asked "where are we", when finishing a sub-issue, or when a phase's done-when list is met.
---

- To report: list the sub-issues with `gh api repos/theaisocietyasu/sparky/issues/79/sub_issues` and the open v1.0 issues with `gh api 'repos/theaisocietyasu/sparky/issues?labels=v1.0&state=all'`. For each open sub-issue in the current phase, inspect `apps/` and tests and say which scope lines exist in code, which are partial and which are missing. Cite files. Do not speculate.
- To record: close the sub-issue from the pull request that finishes it (`Closes #N` in the body). Tick a scope line in the issue body only if the code and a test exist. Do not add sub-issues, phases or prose.
- `docs/ROADMAP.md` holds the direction and the out-of-scope line only. If work in the repo diverges from it, say so and propose the smallest edit; do not rewrite it silently.
