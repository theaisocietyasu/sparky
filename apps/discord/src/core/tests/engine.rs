//! The engine client: traceparent, SSE framing, and frame decoding.

use uuid::Uuid;

use crate::core::types::{ERROR_BODY_CHARS, EngineError, Update};
use crate::engine::client::current_traceparent;
use crate::engine::sse::{decode, drain_frames, take_complete};

#[test]
fn no_traceparent_without_an_active_span() {
    assert!(current_traceparent().is_none());
}

#[test]
fn sse_frames_come_out_whole_even_when_the_bytes_arrive_split() {
    let mut buf = String::new();

    // A frame split across two reads yields nothing until it is complete.
    buf.push_str("event: progress\ndata: {\"text\":\"sear");
    assert_eq!(drain_frames(&mut buf).len(), 0);

    buf.push_str("ching ASU pages\"}\n\nevent: done\ndata: {}\n\n");
    let frames = drain_frames(&mut buf);
    assert_eq!(
        frames,
        vec![
            (
                "progress".to_owned(),
                "{\"text\":\"searching ASU pages\"}".to_owned()
            ),
            ("done".to_owned(), "{}".to_owned()),
        ]
    );
    assert!(buf.is_empty(), "consumed frames leave the buffer clean");
}

#[test]
fn a_frame_without_an_event_name_still_carries_its_data() {
    let mut buf = String::from("data: {\"a\":1}\n\n");
    assert_eq!(
        drain_frames(&mut buf),
        vec![(String::new(), "{\"a\":1}".to_owned())]
    );
}

#[test]
fn a_character_split_across_two_network_chunks_arrives_whole() {
    let frame = "event: progress\ndata: {\"text\":\"\u{1f914} thinking\"}\n\n";
    let bytes = frame.as_bytes();
    // The emoji is four bytes; cut inside it.
    let cut = frame.find('\u{1f914}').unwrap_or_default() + 2;
    let mut pending = Vec::new();
    let mut frames = Vec::new();
    for chunk in [&bytes[..cut], &bytes[cut..]] {
        pending.extend_from_slice(chunk);
        let mut text = take_complete(&mut pending);
        frames.extend(drain_frames(&mut text));
    }
    assert_eq!(frames.len(), 1);
    assert!(frames[0].1.contains('\u{1f914}'), "{:?}", frames[0].1);
    assert!(
        !frames[0].1.contains('\u{fffd}'),
        "no replacement characters"
    );
    assert_eq!(pending.len(), 0);
}

#[test]
fn each_frame_decodes_to_its_update_and_unknown_frames_to_none() {
    let progress = decode("progress", "{\"text\":\"searching\",\"slot\":\"s1\"}");
    assert!(
        matches!(&progress, Some(Update::Progress(p)) if p.text == "searching" && p.slot.as_deref() == Some("s1")),
        "{progress:?}"
    );
    assert!(decode("progress", "not json").is_none());

    let answer = decode(
        "answer",
        &format!(
            "{{\"request_id\":\"{}\",\"conversation_id\":\"{}\",\"text\":\"2am\",\"status\":\"answered\"}}",
            Uuid::nil(),
            Uuid::nil()
        ),
    );
    assert!(
        matches!(&answer, Some(Update::Answer(a)) if a.text == "2am"),
        "{answer:?}"
    );
    assert!(matches!(
        decode("answer", "{}"),
        Some(Update::Failed(EngineError::Transport(_)))
    ));

    let error = decode("error", "{\"error\":\"busy\",\"status\":503}");
    assert!(
        matches!(&error, Some(Update::Failed(EngineError::Status { status: 503, body })) if body == "busy"),
        "{error:?}"
    );
    let raw = "x".repeat(ERROR_BODY_CHARS + 50);
    let unreadable = decode("error", &raw);
    assert!(
        matches!(&unreadable, Some(Update::Failed(EngineError::Status { status: 502, body })) if body.chars().count() == ERROR_BODY_CHARS),
        "{unreadable:?}"
    );

    assert!(decode("done", "{}").is_none());
}

#[test]
fn the_memory_requests_have_the_shape_the_engine_routes_read() {
    use crate::core::types::{ForgetRequest, MemoryList, MemoryRequest};

    let list = serde_json::to_value(MemoryRequest {
        user_id: "111".into(),
        tenant_id: "guild".into(),
    });
    assert_eq!(
        list.ok(),
        Some(serde_json::json!({"user_id": "111", "tenant_id": "guild"}))
    );
    let one = serde_json::to_value(ForgetRequest {
        user_id: "111".into(),
        tenant_id: "guild".into(),
        number: Some(2),
    });
    assert_eq!(
        one.ok(),
        Some(serde_json::json!({"user_id": "111", "tenant_id": "guild", "number": 2}))
    );
    let all = serde_json::to_value(ForgetRequest {
        user_id: "111".into(),
        tenant_id: "guild".into(),
        number: None,
    });
    assert_eq!(
        all.ok(),
        Some(serde_json::json!({"user_id": "111", "tenant_id": "guild"}))
    );
    let body = r#"{"memories":[{"number":1,"id":"0a1b2c3d","kind":"task","text":"essay","created_at":"2026-10-30","due":"2026-11-12"}]}"#;
    let parsed: Result<MemoryList, _> = serde_json::from_str(body);
    assert_eq!(
        parsed.ok().map(|l| l.memories[0].due.clone()),
        Some(Some("2026-11-12".into()))
    );
}

#[test]
fn forget_reads_a_number_or_all() {
    use crate::bot::memory::{Which, parse_which};

    assert_eq!(parse_which("2"), Which::One(2));
    assert_eq!(parse_which(" #3 "), Which::One(3));
    assert_eq!(parse_which("ALL"), Which::All);
    assert_eq!(parse_which("0"), Which::Unclear);
    assert_eq!(parse_which("parking"), Which::Unclear);
}
