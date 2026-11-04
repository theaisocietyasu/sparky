import pytest
from evals.cli import app
from evals.core.types import EvalCase, Expectation, RunnerError, TurnResult
from evals.runner import load_cases
from evals.suites import (
    SUITES,
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
from typer.testing import CliRunner


def _turn(text="", status="answered", citations=(), events=(), latency_ms=100, conv="c1"):
    return TurnResult(
        request_id="r",
        conversation_id=conv,
        status=status,
        text=text,
        citations=list(citations),
        steps=1,
        tokens=1,
        latency_ms=latency_ms,
        events=list(events),
    )


def _case(**expect):
    return EvalCase(id="x", suites=[], question="q", expect=Expectation(**expect))


def test_tool_selection_and_args():
    ev = [
        {
            "kind": "tool_call",
            "tool": "search_courses",
            "arguments": {"query": "Library hours"},
        }
    ]
    assert tool_selection.score(_case(tool="search_courses"), [_turn(events=ev)]).passed
    assert not tool_selection.score(_case(tool="search_courses"), [_turn()]).passed
    assert not tool_selection.score(_case(), [_turn(events=ev)]).passed
    named = tool_args.score(
        _case(tool="search_courses", tool_args_contain={"query": "library"}),
        [_turn(events=ev)],
    )
    assert named.passed, "the key names the argument, the value is matched inside it"
    wrong_key = tool_args.score(
        _case(tool="search_courses", tool_args_contain={"q": "library"}), [_turn(events=ev)]
    )
    assert not wrong_key.passed, "an expectation naming an argument the tool lacks fails"
    assert tool_args.score(_case(tool="search_courses"), [_turn(events=ev)]) is None, (
        "no expectation means no result, not a free pass"
    )


def test_grounding_needs_citation_status_and_mentions():
    good = _turn(
        text="Open 7am to 9pm",
        citations=[{"title": "library_hours", "url": "https://lib.asu.edu/hours"}],
    )
    assert grounding.score(_case(source_key="library_hours", mentions=["7am"]), [good]).passed
    assert not grounding.score(_case(source_key="events"), [good]).passed
    assert not grounding.score(_case(source_key="library_hours"), [_turn(status="stalled")]).passed


def test_grounding_matches_the_source_key_not_the_page_title():
    titled = _turn(
        text="Open 7am to 9pm",
        citations=[
            {
                "key": "library_hours",
                "title": "Hours and Locations | ASU Library",
                "url": "https://lib.asu.edu/hours",
            }
        ],
    )
    assert grounding.score(_case(source_key="library_hours"), [titled]).passed, (
        "a citation labelled with the page title still credits its source"
    )
    assert not grounding.score(_case(source_key="courses"), [titled]).passed


def test_voice_rejects_an_answer_that_names_its_own_machinery():
    leaked = _turn(text="I can check live using search_live with source courses.")
    assert not voice.score(_case(), [leaked]).passed
    assert voice.score(_case(), [_turn(text="Let me check the class catalog.")]).passed
    fabricated = _turn(text="Their official website is ai.asu.edu.")
    assert not voice.score(_case(not_mentions=["ai.asu.edu"]), [fabricated]).passed


def test_refusal_and_clarification():
    assert refusal.score(
        _case(refuse=True), [_turn(text="I can't find that in my sources.")]
    ).passed
    assert not refusal.score(
        _case(refuse=True), [_turn(text="Sure, it is 5.", citations=[{"title": "x"}])]
    ).passed
    assert clarification.score(
        _case(clarify=True), [_turn(text="Which library do you mean?")]
    ).passed
    ev = [{"kind": "tool_call", "tool": "search_courses", "arguments": {}}]
    assert not clarification.score(_case(clarify=True), [_turn(text="Which?", events=ev)]).passed


def test_permissions_memory_latency():
    deny = [{"kind": "policy_decision", "tool": "t", "decision": {"decision": "deny"}}]
    assert permissions.score(_case(policy="deny"), [_turn(events=deny)]).passed
    assert permissions.score(
        _case(policy="confirm"), [_turn(status="awaiting_confirmation")]
    ).passed
    assert memory.score(
        _case(remembers="cs"), [_turn(text="hi"), _turn(text="You study CS.")]
    ).passed
    assert not memory.score(_case(remembers="cs"), [_turn(text="hi")]).passed
    assert latency.score(_case(max_latency_ms=200), [_turn(latency_ms=150)]).passed
    assert not latency.score(_case(max_latency_ms=100), [_turn(latency_ms=150)]).passed


def test_personal_checks_the_memory_write_and_the_last_answer():
    wrote = [{"kind": "memory_written", "memory": "fact"}]
    case = _case(writes_memory=True, remembers="poly")
    assert personal.score(case, [_turn(events=wrote), _turn(text="You are at Poly.")]).passed
    assert not personal.score(case, [_turn(), _turn(text="You are at Poly.")]).passed
    assert not personal.score(case, [_turn(events=wrote), _turn(text="No idea.")]).passed
    kept_out = _case(writes_memory=False, not_mentions=["2.9"])
    assert personal.score(kept_out, [_turn(), _turn(text="Nothing yet.")]).passed
    assert not personal.score(kept_out, [_turn(events=wrote), _turn(text="ok")]).passed
    assert not personal.score(kept_out, [_turn(), _turn(text="Your GPA is 2.9.")]).passed


def test_a_new_conversation_case_starts_the_follow_up_fresh(monkeypatch):
    from evals import runner

    sent = []

    def fake(question, *, conversation_id, visibility, channel_id="eval", **_):
        sent.append((conversation_id, visibility, channel_id))
        return _turn(conv="c1")

    monkeypatch.setattr(runner, "ask", fake)
    case = EvalCase(
        id="p",
        suites=["personal"],
        question="q",
        follow_up="f",
        visibility="private",
        new_conversation=True,
    )
    runner.run_case(case)
    assert sent == [(None, "private", "eval"), (None, "private", "eval-next")]


def test_every_suite_module_is_registered_by_its_name():
    assert list(SUITES) == [
        "tool_selection",
        "tool_args",
        "grounding",
        "voice",
        "memory",
        "personal",
        "permissions",
        "clarification",
        "refusal",
        "latency",
    ]
    assert SUITES["voice"] is voice.score


def test_load_cases_rejects_a_case_naming_an_unknown_suite(tmp_path):
    (tmp_path / "cases.jsonl").write_text(
        '{"id": "a", "suites": ["grounding", "groundng"], "question": "q"}\n'
    )
    with pytest.raises(RunnerError, match="groundng"):
        load_cases(tmp_path)


def test_run_rejects_an_unknown_suite_name():
    result = CliRunner().invoke(app, ["run", "--suite", "groundng"])
    assert result.exit_code == 2
    assert "groundng" in result.output
