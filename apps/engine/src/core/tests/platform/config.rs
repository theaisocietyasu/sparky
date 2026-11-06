//! The platform section: what it needs, and what it rules out.

use figment::Figment;
use figment::providers::{Format, Toml};

use crate::core::config::Config;

/// The sections with no defaults, without the platform section.
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
[memory]
key = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="
"#;

/// A complete platform section.
const ON: &str = r#"
[platform]
url = "https://platform.example.org"
token = "plat_x"
"#;

fn load(extra: &str) -> Result<Config, String> {
    let cfg: Config = Figment::new()
        .merge(Toml::string(BASE))
        .merge(Toml::string(extra))
        .extract()
        .map_err(|e| e.to_string())?;
    cfg.validate().map_err(|e| e.to_string())?;
    Ok(cfg)
}

#[test]
fn a_complete_platform_section_boots_with_its_defaults() {
    let Ok(cfg) = load(ON) else {
        unreachable!("a complete platform section is valid")
    };
    assert_eq!(cfg.platform.timeout_secs, 10);
}

#[test]
fn the_platform_needs_a_url_and_a_token() {
    assert!(load("").is_err_and(|e| e.contains("SPARKY_PLATFORM__URL")));
    let no_url = "[platform]\ntoken = \"plat_x\"\n";
    assert!(load(no_url).is_err_and(|e| e.contains("SPARKY_PLATFORM__URL")));
    let bad_url = "[platform]\nurl = \"ftp://x\"\ntoken = \"plat_x\"\n";
    assert!(load(bad_url).is_err_and(|e| e.contains("SPARKY_PLATFORM__URL")));
    let no_token = "[platform]\nurl = \"https://p.example\"\n";
    assert!(load(no_token).is_err_and(|e| e.contains("SPARKY_PLATFORM__TOKEN")));
}

#[test]
fn settings_the_platform_cannot_serve_are_rejected() {
    let with = |more: &str| load(&format!("{ON}{more}"));
    assert!(with("timeout_secs = 0\n").is_err());
    assert!(
        with("[agent]\nconfirmation_ttl_secs = 90000\n")
            .is_err_and(|e| e.contains("confirmation_ttl_secs"))
    );
}

#[test]
fn platform_tools_the_engine_serves_or_officers_use_are_skipped_unless_told_otherwise() {
    let Ok(cfg) = load(ON) else {
        unreachable!("a complete platform section is valid")
    };
    assert_eq!(cfg.platform.mcp_skip, vec!["batch", "search", "modules."]);
    assert_eq!(cfg.platform.mcp_private, Vec::<String>::new());
    assert!(load(&format!("{ON}mcp_private = [\"\"]\n")).is_err_and(|e| e.contains("mcp_private")));
    assert!(load(&format!("{ON}mcp_skip = [\"\"]\n")).is_err_and(|e| e.contains("mcp_skip")));
}
