//! Pages read_page read, sent to the platform with the URL only.

use serde_json::json;

use super::{Fake, client};
use crate::core::traits::knowledge::discovery::Discovery;
use crate::stores::platform::PlatformDiscovery;

const DISCOVERED: &str = "/api/knowledge/discovered";

#[tokio::test]
async fn a_page_is_sent_as_its_url_only() {
    let fake = Fake::default();
    fake.on(
        "POST",
        DISCOVERED,
        201,
        json!({"key": "read/asu.edu/abc", "status": "added"}),
    );
    let discovery = PlatformDiscovery::new(client(&fake).await);
    let result = discovery.suggest("https://asu.edu/hours").await;
    assert!(result.is_ok());
    let sent = fake.last();
    assert_eq!(sent.path, DISCOVERED);
    assert_eq!(sent.body, json!({"url": "https://asu.edu/hours"}));
}

#[tokio::test]
async fn a_refused_page_is_an_error() {
    let fake = Fake::default();
    fake.on("POST", DISCOVERED, 403, json!({"error": "scope"}));
    let discovery = PlatformDiscovery::new(client(&fake).await);
    assert!(discovery.suggest("https://asu.edu/hours").await.is_err());
}
