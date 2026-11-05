//! Tool registration: search takes the schema Platform publishes.

use std::sync::Arc;

use figment::Figment;
use figment::providers::{Format, Toml};
use serde_json::Value;

use crate::core::config::Config;
use crate::core::tests::support::{FakeQueries, HostedLogins};
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
            description: String::new(),
            params: Vec::new(),
        })
        .collect()
}

/// Every tool definition the engine registers for cfg over a registry of keys.
async fn registered(cfg: &Config, keys: &[&str]) -> Vec<ToolDefinition> {
    let built = build_tools(
        cfg,
        Arc::new(FakeQueries::new(registry(keys))),
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

/// The source keys search offers.
fn live_sources(definitions: &[ToolDefinition]) -> Vec<String> {
    definitions
        .iter()
        .find(|d| d.name == "search")
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
async fn search_offers_the_sources_platform_publishes_and_no_old_search_tools() {
    let definitions = registered(&config(), &["events", "dining"]).await;
    assert_eq!(live_sources(&definitions), vec!["events", "dining"]);
    let old = definitions
        .iter()
        .filter(|d| d.name == "search_live" || d.name == "search_knowledge")
        .count();
    assert_eq!(old, 0);
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
