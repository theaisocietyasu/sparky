"""Runs golden cases against the engine /chat and collects the answer and trace for each turn."""

from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any

import httpx

from evals.core.settings import settings
from evals.core.types import EvalCase, RunnerError, TurnResult
from evals.suites import SUITES


def load_cases(cases_dir: Path | None = None) -> list[EvalCase]:
    """Every case under cases_dir. Raises RunnerError on none or on an unknown suite name."""
    cases_dir = cases_dir or settings().evals.cases_dir
    cases: list[EvalCase] = []
    for path in sorted(cases_dir.glob("*.jsonl")):
        for line in path.read_text().splitlines():
            if line.strip():
                case = EvalCase.model_validate_json(line)
                unknown = sorted(set(case.suites) - set(SUITES))
                if unknown:
                    raise RunnerError(
                        f"case {case.id} in {path.name} names unknown suite "
                        f"{', '.join(unknown)}; known: {', '.join(SUITES)}"
                    )
                cases.append(case)
    if not cases:
        raise RunnerError(f"no cases under {cases_dir}")
    return cases


def read_trace(request_id: str, traces_dir: Path | None = None) -> list[dict[str, Any]]:
    path = (traces_dir or settings().evals.traces_dir) / f"{request_id}.jsonl"
    if not path.exists():
        raise RunnerError(
            f"trace {path} not found; check that the engine and evals state directories match"
        )
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def ask(
    question: str,
    *,
    roles: list[str],
    conversation_id: str | None,
    user_id: str,
    engine_url: str | None = None,
    visibility: str = "public",
    channel_id: str = "eval",
) -> TurnResult:
    cfg = settings().evals
    body = {
        "user_id": user_id,
        "roles": roles,
        "message": question,
        "channel_id": channel_id,
        "visibility": visibility,
    }
    if conversation_id:
        body["conversation_id"] = conversation_id
    url = (engine_url or cfg.engine_url).rstrip("/") + "/chat"
    token = cfg.engine_service_token.get_secret_value()
    if not token:
        raise RunnerError(
            "SPARKY_EVALS__ENGINE_SERVICE_TOKEN is unset; the engine rejects unauthenticated "
            "calls, so every case would fail with 401"
        )
    started = time.monotonic()
    try:
        r = httpx.post(
            url,
            json=body,
            timeout=cfg.request_timeout_secs,
            headers={"Authorization": f"Bearer {token}"},
        )
    except httpx.HTTPError as e:
        raise RunnerError(f"engine at {url} unreachable: {e}") from e
    latency_ms = int((time.monotonic() - started) * 1000)
    if r.status_code != 200:
        raise RunnerError(f"engine returned {r.status_code}: {r.text[:200]}")
    try:
        d = r.json()
        turn = TurnResult(
            request_id=d["request_id"],
            conversation_id=d["conversation_id"],
            status=d["status"],
            text=d["text"],
            citations=d["citations"],
            steps=d["steps"],
            tokens=d["tokens"],
            latency_ms=latency_ms,
            events=[],
        )
    except (ValueError, KeyError, TypeError) as e:
        raise RunnerError(f"unexpected engine response shape: {e}") from e
    return turn.model_copy(update={"events": read_trace(turn.request_id)})


def run_case(case: EvalCase, engine_url: str | None = None) -> list[TurnResult]:
    """One or two turns. The follow-up continues the conversation unless new_conversation is set."""
    user_id = f"eval-{case.id}"
    first = ask(
        case.question,
        roles=case.roles,
        conversation_id=None,
        user_id=user_id,
        engine_url=engine_url,
        visibility=case.visibility,
    )
    turns = [first]
    if case.follow_up:
        fresh = case.new_conversation
        turns.append(
            ask(
                case.follow_up,
                roles=case.roles,
                conversation_id=None if fresh else first.conversation_id,
                user_id=user_id,
                engine_url=engine_url,
                visibility=case.visibility,
                channel_id="eval-next" if fresh else "eval",
            )
        )
    return turns
