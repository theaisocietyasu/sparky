"""Types for the engine evals."""

from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, Field


class Expectation(BaseModel):
    """What a golden case expects of the engine. Every field is optional."""

    status: str | None = None
    source_key: str | None = None
    tool: str | None = None
    tool_args_contain: dict[str, str] = Field(default_factory=dict)
    policy: Literal["allow", "deny", "confirm"] | None = None
    refuse: bool = False
    clarify: bool = False
    mentions: list[str] = Field(default_factory=list)
    not_mentions: list[str] = Field(default_factory=list)
    remembers: str | None = None
    writes_memory: bool | None = None
    max_latency_ms: int | None = None


class EvalCase(BaseModel):
    """A hand-written question with expectations. follow_up makes a two-turn case.

    new_conversation sends the follow-up as a fresh conversation, so only memory carries over.
    """

    id: str
    suites: list[str]
    question: str
    roles: list[str] = Field(default_factory=list)
    follow_up: str | None = None
    visibility: Literal["public", "private"] = "public"
    new_conversation: bool = False
    expect: Expectation = Field(default_factory=Expectation)


class Citation(BaseModel):
    """One source under an answer. Mirrors the engine wire shape."""

    key: str = ""
    title: str
    url: str | None = None


class TurnResult(BaseModel):
    """The engine answer to one turn plus its trace."""

    request_id: str
    conversation_id: str
    status: str
    text: str
    citations: list[Citation]
    steps: int
    tokens: int
    latency_ms: int
    events: list[dict[str, Any]]

    def tool_calls(self) -> list[dict[str, Any]]:
        """The tool_call events of this turn, in order."""
        return [e for e in self.events if e["kind"] == "tool_call"]


class Score(BaseModel):
    """One suite verdict on one case, with a reason a reader can act on."""

    passed: bool
    detail: str


class CaseResult(BaseModel):
    """The score one suite gave one case, tied to the engine request it judged."""

    case_id: str
    suite: str
    score: Score
    request_id: str


class SuiteReport(BaseModel):
    """How many cases one suite passed out of those it scored."""

    suite: str
    passed: int
    total: int

    @property
    def rate(self) -> float:
        """The pass rate, or 0.0 when the suite scored nothing."""
        return self.passed / self.total if self.total else 0.0


class EvalReport(BaseModel):
    """One eval run: the engine it ran against, every result, and the per-suite totals."""

    engine_url: str
    cases: int
    results: list[CaseResult]
    suites: list[SuiteReport]


class RunnerError(RuntimeError):
    """A case could not be run: no cases, engine unreachable, or no trace for the request."""
