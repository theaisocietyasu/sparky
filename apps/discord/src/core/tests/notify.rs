//! Reminders as direct messages: the wire shape the engine sends and the message text.

use crate::bot::notify::message_text;
use crate::core::types::OutgoingList;

#[test]
fn an_outgoing_list_reads_and_each_item_becomes_a_reminder_line() {
    let raw = r#"{"notifications":[{"id":"n1","user_id":"111","text":"Essay due"}]}"#;
    let list: OutgoingList = serde_json::from_str(raw).unwrap_or_default();
    assert_eq!(list.notifications.len(), 1);
    assert_eq!(list.notifications[0].user_id, "111");
    assert_eq!(
        message_text(&list.notifications[0].text),
        "\u{23f0} Reminder: Essay due"
    );
}
