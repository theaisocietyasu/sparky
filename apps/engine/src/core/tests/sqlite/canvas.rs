//! Canvas feed links in the file: sealed at rest, one per user, replaced and removed.

use sqlx::Row;

use super::{seal, store};
use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::stores::sqlite::Seal;
use secrecy::ExposeSecret;

const LINK: &str = "https://canvas.asu.edu/feeds/calendars/user_AbC123.ics";

#[tokio::test]
async fn a_link_round_trips_and_is_sealed_at_rest() {
    let store = store().await;
    let feeds = store.canvas_feeds(seal());
    assert!(feeds.save("111", LINK).await.is_ok());
    let loaded = feeds.load("111").await.ok().flatten();
    assert_eq!(
        loaded
            .as_ref()
            .map(|l| l.expose_secret().to_owned())
            .as_deref(),
        Some(LINK)
    );
    assert!(matches!(feeds.load("222").await, Ok(None)));
    let Ok(row) = sqlx::query("SELECT sealed FROM canvas_feeds WHERE user_id = '111'")
        .fetch_one(store.pool())
        .await
    else {
        unreachable!("the row is there")
    };
    let sealed: Vec<u8> = row.try_get("sealed").unwrap_or_default();
    assert!(!String::from_utf8_lossy(&sealed).contains("canvas.asu.edu"));
}

#[tokio::test]
async fn a_new_link_replaces_the_old_and_remove_deletes_it() {
    let store = store().await;
    let feeds = store.canvas_feeds(seal());
    let newer = "https://canvas.asu.edu/feeds/calendars/user_New.ics";
    assert!(feeds.save("111", LINK).await.is_ok());
    assert!(feeds.save("111", newer).await.is_ok());
    let loaded = feeds.load("111").await.ok().flatten();
    assert_eq!(
        loaded.map(|l| l.expose_secret().to_owned()).as_deref(),
        Some(newer)
    );
    assert!(matches!(feeds.delete("111").await, Ok(true)));
    assert!(matches!(feeds.delete("111").await, Ok(false)));
    assert!(matches!(feeds.load("111").await, Ok(None)));
}

#[tokio::test]
async fn a_link_sealed_under_another_key_reads_as_not_linked() {
    let store = store().await;
    assert!(store.canvas_feeds(seal()).save("111", LINK).await.is_ok());
    let other = store.canvas_feeds(Seal::new(&[9; 32]));
    assert!(matches!(other.load("111").await, Ok(None)));
}
