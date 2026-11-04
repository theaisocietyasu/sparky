//! Buttons and their custom ids.

use uuid::Uuid;

use crate::core::tests::support::{labels_of, response};
use crate::core::types::chat::Confirmation;
use crate::render::components::{Action, CustomId, forget_rows, rows_for};

#[test]
fn a_component_id_survives_the_round_trip_and_rejects_anything_else() {
    let token = Uuid::new_v4();
    let convo = Uuid::new_v4();
    let id = CustomId::new(Action::Approve, token, convo);
    let wire = id.to_string();

    assert!(wire.starts_with("sparky:"), "{wire}");
    assert!(wire.len() <= 100, "Discord caps custom_id at 100 bytes");
    assert_eq!(CustomId::parse(&wire), Some(id));

    // Anything the bot did not mint is not ours to act on.
    assert_eq!(CustomId::parse("approve"), None);
    assert_eq!(CustomId::parse("other:approve:x:y"), None);
    assert_eq!(CustomId::parse("sparky:approve:not-a-uuid:x"), None);
    assert_eq!(CustomId::parse("sparky:launch:x:y"), None, "unknown action");
}

#[test]
fn a_confirmation_offers_the_two_answers_and_a_plain_answer_offers_none() {
    let resp = response("", vec![], "awaiting_confirmation");
    assert!(rows_for(&resp).is_empty(), "no confirmation, no buttons");

    let mut asked = response("", vec![], "awaiting_confirmation");
    asked.confirmation = Some(Confirmation {
        token: Uuid::new_v4(),
        tool: "announce".into(),
        summary: "Post the announcement.".into(),
    });
    let rows = rows_for(&asked);
    assert_eq!(rows.len(), 1, "one row of answers");
    assert_eq!(rows[0].len(), 2, "approve and deny");
}

#[test]
fn forget_ids_carry_the_asker_and_survive_the_round_trip() {
    let user = 123_456_789_012_345_678_u64;
    for id in [CustomId::ForgetAll { user }, CustomId::KeepAll { user }] {
        let wire = id.to_string();
        assert!(wire.len() <= 100, "{wire}");
        assert_eq!(CustomId::parse(&wire), Some(id));
    }
    assert_eq!(CustomId::parse("sparky:forget_all:not-a-number"), None);
    assert_eq!(CustomId::parse("sparky:forget_all:1:extra"), None);
    assert_eq!(CustomId::parse("sparky:forget_all"), None);

    let rows = forget_rows(user);
    assert_eq!(
        rows[0][0],
        crate::render::components::ButtonSpec::Press {
            id: CustomId::ForgetAll { user },
            label: "Forget everything",
            danger: true,
        }
    );
    assert_eq!(labels_of(&rows[0]), vec!["Forget everything", "Cancel"]);

    assert!(CustomId::ForgetAll { user }.may_press(user));
    assert!(!CustomId::ForgetAll { user }.may_press(user + 1));
    assert!(!CustomId::KeepAll { user }.may_press(7));
    let confirm = CustomId::new(
        crate::render::components::Action::Deny,
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    assert!(
        confirm.may_press(7),
        "the engine checks confirmation callers"
    );

    let lost = crate::render::reply::memory_failure(&crate::core::types::EngineError::Status {
        status: 404,
        body: String::new(),
    });
    assert_eq!(lost, crate::render::reply::UNAVAILABLE);
}

#[test]
fn feedback_ids_carry_the_request_and_the_asker() {
    let request = Uuid::new_v4();
    let user = 123_456_789_012_345_678_u64;
    let row = crate::render::components::feedback_row(request, user);
    assert_eq!(labels_of(&row), vec!["Helpful", "Not helpful"]);
    for (button, helpful) in row.iter().zip([true, false]) {
        let crate::render::components::ButtonSpec::Press { id, .. } = button else {
            unreachable!("a rating is a press")
        };
        assert_eq!(
            *id,
            CustomId::Feedback {
                helpful,
                request,
                user
            }
        );
        let wire = id.to_string();
        assert!(wire.len() <= 100, "{wire}");
        assert_eq!(CustomId::parse(&wire), Some(*id));
        assert!(id.may_press(user));
        assert!(!id.may_press(user + 1));
    }
    assert_eq!(CustomId::parse("sparky:helpful:not-a-uuid:1"), None);
    assert_eq!(
        CustomId::parse(&format!("sparky:helpful:{}", request.simple())),
        None
    );
}
