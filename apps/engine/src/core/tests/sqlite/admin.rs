//! Admin changes in the file: the newest row per kind and key is in force, and held actions can be denied.

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use super::{caller, store};
use crate::core::traits::admin::AdminStore;
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::types::admin::{ChangeKind, INSTRUCTIONS_KEY, NewChange};
use crate::core::types::safety::policy::{PendingAction, ProposedAction};
use crate::core::types::tools::RiskClass;

fn change(kind: ChangeKind, key: &str, value: Value) -> NewChange {
    NewChange {
        tenant_id: "g".into(),
        kind,
        key: key.into(),
        value,
        actor: "mod".into(),
    }
}

#[tokio::test]
async fn the_newest_change_per_key_is_in_force_and_history_is_newest_first() {
    let store = store().await.admin();
    let first = store
        .record(change(
            ChangeKind::Instructions,
            INSTRUCTIONS_KEY,
            json!("one"),
        ))
        .await;
    let Ok(first) = first else {
        unreachable!("the change is recorded")
    };
    assert!(
        store
            .record(change(ChangeKind::Tool, "run_sandbox", json!(false)))
            .await
            .is_ok()
    );
    assert!(
        store
            .record(change(
                ChangeKind::Instructions,
                INSTRUCTIONS_KEY,
                json!("two")
            ))
            .await
            .is_ok()
    );
    let Ok(current) = store.current("g").await else {
        unreachable!("the current changes read")
    };
    let values: Vec<(ChangeKind, Value)> =
        current.iter().map(|c| (c.kind, c.value.clone())).collect();
    assert_eq!(
        values,
        vec![
            (ChangeKind::Instructions, json!("two")),
            (ChangeKind::Tool, json!(false))
        ]
    );
    assert!(matches!(store.current("other").await, Ok(v) if v.is_empty()));
    let Ok(history) = store.history("g", 2).await else {
        unreachable!("the history reads")
    };
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].value, json!("two"));
    let Ok(Some(earlier)) = store.change("g", first.id).await else {
        unreachable!("the first change reads by id")
    };
    assert_eq!(earlier.value, json!("one"));
    assert!(matches!(store.change("other", first.id).await, Ok(None)));
}

#[tokio::test]
async fn a_held_action_is_listed_without_arguments_and_denied_once() {
    let file = store().await;
    let confirmations = file.confirmations();
    let admin = file.admin();
    let token = Uuid::new_v4();
    let pending = PendingAction {
        call_id: "c1".into(),
        action: ProposedAction {
            tool: "gcal_create".into(),
            risk: RiskClass::ExternalWrite,
            arguments: json!({"title": "secret plans"}),
            private: true,
        },
    };
    assert!(
        confirmations
            .hold(&caller(), token, &pending, "h", Duration::from_mins(2))
            .await
            .is_ok()
    );
    let Ok(held) = admin.held(&caller().tenant_id).await else {
        unreachable!("the held actions read")
    };
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].tool, "gcal_create");
    assert_eq!(held[0].user_id, caller().user_id);
    assert!(!format!("{held:?}").contains("secret plans"));

    let token = token.to_string();
    assert!(matches!(
        admin.deny(&caller().tenant_id, &token).await,
        Ok(true)
    ));
    assert!(matches!(
        admin.deny(&caller().tenant_id, &token).await,
        Ok(false)
    ));
    assert!(matches!(admin.held(&caller().tenant_id).await, Ok(v) if v.is_empty()));
    assert!(matches!(
        confirmations
            .claim(&caller(), Uuid::parse_str(&token).unwrap_or_default(), true)
            .await,
        Ok(None)
    ));
}
