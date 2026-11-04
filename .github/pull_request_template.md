## What and why

<!-- One logical change. What it does, and why; the diff shows how. -->

Closes #

## Checklist

- [ ] `just check` passes (the pre-commit hook runs the recipes for the units the change touches)
- [ ] New behaviour has a test, and the test fails without the change
- [ ] Names the issue it closes (`Closes #N`)
- [ ] No contradiction with `docs/ARCHITECTURE.md` or `docs/ROADMAP.md`, or the doc is updated here
- [ ] A new tunable is in `sparky.toml` at its default; a new secret is in `.env.example`
- [ ] A change to a Platform API call matches the Platform repo, or names the Platform change it needs
- [ ] Comments follow the style in `AGENTS.md`: plain ASCII, what and not why
- [ ] Nothing here lets a credential reach a prompt, a log, a trace or a metric label
- [ ] Nothing here lets personal memory reach an answer others can read
