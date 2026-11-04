//! Reply text: chunking, failure wording, and the memory listing.

use crate::core::types::memory::ListedMemory;
use crate::core::types::{EngineError, MemoryList};
use crate::render::reply::{
    MAX_MESSAGE, NOT_YOURS, NOTHING_REMEMBERED, UNAVAILABLE, chunk, confirm_failure,
    confirm_refused, failure, forget_failure, forgot, memory_failure, render_memories,
};

#[test]
fn short_text_is_one_message() {
    assert_eq!(chunk("hello", MAX_MESSAGE), vec!["hello".to_owned()]);
}

#[test]
fn long_text_splits_on_line_boundaries_under_the_limit() {
    let text = (0..200)
        .map(|i| format!("line {i} {}", "x".repeat(40)))
        .collect::<Vec<_>>()
        .join("\n");
    let parts = chunk(&text, 500);
    assert!(parts.len() > 1);
    assert!(parts.iter().all(|p| p.len() <= 500));
    assert!(parts.iter().all(|p| !p.ends_with('\n')));
    let rejoined = parts.join("\n");
    assert!(rejoined.contains("line 199"));
}

#[test]
fn chunking_always_advances_even_at_tiny_limits() {
    let parts = chunk("ééé", 1);
    assert_eq!(parts, vec!["é", "é", "é"]);
    let parts = chunk("a é b", 2);
    assert_eq!(parts.concat().replace(' ', ""), "aéb");
}

#[test]
fn capacity_and_outage_read_differently_to_the_user() {
    let busy = failure(&EngineError::Status {
        status: 503,
        body: "the model is at capacity".into(),
    });
    assert!(busy.contains("busy"), "{busy}");

    let down = failure(&EngineError::Transport("connection refused".into()));
    assert!(down.contains("unavailable"), "{down}");

    let broken = failure(&EngineError::Status {
        status: 502,
        body: String::new(),
    });
    assert!(broken.contains("unavailable"), "{broken}");

    let gone = failure(&EngineError::Status {
        status: 404,
        body: "no such conversation".into(),
    });
    assert!(gone.contains("Ask again"), "{gone}");
}

#[test]
fn a_refused_press_is_told_privately_and_other_failures_read_as_outages() {
    let status = |status| EngineError::Status {
        status,
        body: String::new(),
    };
    assert!(confirm_refused(&status(404)));
    assert!(confirm_refused(&status(409)));
    assert!(!confirm_refused(&status(502)));
    assert!(!confirm_refused(&EngineError::Transport("down".into())));
    assert_eq!(confirm_failure(&status(404)), NOT_YOURS);
    assert_eq!(confirm_failure(&status(500)), UNAVAILABLE);
    assert_eq!(failure(&status(409)), "That approval is no longer open.");
}

#[test]
fn memories_render_numbered_or_say_none_is_kept() {
    assert_eq!(
        render_memories(&MemoryList::default(), 2_000),
        vec![NOTHING_REMEMBERED.to_owned()]
    );
    let memory = |number: usize, kind: &str, text: &str, due: Option<&str>| ListedMemory {
        number,
        kind: kind.into(),
        text: text.into(),
        created_at: "2026-10-30".into(),
        due: due.map(str::to_owned),
    };
    let list = MemoryList {
        memories: vec![
            memory(1, "task", "CSE 310 project", Some("2026-11-12")),
            memory(2, "fact", "CS junior at Poly", None),
        ],
    };
    let out = render_memories(&list, 2_000).join("\n");
    assert!(
        out.contains("1. task: CSE 310 project (due 2026-11-12)"),
        "{out}"
    );
    assert!(
        out.contains("2. fact: CS junior at Poly (2026-10-30)"),
        "{out}"
    );

    let many = MemoryList {
        memories: (1..=200)
            .map(|i| memory(i, "fact", &format!("fact number {i}"), None))
            .collect(),
    };
    let parts = render_memories(&many, 500);
    assert!(parts.len() > 1);
    assert!(parts.iter().all(|p| p.len() <= 500));

    assert_eq!(forgot(1, Some(2)), "Forgot memory 2.");
    assert_eq!(
        forgot(0, Some(9)),
        "I have no memory 9. /memories shows the numbers."
    );
    assert_eq!(forgot(3, None), "Forgot 3 things.");
    assert_eq!(
        forget_failure(
            &EngineError::Status {
                status: 404,
                body: String::new()
            },
            9
        ),
        forgot(0, Some(9))
    );
    let off = memory_failure(&EngineError::Status {
        status: 503,
        body: String::new(),
    });
    assert_eq!(off, "I cannot reach my memory right now. Try again soon.");
}
