//! Tool registration: search_live offers only the sources the platform publishes.

use std::sync::Arc;

use figment::Figment;
use figment::providers::{Format, Toml};
use serde_json::Value;

use crate::core::config::Config;
use crate::core::tests::support::{FakeQueries, HostedLogins, Stored};
use crate::core::types::knowledge::query::QuerySourceInfo;
use crate::core::types::tools::ToolDefinition;
use crate::wiring::tools::{Personal, build_tools};

/// The sections with no defaults.
const BASE: &str = r#"
[app]
env = "test"
http_addr = "127.0.0.1:0"
log_level = "info"
[engine]
service_token = "t"
[discord]
guild_id = 1
[model]
base_url = "http://localhost:8000/v1"
api_key = ""
name = "m"
max_tokens = 256
[embedding]
base_url = "http://localhost:8001/v1"
api_key = ""
name = "e"
dim = 1024
[platform]
url = "https://platform.example.org"
token = "plat_x"
"#;

fn config() -> Config {
    match Figment::new().merge(Toml::string(BASE)).extract() {
        Ok(cfg) => cfg,
        Err(e) => unreachable!("the test config parses: {e}"),
    }
}

/// A registry that publishes these keys.
fn registry(keys: &[&str]) -> Vec<QuerySourceInfo> {
    keys.iter()
        .map(|key| QuerySourceInfo {
            key: (*key).to_owned(),
            params: Vec::new(),
            indexed: false,
        })
        .collect()
}

/// Every tool definition the engine registers for cfg over a registry of keys.
async fn registered(cfg: &Config, keys: &[&str]) -> Vec<ToolDefinition> {
    let built = build_tools(
        cfg,
        Arc::new(FakeQueries::new(registry(keys))),
        Arc::new(Stored::empty()),
        None,
        Arc::new(HostedLogins),
        Personal::default(),
    )
    .await;
    match built {
        Ok((tools, _, _)) => tools.definitions(),
        Err(e) => unreachable!("the tools build: {e}"),
    }
}

/// The source keys search_live offers.
fn live_sources(definitions: &[ToolDefinition]) -> Vec<String> {
    definitions
        .iter()
        .find(|d| d.name == "search_live")
        .and_then(|d| d.parameters.pointer("/properties/source/enum").cloned())
        .and_then(|keys| match keys {
            Value::Array(keys) => Some(
                keys.iter()
                    .filter_map(|k| k.as_str().map(str::to_owned))
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn search_live_offers_only_the_sources_the_platform_publishes() {
    let offered = live_sources(&registered(&config(), &["events", "dining"]).await);
    assert!(!offered.iter().any(|k| k == "clubs"), "{offered:?}");
    assert!(offered.iter().any(|k| k == "events"), "{offered:?}");
    // The fallback stays offered, so a call that names no source still has one.
    assert!(offered.iter().any(|k| k == "web"), "{offered:?}");
}

#[tokio::test]
async fn upcoming_is_registered_only_with_a_source_and_follows_its_switch() {
    let names =
        |defs: Vec<ToolDefinition>| -> Vec<String> { defs.into_iter().map(|d| d.name).collect() };
    let none = names(registered(&config(), &["events"]).await);
    assert!(!none.iter().any(|n| n == "upcoming"), "{none:?}");
    let with_gcal = |extra: &str| -> Config {
        match Figment::new()
            .merge(Toml::string(&format!(
                "{BASE}[gcal]\nenabled = true\n{extra}"
            )))
            .extract()
        {
            Ok(cfg) => cfg,
            Err(e) => unreachable!("the test config parses: {e}"),
        }
    };
    let on = names(registered(&with_gcal(""), &["events"]).await);
    assert!(on.iter().any(|n| n == "upcoming"), "{on:?}");
    let off = names(registered(&with_gcal("[upcoming]\nenabled = false\n"), &["events"]).await);
    assert!(!off.iter().any(|n| n == "upcoming"), "{off:?}");
    let source_off = names(
        registered(
            &with_gcal("[tools]\ndisabled = [\"google_calendar\"]\n"),
            &["events"],
        )
        .await,
    );
    assert!(
        !source_off.iter().any(|n| n == "upcoming"),
        "{source_off:?}"
    );
}
