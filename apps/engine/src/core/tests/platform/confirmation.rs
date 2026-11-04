//! Confirmations on the platform: held once, claimed once.

use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use super::{Fake, MEMBER, caller, client};
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::types::safety::policy::{PendingAction, ProposedAction};
use crate::core::types::tools::RiskClass;
use crate::stores::platform::PlatformConfirmations;

fn pending() -> PendingAction {
    PendingAction {
        call_id: "c1".into(),
        action: ProposedAction {
            tool: "canvas.submit".into(),
            risk: RiskClass::ExternalWrite,
            arguments: json!({"id": 1}),
            private: false,
        },
    }
}

#[tokio::test]
async fn a_held_action_is_claimed_once() {
    let fake = Fake::default();
    let token = Uuid::new_v4();
    let Ok(action) = serde_json::to_value(pending()) else {
        unreachable!("an action serializes")
    };
    fake.on(
        "PUT",
        &format!("{MEMBER}/pending/{token}"),
        201,
        json!({"held": true}),
    );
    let claim = format!("{MEMBER}/pending/{token}/claim");
    fake.on(
        "POST",
        &claim,
        200,
        json!({"action": action, "payload_hash": "h"}),
    );
    fake.on(
        "POST",
        &claim,
        404,
        json!({"error": "No pending action for this token"}),
    );
    let store = PlatformConfirmations::new(client(&fake).await);

    assert!(
        store
            .hold(&caller(), token, &pending(), "h", Duration::from_mins(2))
            .await
            .is_ok()
    );
    let held = fake.last().body;
    assert_eq!(held["payload_hash"], "h");
    assert_eq!(held["ttl_seconds"], 120);
    assert_eq!(held["action"]["action"]["tool"], "canvas.submit");

    let Ok(Some(claimed)) = store.claim(&caller(), token, true).await else {
        unreachable!("the held action comes back")
    };
    assert_eq!(fake.last().body, json!({"approved": true}));
    assert_eq!(claimed.action.tool, "canvas.submit");
    assert!(matches!(
        store.claim(&caller(), token, false).await,
        Ok(None)
    ));
}
