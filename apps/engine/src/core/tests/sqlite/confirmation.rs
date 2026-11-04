//! Pending actions in the file: claimed once, by the caller, before expiry.

use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use super::{caller, other, store};
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::types::safety::policy::{PendingAction, ProposedAction};
use crate::core::types::tools::RiskClass;

fn pending() -> PendingAction {
    PendingAction {
        call_id: "c1".into(),
        action: ProposedAction {
            tool: "canvas.submit".into(),
            risk: RiskClass::ExternalWrite,
            arguments: json!({"id": 1}),
        },
    }
}

#[tokio::test]
async fn a_held_action_is_claimed_once_and_only_by_its_caller() {
    let store = store().await.confirmations();
    let token = Uuid::new_v4();
    assert!(
        store
            .hold(&caller(), token, &pending(), "h", Duration::from_mins(2))
            .await
            .is_ok()
    );
    assert!(matches!(store.claim(&other(), token, true).await, Ok(None)));
    let Ok(Some(claimed)) = store.claim(&caller(), token, true).await else {
        unreachable!("the held action comes back")
    };
    assert_eq!(claimed.action.tool, "canvas.submit");
    assert!(matches!(
        store.claim(&caller(), token, false).await,
        Ok(None)
    ));
}

#[tokio::test]
async fn an_expired_action_is_not_claimed() {
    let store = store().await.confirmations();
    let token = Uuid::new_v4();
    assert!(
        store
            .hold(&caller(), token, &pending(), "h", Duration::ZERO)
            .await
            .is_ok()
    );
    assert!(matches!(
        store.claim(&caller(), token, true).await,
        Ok(None)
    ));
}
