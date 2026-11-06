//! Settings in chat: check-ins, a pasted Canvas link, and unlinking, over the real file.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::core::tests::support::HostedLogins;
use crate::core::traits::nudge::NudgeStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::nudge::{NudgeKind, NudgePrefs, Person};
use crate::core::types::tools::canvas::feed_in;
use crate::core::types::tools::{RiskClass, ToolError};
use crate::runtime::tools::settings::{self, Accounts, SettingsStores};
use crate::stores::sqlite::{Seal, SqliteStore, StoreOptions};

const LINK: &str = "https://canvas.asu.edu/feeds/calendars/user_AbC123.ics";

async fn store() -> SqliteStore {
    let options = StoreOptions {
        path: "sqlite::memory:".into(),
        max_connections: 1,
        busy_timeout: Duration::from_secs(1),
    };
    match SqliteStore::open(&options).await {
        Ok(store) => store,
        Err(e) => unreachable!("an in-memory store opens: {e}"),
    }
}

fn ctx() -> RequestContext {
    RequestContext::new("guild", "111", Duration::from_secs(5))
}

fn me() -> Person {
    Person {
        tenant: "guild".into(),
        user: "111".into(),
    }
}

async fn stores() -> (SettingsStores, Arc<dyn NudgeStore>, Arc<dyn CanvasFeeds>) {
    let file = store().await;
    let nudges: Arc<dyn NudgeStore> = Arc::new(file.nudges(Seal::new(&[7; 32])));
    let feeds: Arc<dyn CanvasFeeds> = Arc::new(file.canvas_feeds(Seal::new(&[7; 32])));
    let stores = SettingsStores {
        nudges: Some(Arc::clone(&nudges)),
        nudge_defaults: Some(NudgePrefs {
            quiet_start: 22,
            quiet_end: 8,
            off: Vec::new(),
        }),
        canvas: Some(Arc::clone(&feeds)),
        accounts: Some(Accounts {
            store: Arc::new(HostedLogins),
            names: Vec::new(),
        }),
    };
    (stores, nudges, feeds)
}

fn by_name(tools: &[Arc<dyn Tool>], name: &str) -> Arc<dyn Tool> {
    match tools.iter().find(|t| t.definition().name == name) {
        Some(tool) => Arc::clone(tool),
        None => unreachable!("{name} is registered"),
    }
}

#[tokio::test]
async fn quiet_hours_and_kinds_are_changed_in_chat() {
    let (stores, nudges, _) = stores().await;
    let tool = by_name(&settings::tools(stores), "settings");
    let quiet = tool
        .call(
            &ctx(),
            json!({"action": "quiet_hours", "quiet_start": 23, "quiet_end": 9}),
        )
        .await;
    assert!(matches!(&quiet, Ok(out) if out.content.contains("quiet from 23:00 to 9:00")));
    let off = tool
        .call(
            &ctx(),
            json!({"action": "nudges_off", "kinds": ["brief", "tips"]}),
        )
        .await;
    assert!(matches!(&off, Ok(out) if out.content.contains("brief, tips")));
    let on = tool
        .call(&ctx(), json!({"action": "nudges_on", "kinds": ["tips"]}))
        .await;
    assert!(on.is_ok());
    let saved = nudges.prefs(&me()).await.ok().flatten();
    assert_eq!(
        saved,
        Some(NudgePrefs {
            quiet_start: 23,
            quiet_end: 9,
            off: vec![NudgeKind::Brief],
        })
    );
}

#[tokio::test]
async fn a_wrong_kind_lists_the_kinds() {
    let (stores, _, _) = stores().await;
    let tool = by_name(&settings::tools(stores), "settings");
    let wrong = tool
        .call(&ctx(), json!({"action": "nudges_off", "kinds": ["spam"]}))
        .await;
    assert!(matches!(wrong, Err(ToolError::InvalidArguments(m)) if m.contains("due_soon")));
}

#[tokio::test]
async fn show_says_canvas_is_linked_once_a_link_is_saved() {
    let (stores, _, feeds) = stores().await;
    let tool = by_name(&settings::tools(stores), "settings");
    let before = tool.call(&ctx(), json!({"action": "show"})).await;
    assert!(matches!(&before, Ok(out) if out.content.contains("Canvas calendar: not linked")));
    assert!(feeds.save("111", LINK).await.is_ok());
    let after = tool.call(&ctx(), json!({"action": "show"})).await;
    assert!(matches!(&after, Ok(out) if out.content.contains("Canvas calendar: linked.")));
}

#[tokio::test]
async fn disconnect_is_confirmed_and_removes_the_canvas_link() {
    let (stores, _, feeds) = stores().await;
    let tool = by_name(&settings::tools(stores), "disconnect");
    assert_eq!(tool.definition().risk, RiskClass::Destructive);
    assert!(feeds.save("111", LINK).await.is_ok());
    let out = tool.call(&ctx(), json!({"account": "canvas"})).await;
    assert!(matches!(&out, Ok(o) if o.content == "Unlinked canvas."));
    assert!(matches!(feeds.load("111").await, Ok(None)));
}

#[test]
fn a_feed_link_is_found_inside_a_message() {
    let hosts = vec!["canvas.asu.edu".to_owned()];
    let found = feed_in(&format!("here it is: <{LINK}>, thanks"), &hosts);
    assert_eq!(found, Some((LINK.to_owned(), LINK.to_owned())));
    assert_eq!(feed_in("my canvas is canvas.asu.edu", &hosts), None);
}
