//! Context assembly: ordering and budget trimming.

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::core::tests::support::ctx;
use crate::core::types::agent::assemble::{Budget, Sections, Templates};
use crate::core::types::conversation::message::{Message, Role};
use crate::core::types::knowledge::evidence::age;
use crate::core::types::memory::{Memory, MemoryKind};
use crate::runtime::harness::agent::prompt::assemble::assemble;

#[test]
fn system_comes_first_and_input_last() {
    let out = assemble(
        &ctx(),
        &Sections {
            system: "You are Sparky.",
            input: "hi",
            date: "Friday 11 September 2026",
            ..Sections::default()
        },
        Budget::default(),
    );
    assert_eq!(out.messages.first().map(|m| m.role), Some(Role::System));
    assert_eq!(out.messages.last().map(|m| m.content.as_str()), Some("hi"));
}

#[test]
fn history_keeps_the_newest_turns() {
    let history: Vec<Message> = (0..50)
        .map(|i| Message::user(format!("turn {i} {}", "y".repeat(100))))
        .collect();
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            history: &history,
            input: "q",
            date: "Friday 11 September 2026",
            ..Sections::default()
        },
        Budget {
            history: 300,
            ..Budget::default()
        },
    );
    assert!(out.messages.len() < 52);
    let last_history = &out.messages[out.messages.len() - 2];
    assert!(last_history.content.starts_with("turn 49"));
}

#[test]
fn history_never_starts_with_a_tool_result() {
    let history = vec![
        Message::assistant_tool_calls("", vec![]),
        Message::tool_result("c1", "echo", "big result ".repeat(50)),
        Message::user("follow-up"),
    ];
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            history: &history,
            input: "q",
            date: "Friday 11 September 2026",
            ..Sections::default()
        },
        Budget {
            history: 160,
            ..Budget::default()
        },
    );
    let first_history = out.messages.iter().skip(1).find(|m| m.role != Role::System);
    assert!(first_history.is_none_or(|m| m.role != Role::Tool));
}

#[test]
fn a_resumed_run_appends_no_input_of_its_own() {
    // A resumed run adds no user turn after the history.
    let history = vec![
        Message::user("ban that spammer"),
        Message::assistant("working on it"),
    ];
    let out = assemble(
        &ctx(),
        &Sections {
            system: "sys",
            memory: &[],
            uploads: &[],
            history: &history,
            turn: &[],
            capabilities: "",
            input: "",
            date: "Friday 11 September 2026",
            templates: Templates::default(),
        },
        Budget::default(),
    );

    assert_eq!(
        out.messages.last().map(|m| m.role),
        Some(Role::Assistant),
        "the last message is the history's own, not an empty user turn"
    );
    assert!(
        out.messages
            .iter()
            .all(|m| !(m.role == Role::User && m.content.is_empty())),
        "no empty user turn is added"
    );
}

#[test]
fn the_wording_around_every_section_comes_from_configuration() {
    let memories = vec![Memory {
        id: Uuid::new_v4(),
        kind: MemoryKind::Preference,
        content: "prefers mornings".into(),
        sensitive: false,
        created_at: Utc::now(),
        expires_at: None,
    }];
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            memory: &memories,
            input: "q",
            date: "Friday 11 September 2026",
            templates: Templates {
                role_line_no_roles: "caller {user}",
                memory_header: "REMEMBERED",
                ..Templates::default()
            },
            ..Sections::default()
        },
        Budget::default(),
    );
    let text: String = out.messages.iter().map(|m| m.content.clone()).collect();
    assert!(text.contains("REMEMBERED"), "{text}");
    assert!(text.contains("caller u"), "{text}");
}

#[test]
fn the_capabilities_heading_is_written_around_the_rendered_list() {
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            capabilities: "- search_library_hours (tool): finds ASU pages",
            input: "q",
            templates: Templates {
                capabilities_header: "WHAT YOU CAN DO",
                ..Templates::default()
            },
            ..Sections::default()
        },
        Budget::default(),
    );
    let text: String = out.messages.iter().map(|m| m.content.clone()).collect();
    assert!(
        text.contains("WHAT YOU CAN DO\n- search_library_hours (tool)"),
        "{text}"
    );
}

#[test]
fn the_date_reaches_the_prompt_so_today_is_answerable() {
    // The date appears in the assembled prompt.
    let a = assemble(
        &ctx(),
        &Sections {
            system: "You are Sparky.",
            input: "hours today?",
            date: "Friday 11 September 2026",
            ..Sections::default()
        },
        Budget::default(),
    );
    let system = &a.messages[0].content;
    assert!(system.contains("Friday 11 September 2026"), "{system}");
}

#[test]
fn no_date_writes_no_date_line() {
    let a = assemble(
        &ctx(),
        &Sections {
            system: "You are Sparky.",
            input: "hi",
            date: "",
            ..Sections::default()
        },
        Budget::default(),
    );
    assert!(
        !a.messages[0].content.contains("Today is"),
        "{:?}",
        a.messages[0]
    );
}

#[test]
fn thinking_the_model_wrote_inline_is_lifted_out_of_the_answer() {
    use crate::runtime::harness::agent::call::thought::split;

    let (thought, visible) = split(
        "",
        "<think>The hours are not in what I was given.</think>Hayden closes at 2am.",
    );
    assert_eq!(
        thought.as_deref(),
        Some("The hours are not in what I was given.")
    );
    assert_eq!(
        visible, "Hayden closes at 2am.",
        "the tags never reach a user"
    );

    // A step that ran out of room mid-thought still reported a thought.
    let (cut, visible) = split("", "<think>I should search for");
    assert_eq!(cut.as_deref(), Some("I should search for"));
    assert_eq!(visible.len(), 0);

    // Reasoning the provider returned in a field of its own wins over the text.
    let (given, visible) = split("provider reasoning", "<think>inline</think>answer");
    assert_eq!(given.as_deref(), Some("provider reasoning"));
    assert_eq!(visible, "answer");

    let (none, visible) = split("", "Hayden closes at 2am.");
    assert!(none.is_none());
    assert_eq!(visible, "Hayden closes at 2am.");
}

#[test]
fn the_tool_exchange_of_this_request_follows_the_question_and_is_never_dropped() {
    use crate::core::types::conversation::message::ToolCall;

    let call = Message::assistant_tool_calls(
        "",
        vec![ToolCall {
            id: "c1".into(),
            name: "search_news".into(),
            arguments: serde_json::json!({"keywords": "robotics"}),
        }],
    );
    let result = Message::tool_result("c1", "search_news", "x".repeat(4_000));
    let turn = [call, result];
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            input: "latest news",
            turn: &turn,
            date: "Friday 11 September 2026",
            ..Sections::default()
        },
        Budget {
            total: 1_200,
            ..Budget::default()
        },
    );
    let roles: Vec<Role> = out.messages.iter().map(|m| m.role).collect();
    let asked = out.messages.iter().position(|m| m.content == "latest news");
    let answered = out.messages.iter().position(|m| m.role == Role::Tool);
    assert!(
        asked.is_some() && answered > asked,
        "the result comes after the question: {roles:?}"
    );
    assert_eq!(
        out.messages.last().map(|m| m.role),
        Some(Role::Tool),
        "the tool result is the last thing the model reads"
    );
}

#[test]
fn a_page_age_reads_in_hours_under_two_days_and_in_days_after() {
    let now = Utc::now();
    assert_eq!(age(now - Duration::minutes(10), now), "under an hour ago");
    assert_eq!(age(now - Duration::minutes(90), now), "1 hour ago");
    assert_eq!(age(now - Duration::hours(30), now), "30 hours ago");
    assert_eq!(age(now - Duration::hours(50), now), "2 days ago");
}

#[test]
fn a_reply_quotes_the_message_it_answers_just_before_the_input() {
    let mut ctx = ctx();
    ctx.reply_to = Some("The library closes at 10pm.".to_owned());
    let out = assemble(
        &ctx,
        &Sections {
            system: "s",
            input: "and on Sunday?",
            ..Sections::default()
        },
        Budget::default(),
    );
    let quoted = out
        .messages
        .iter()
        .rposition(|m| m.content.contains("The library closes at 10pm."));
    let input = out
        .messages
        .iter()
        .rposition(|m| m.content == "and on Sunday?");
    assert_eq!(
        quoted.map(|i| out.messages[i].role),
        Some(Role::System),
        "the quote is a system block"
    );
    assert_eq!(quoted.map(|i| i + 1), input, "the quote precedes the input");
}

#[test]
fn no_reply_writes_no_quote() {
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            input: "hello",
            ..Sections::default()
        },
        Budget::default(),
    );
    let header = Templates::default().reply_header;
    assert!(!out.messages.iter().any(|m| m.content.contains(header)));
}

#[test]
fn a_quoted_reply_is_cut_to_its_budget() {
    let mut ctx = ctx();
    ctx.reply_to = Some(
        (0..40)
            .map(|i| format!("line {i} of a very long answer"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let budget = Budget {
        reply: 60,
        ..Budget::default()
    };
    let out = assemble(
        &ctx,
        &Sections {
            system: "s",
            input: "q",
            ..Sections::default()
        },
        budget,
    );
    let header = Templates::default().reply_header;
    let block = out.messages.iter().find(|m| m.content.contains(header));
    assert!(block.is_some(), "the quote is in the prompt");
    assert_eq!(
        block.map(|m| m.content.contains("line 0")),
        Some(true),
        "the quote starts at the top of the message"
    );
    assert_eq!(
        block.map(|m| m.content.contains("line 39")),
        Some(false),
        "the tail is cut"
    );
    assert_eq!(
        block.map(|m| m.estimated_tokens(budget.chars_per_token) <= budget.reply),
        Some(true),
        "the quote stays inside its budget"
    );
}

#[test]
fn a_tool_result_longer_than_the_room_left_is_cut_to_fit() {
    use crate::core::types::conversation::message::ToolCall;

    let call = Message::assistant_tool_calls(
        "",
        vec![ToolCall {
            id: "c1".into(),
            name: "search_knowledge".into(),
            arguments: serde_json::json!({"query": "AI Society events"}),
        }],
    );
    let result = Message::tool_result("c1", "search_knowledge", "x".repeat(40_000));
    let turn = [call, result];
    let budget = Budget {
        total: 1_200,
        ..Budget::default()
    };
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            input: "latest events",
            turn: &turn,
            date: "Friday 11 September 2026",
            ..Sections::default()
        },
        budget,
    );
    assert!(
        out.estimated_tokens <= budget.total,
        "{} over {}",
        out.estimated_tokens,
        budget.total
    );
    let last = out
        .messages
        .last()
        .cloned()
        .unwrap_or_else(|| Message::user(""));
    assert_eq!(last.role, Role::Tool);
    assert_eq!(last.tool_call_id.as_deref(), Some("c1"));
    assert!(last.content.starts_with("xxxx"));
    assert!(
        last.content
            .contains("more characters of this result were cut"),
        "{}",
        &last.content[last.content.len().saturating_sub(120)..]
    );
}

#[test]
fn a_short_result_keeps_its_share_and_a_long_one_takes_the_rest() {
    let short = Message::tool_result("c1", "a", "short answer");
    let long = Message::tool_result("c2", "b", "y".repeat(40_000));
    let turn = [short, long];
    let budget = Budget {
        total: 1_200,
        ..Budget::default()
    };
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            input: "q",
            turn: &turn,
            ..Sections::default()
        },
        budget,
    );
    assert!(out.estimated_tokens <= budget.total);
    let results: Vec<&str> = out
        .messages
        .iter()
        .filter(|m| m.role == Role::Tool)
        .map(|m| m.content.as_str())
        .collect();
    assert_eq!(results.first().copied(), Some("short answer"));
    assert!(
        results
            .get(1)
            .is_some_and(|c| c.len() > 2_000 && c.len() < 40_000)
    );
}

#[test]
fn a_turn_that_fits_is_carried_whole() {
    let result = Message::tool_result("c1", "a", "z".repeat(2_000));
    let turn = [result];
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            input: "q",
            turn: &turn,
            ..Sections::default()
        },
        Budget {
            total: 4_000,
            ..Budget::default()
        },
    );
    assert!(
        out.messages
            .last()
            .is_some_and(|m| m.content == "z".repeat(2_000))
    );
}

#[test]
fn a_leading_summary_is_kept_before_older_turns_when_history_is_over_budget() {
    let mut history = vec![Message::summary("SUMMARY of earlier turns")];
    history.extend((0..20).map(|i| Message::user(format!("turn {i} {}", "y".repeat(100)))));
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            history: &history,
            input: "q",
            ..Sections::default()
        },
        Budget {
            history: 120,
            ..Budget::default()
        },
    );
    assert!(
        out.messages
            .iter()
            .any(|m| m.role == Role::Summary && m.content.starts_with("SUMMARY")),
        "the summary survives trimming"
    );
    assert!(
        out.messages
            .iter()
            .any(|m| m.content.starts_with("turn 19"))
    );
    assert!(
        !out.messages
            .iter()
            .any(|m| m.content.starts_with("turn 0 "))
    );
}

fn remembered(kind: MemoryKind, content: &str) -> Memory {
    Memory {
        id: Uuid::new_v4(),
        kind,
        content: content.into(),
        sensitive: false,
        created_at: Utc::now(),
        expires_at: None,
    }
}

#[test]
fn a_memory_line_carries_its_short_id_and_lines_stop_at_the_budget() {
    let memories: Vec<Memory> = (0..40)
        .map(|n| {
            remembered(
                MemoryKind::Fact,
                &format!("fact number {n} about the student"),
            )
        })
        .collect();
    let budget = Budget {
        memory: 60,
        ..Budget::default()
    };
    let out = assemble(
        &ctx(),
        &Sections {
            system: "s",
            memory: &memories,
            input: "q",
            ..Sections::default()
        },
        budget,
    );
    let text: String = out.messages.iter().map(|m| m.content.clone()).collect();
    let first = format!("- [{}] fact: fact number 0", memories[0].short_id());
    assert!(text.contains(&first), "{text}");
    assert!(out.memory_used > 0 && out.memory_used < memories.len());
}

#[test]
fn the_memory_and_discovery_rules_are_only_in_a_private_turn() {
    let sections = Sections {
        system: "s",
        input: "q",
        templates: Templates {
            memory_rules: "MEMORY RULES",
            discovery_rules: "DISCOVERY RULES",
            ..Templates::default()
        },
        ..Sections::default()
    };
    let public = assemble(&ctx(), &sections, Budget::default());
    let private = assemble(
        &ctx().with_visibility(crate::core::types::conversation::Visibility::Private),
        &sections,
        Budget::default(),
    );
    let text = |out: &crate::core::types::agent::assemble::Assembled| -> String {
        out.messages.iter().map(|m| m.content.clone()).collect()
    };
    assert!(!text(&public).contains("MEMORY RULES"));
    assert!(text(&private).contains("MEMORY RULES"));
    assert!(!text(&public).contains("DISCOVERY RULES"));
    assert!(text(&private).contains("DISCOVERY RULES"));
}
