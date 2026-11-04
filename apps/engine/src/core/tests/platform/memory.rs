//! Memories on the platform: read only, platform kinds mapped to fact, preference and task.

use serde_json::json;

use super::{Fake, MEMBER, caller, client};
use crate::core::traits::memory::MemoryStore;
use crate::core::types::memory::{MemoryKind, NewMemory};
use crate::stores::platform::PlatformMemory;

#[tokio::test]
async fn recall_maps_platform_kinds_and_reads_platform_rows() {
    let fake = Fake::default();
    fake.on(
        "GET",
        &format!("{MEMBER}/memories"),
        200,
        json!({"memories": [{
            "id": "6b1f6d2e-6c1e-4d4e-9a55-0a5c3c2f1b11",
            "kind": "semantic",
            "content": "Takes CSE 310",
            "sensitivity": "normal",
            "confidence": 0.9,
            "source_seq": null,
            "created_at": "2026-10-08T01:00:00.123456",
            "expires_at": "2026-11-08T01:00:00+00:00"
        }, {
            "id": "6b1f6d2e-6c1e-4d4e-9a55-0a5c3c2f1b12",
            "kind": "profile",
            "content": "Wants short answers",
            "sensitivity": "sensitive",
            "confidence": 1.0,
            "created_at": "2026-10-08T01:00:00",
            "expires_at": null
        }]}),
    );
    let Ok(recalled) = PlatformMemory::new(client(&fake).await)
        .recall(&caller())
        .await
    else {
        unreachable!("memories load")
    };
    assert_eq!(
        fake.last().query.get("limit").map(String::as_str),
        Some("500")
    );
    assert_eq!(recalled.len(), 2);
    assert_eq!(recalled[0].content, "Takes CSE 310");
    assert_eq!(recalled[0].kind, MemoryKind::Fact);
    assert!(recalled[0].expires_at.is_some());
    assert_eq!(recalled[1].kind, MemoryKind::Preference);
    assert!(recalled[1].sensitive);
}

#[tokio::test]
async fn an_unknown_kind_is_an_error_and_writes_are_refused() {
    let fake = Fake::default();
    fake.on(
        "GET",
        &format!("{MEMBER}/memories"),
        200,
        json!({"memories": [{
            "id": "6b1f6d2e-6c1e-4d4e-9a55-0a5c3c2f1b11",
            "kind": "dream",
            "content": "x",
            "created_at": "2026-10-08T01:00:00",
            "expires_at": null
        }]}),
    );
    let store = PlatformMemory::new(client(&fake).await);
    assert!(store.recall(&caller()).await.is_err());
    let new = NewMemory {
        kind: MemoryKind::Fact,
        text: "x".into(),
        due: None,
        sensitive: false,
    };
    assert!(store.remember(&caller(), &new).await.is_err());
    assert!(store.forget_all(&caller()).await.is_err());
}
