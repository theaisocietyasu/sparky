//! The turn card: steps, drafts, the final answer, approvals, and edit pacing.

use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::core::tests::support::{labels_of, linked, progress, render, response, unlinked};
use crate::core::types::chat::Confirmation;
use crate::render::card::{
    ANSWERING, Pacer, Steps, THINKING, answer, failed, resumed, source_label, steps_of, thinking,
};
use crate::render::components::{ButtonSpec, rows_for};

#[test]
fn a_linked_source_becomes_a_button_and_an_unlinked_one_a_subtext_line() {
    let resp = response(
        "2am",
        vec![linked("library_hours"), unlinked("front desk note")],
        "answered",
    );
    let out = render(&resp, 2_000);
    assert_eq!(out.len(), 1);
    assert!(!out[0].contains("Sources"), "no source list in the text");
    assert!(!out[0].contains("fetched"), "no fetch dates either");
    assert!(out[0].contains("Also from** front desk note"), "{}", out[0]);

    let rows = rows_for(&resp);
    assert_eq!(rows.len(), 1, "one row of sources");
    assert_eq!(
        rows[0],
        vec![ButtonSpec::Link {
            label: "library hours".into(),
            url: "https://asu.edu/library_hours".into(),
        }],
        "only the source with a page of its own is a button"
    );
}

#[test]
fn a_button_label_stays_within_what_discord_shows() {
    assert_eq!(source_label("library_hours"), "library hours");
    let long = source_label(&"a".repeat(80));
    assert!(long.chars().count() <= 40, "{long}");
    assert!(long.ends_with('\u{2026}'), "{long}");
}

#[test]
fn empty_answer_explains_the_status() {
    let out = render(&response("", vec![], "deadline"), 2_000);
    assert!(out[0].contains("too long"));
}

#[test]
fn steps_append_as_subtext_and_an_immediate_repeat_collapses() {
    let mut steps = Steps::default();
    assert!(steps.push(None, "thinking"));
    assert!(
        !steps.push(None, "thinking"),
        "an immediate repeat is dropped"
    );
    assert!(steps.push(None, "searching live courses"));
    assert!(steps.push(None, "thinking"), "a repeat later on is kept");
    assert!(!steps.push(None, "   "));
    assert_eq!(steps.lines().len(), 3);

    let head = thinking(&[], None, 2_000, 0);
    assert!(head.ends_with(THINKING), "{head}");
    let shown = thinking(&steps.lines(), None, 2_000, 0);
    assert_eq!(
        shown,
        format!("{head}\n-# thinking\n-# searching live courses\n-# thinking")
    );
    assert_ne!(
        thinking(&[], None, 2_000, 1),
        head,
        "the spinner turns with the frame"
    );

    let many: Vec<String> = (0..200).map(|i| format!("step number {i}")).collect();
    let folded = thinking(&many, None, 500, 0);
    assert!(folded.len() <= 500);
    assert!(folded.ends_with("step number 199"), "the newest step stays");
    assert!(folded.contains("earlier steps"), "{folded}");
}

#[test]
fn a_tool_result_writes_over_the_line_its_own_start_wrote() {
    let mut steps = Steps::default();
    assert!(steps.push(Some("model:1"), "thinking"));
    assert!(steps.push(Some("tool:c1"), "`search` running"));
    assert!(steps.push(Some("model:1"), "checking the hours page"));
    assert!(steps.push(Some("tool:c1"), "`search` gave 3 passages"));
    assert!(
        !steps.push(Some("tool:c1"), "`search` gave 3 passages"),
        "the same line twice changes nothing"
    );
    assert_eq!(
        steps.lines(),
        vec![
            "checking the hours page".to_owned(),
            "`search` gave 3 passages".to_owned()
        ],
        "each slot keeps its place and holds its newest line"
    );
}

#[test]
fn the_final_card_keeps_steps_then_answer_then_footers_in_order() {
    let mut resp = response(
        "Hayden closes at 2am.",
        vec![linked("Hayden hours"), unlinked("desk note")],
        "answered",
    );
    resp.memories = vec!["You study CSE.".into()];
    let steps = vec![
        "\u{1f914} thinking".to_owned(),
        "\u{2705} `search` \u{2192} three passages".to_owned(),
    ];

    let out = answer(&steps, &resp, 2_000);
    assert_eq!(out.len(), 1);
    let card = &out[0];
    assert!(!card.contains(THINKING), "the header goes once answered");
    let order = [
        "-# \u{1f914} thinking",
        "-# \u{2705} `search`",
        "Hayden closes at 2am.",
        "Also from** desk note",
        "Memory used**\n- You study CSE.",
    ];
    let positions: Vec<usize> = order
        .iter()
        .map(|part| card.find(part).unwrap_or(usize::MAX))
        .collect();
    assert!(positions.iter().all(|&p| p != usize::MAX), "{card}");
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{card}");

    let bare = answer(&[], &response("hi", vec![], "answered"), 2_000);
    assert_eq!(bare, vec!["hi".to_owned()], "no footers without content");
}

#[test]
fn an_oversized_card_folds_steps_then_trims_footers_then_continues() {
    let steps: Vec<String> = (0..40)
        .map(|i| format!("step {i} {}", "s".repeat(20)))
        .collect();
    let resp = response("short answer", vec![], "answered");
    let folded = answer(&steps, &resp, 500);
    assert_eq!(folded.len(), 1);
    assert!(folded[0].starts_with("-# 40 steps"), "{}", folded[0]);
    assert!(!folded[0].contains("step 0"), "{}", folded[0]);

    let mut resp = response("short answer", Vec::new(), "answered");
    resp.memories = (0..40)
        .map(|i| format!("memory {i} {}", "m".repeat(30)))
        .collect();
    let trimmed = answer(&steps, &resp, 500);
    assert_eq!(trimmed.len(), 1, "{trimmed:?}");
    assert!(trimmed[0].contains("- memory 2"));
    assert!(!trimmed[0].contains("- memory 3"));
    assert!(trimmed[0].contains("and 37 more"));

    let resp = response(&"word ".repeat(300), vec![linked("one")], "answered");
    let spilled = answer(&steps, &resp, 500);
    assert!(spilled.len() > 1, "only a long answer continues");
    assert!(spilled.iter().all(|m| m.len() <= 500));
    assert!(spilled[0].starts_with("-# 40 steps"));
}

#[test]
fn an_accepted_approval_keeps_the_steps_and_replaces_prompt_and_footers() {
    let mut asked = response("", vec![unlinked("old source")], "awaiting_confirmation");
    asked.confirmation = Some(Confirmation {
        token: Uuid::new_v4(),
        tool: "announce".into(),
        summary: "Post the announcement.".into(),
    });
    let steps = vec!["thinking".to_owned(), "search finished".to_owned()];
    let card = answer(&steps, &asked, 2_000);
    assert_eq!(card.len(), 1);
    assert!(card[0].contains("`announce` needs your approval:** Post the announcement."));
    assert!(!card[0].contains("no answer"), "{}", card[0]);
    assert_eq!(labels_of(&rows_for(&asked)[0]), vec!["Yes, do it", "No"]);
    assert_eq!(steps_of(&card[0]), (0, steps.clone()));

    let done = response("Registered.", vec![unlinked("new source")], "answered");
    let out = resumed(&card[0], true, &done, 2_000);
    assert_eq!(
        out,
        vec![
            "-# thinking\n-# search finished\n-# approved\n\nRegistered.\n\n**\u{1f4da} Also from** new source"
                .to_owned()
        ]
    );
    let declined = resumed(&card[0], false, &done, 2_000);
    assert!(declined[0].contains("-# declined"));
    assert!(!declined[0].contains("needs your approval"));
    assert!(!declined[0].contains("old source"));

    let broke = failed(&["thinking".to_owned()], "Sparky is unavailable.", 2_000);
    assert_eq!(broke, "-# thinking\n\nSparky is unavailable.");
}

#[test]
fn a_resumed_card_at_the_discord_limit_folds_and_stays_within_it() {
    let steps: Vec<String> = (0..60)
        .map(|i| format!("step {i} {}", "s".repeat(20)))
        .collect();
    let mut asked = response(&"a".repeat(400), vec![], "awaiting_confirmation");
    asked.confirmation = Some(Confirmation {
        token: Uuid::new_v4(),
        tool: "announce".into(),
        summary: "Post the announcement.".into(),
    });
    let card = answer(&steps, &asked, 2_000);
    assert_eq!(card.len(), 1);
    assert!(card[0].len() <= 2_000);
    assert!(card[0].starts_with("-# 60 steps"), "{}", card[0]);
    assert_eq!(steps_of(&card[0]).0, 60);

    let done = response(&"word ".repeat(380), vec![linked("one")], "answered");
    let out = resumed(&card[0], true, &done, 2_000);
    assert!(out.iter().all(|m| m.len() <= 2_000));
    assert!(
        out[0].starts_with("-# 60 steps\n-# approved\n\nword"),
        "{}",
        out[0]
    );

    let longer = response(&"word ".repeat(398), vec![linked("one")], "answered");
    let out = resumed(&card[0], true, &longer, 2_000);
    assert!(out.iter().all(|m| m.len() <= 2_000));
    assert!(out[0].starts_with("-# 61 steps\n\nword"), "{}", out[0]);
    assert!(!out[0].contains("needs your approval"));
}

#[test]
fn the_pacer_holds_a_change_for_the_next_slot_and_never_drops_it() {
    let every = Duration::from_millis(1_500);
    let start = Instant::now();
    let mut pacer = Pacer::new(every, start);
    assert_eq!(pacer.wait(start), None, "nothing waits");

    pacer.mark(false);
    assert_eq!(
        pacer.wait(start),
        None,
        "an unchanged step waits for nothing"
    );

    pacer.mark(true);
    let later = start + Duration::from_millis(500);
    assert_eq!(pacer.wait(later), Some(Duration::from_secs(1)));
    let late = start + Duration::from_secs(2);
    assert_eq!(
        pacer.wait(late),
        Some(Duration::ZERO),
        "overdue edits go now"
    );

    pacer.flushed(late);
    assert_eq!(pacer.wait(late), None);
    pacer.mark(true);
    assert_eq!(pacer.wait(late), Some(every));
}

#[test]
fn the_thinking_line_follows_the_reasoning_and_settles_on_the_thought() {
    let mut steps = Steps::default();
    assert!(steps.apply(&progress(
        "\u{1f914} thinking",
        Some("model:1"),
        false,
        false
    )));
    assert!(steps.apply(&progress(
        "\u{1f914} The hours are in row two.",
        Some("model:1"),
        false,
        false
    )));
    assert!(steps.apply(&progress(
        "\u{1f4ad} The hours are in row two. Hayden closes at 2am.",
        Some("model:1"),
        false,
        false
    )));
    assert_eq!(
        steps.lines(),
        vec!["\u{1f4ad} The hours are in row two. Hayden closes at 2am.".to_owned()],
        "one line, rewritten as the reasoning grows"
    );
}

#[test]
fn the_answer_draft_is_the_body_of_the_running_card_and_can_be_withdrawn() {
    let mut steps = Steps::default();
    assert!(steps.apply(&progress(
        "\u{1f4ad} thought",
        Some("model:1"),
        false,
        false
    )));
    assert!(steps.apply(&progress(
        "Hayden closes at 2am.",
        Some("answer"),
        false,
        true
    )));
    assert!(
        !steps.apply(&progress(
            "Hayden closes at 2am.",
            Some("answer"),
            false,
            true
        )),
        "a repeat changes nothing"
    );
    assert_eq!(steps.lines().len(), 1, "a draft is not a step");
    let card = thinking(&steps.lines(), steps.draft(), 2_000, 0);
    assert!(card.contains(ANSWERING), "{card}");
    assert!(
        card.ends_with("-# \u{1f4ad} thought\n\nHayden closes at 2am."),
        "{card}"
    );

    assert!(steps.apply(&progress("", Some("answer"), true, true)));
    assert_eq!(steps.draft(), None);
    assert!(thinking(&steps.lines(), steps.draft(), 2_000, 0).contains(THINKING));
}

#[test]
fn a_draft_too_long_for_the_card_folds_the_steps_then_keeps_its_newest_part() {
    let steps: Vec<String> = (0..20).map(|i| format!("step number {i}")).collect();
    let draft = format!("{} the end.", "word ".repeat(200));
    let card = thinking(&steps, Some(&draft), 500, 0);
    assert!(card.len() <= 500, "{}", card.len());
    assert!(
        card.ends_with("the end."),
        "the newest part of the draft stays"
    );
    assert!(card.contains("20 earlier steps"), "{card}");
    assert!(card.contains("\u{2026}word"), "{card}");
}
