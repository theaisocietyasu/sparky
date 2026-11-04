"""Personal answers from direct-message memory: what is written, what is used, what is kept out."""

from __future__ import annotations

from evals.core.types import EvalCase, Score, TurnResult


def score(case: EvalCase, turns: list[TurnResult]) -> Score | None:
    """Checks the memory writes of the first turn, then the last answer for what it must use."""
    want = case.expect
    if want.writes_memory is not None:
        wrote = any(e.get("kind") == "memory_written" for e in turns[0].events)
        if wrote != want.writes_memory:
            verb = "wrote" if wrote else "did not write"
            return Score(passed=False, detail=f"first turn {verb} a memory")
    last = turns[-1].text.lower()
    if want.remembers and want.remembers.lower() not in last:
        return Score(passed=False, detail=f"missing {want.remembers!r} in the last answer")
    leaked = [m for m in want.not_mentions if m.lower() in last]
    if leaked:
        return Score(passed=False, detail=f"last answer mentions {', '.join(leaked)}")
    return Score(passed=True, detail="memory written and used as expected")
