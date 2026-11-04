"""One module per suite, each exposing score(case, turns). A suite is a registry entry."""

from __future__ import annotations

from collections.abc import Callable

from evals.core.types import EvalCase, Score, TurnResult
from evals.suites import (
    clarification,
    grounding,
    latency,
    memory,
    permissions,
    personal,
    refusal,
    tool_args,
    tool_selection,
    voice,
)

Scorer = Callable[[EvalCase, list[TurnResult]], Score | None]

_MODULES = (
    tool_selection,
    tool_args,
    grounding,
    voice,
    memory,
    personal,
    permissions,
    clarification,
    refusal,
    latency,
)

#: Every suite by name, in report order.
SUITES: dict[str, Scorer] = {m.__name__.rsplit(".", 1)[-1]: m.score for m in _MODULES}
