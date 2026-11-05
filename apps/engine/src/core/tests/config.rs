//! Configuration defaults, layering, and validation.

use crate::core::config::{Agent, Config, Trace};

/// The values sparky.toml leaves to .env, so the committed file can be loaded on its own.
const SECRETS: &str = r#"
[app]
env = "test"
http_addr = "127.0.0.1:0"
[engine]
service_token = "t"
[discord]
guild_id = 1
[model]
base_url = "http://localhost:8000/v1"
api_key = ""
[platform]
url = "https://platform.example.org"
token = "plat_x"
[memory]
key = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="
"#;

#[test]
fn traces_default_to_the_sparky_state_directory() {
    assert_eq!(Trace::default().dir, ".sparky/traces");
}

#[test]
fn every_section_budget_fits_inside_the_prompt_budget() {
    let a = Agent::default();
    for section in [a.history_budget_tokens, a.memory_budget_tokens] {
        assert!(section <= a.prompt_budget_tokens);
    }
}

#[test]
fn the_committed_sparky_toml_loads_and_validates() {
    use figment::providers::{Format, Toml};

    // The committed sparky.toml loads and validates.
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sparky.toml");
    let cfg: Result<Config, String> = figment::Figment::new()
        .merge(Toml::string(SECRETS))
        .merge(Toml::file(file))
        .extract()
        .map_err(|e| e.to_string());
    match cfg {
        Ok(cfg) => {
            if let Err(e) = cfg.validate() {
                unreachable!("sparky.toml does not validate: {e}")
            }
        }
        Err(e) => unreachable!("sparky.toml does not load: {e}"),
    }
}

/// The SPARKY_SECTION__KEY lines of .env.example as the TOML layer they stand for.
fn env_example_as_toml() -> String {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env.example");
    let Ok(text) = std::fs::read_to_string(path) else {
        unreachable!(".env.example is committed beside sparky.toml")
    };
    let mut sections: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(rest) = key.trim().strip_prefix("SPARKY_") else {
            continue;
        };
        let Some((section, field)) = rest.split_once("__") else {
            continue;
        };
        sections
            .entry(section.to_lowercase())
            .or_default()
            .push((field.to_lowercase(), value.trim().to_owned()));
    }
    let mut out = String::new();
    for (section, fields) in sections {
        let _ = writeln!(out, "[{section}]");
        for (field, value) in fields {
            // A value figment would coerce from the environment is written as that scalar.
            if value.parse::<i64>().is_ok() || value.parse::<bool>().is_ok() {
                let _ = writeln!(out, "{field} = {value}");
            } else {
                let _ = writeln!(
                    out,
                    "{field} = \"{}\"",
                    value.replace('\\', "\\\\").replace('"', "\\\"")
                );
            }
        }
    }
    out
}

#[test]
fn a_fresh_clone_boots_on_sparky_toml_plus_the_env_example() {
    use figment::providers::{Format, Toml};

    // The committed settings plus .env.example with nothing filled in load and validate.
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sparky.toml");
    let cfg: Result<Config, String> = figment::Figment::new()
        .merge(Toml::file(file))
        .merge(Toml::string(&env_example_as_toml()))
        .extract()
        .map_err(|e| e.to_string());
    match cfg {
        Ok(cfg) => {
            if let Err(e) = cfg.validate() {
                unreachable!(
                    "a clone that copied .env.example does not boot: {e}. \
                     Add the setting to .env.example, or change the default it fights."
                )
            }
        }
        Err(e) => unreachable!("sparky.toml plus .env.example does not load: {e}"),
    }
}

/// Loads a config from the sections that have no defaults plus extra.
fn load(extra: &str) -> Result<Config, String> {
    use figment::providers::{Format, Toml};

    let base = r#"
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
[memory]
key = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc="
"#;
    let cfg: Config = figment::Figment::new()
        .merge(Toml::string(base))
        .merge(Toml::string(extra))
        .extract()
        .map_err(|e| e.to_string())?;
    cfg.validate().map_err(|e| e.to_string())?;
    Ok(cfg)
}

/// The config extra produces, or a failure naming why it did not load.
fn ok(extra: &str) -> Config {
    match load(extra) {
        Ok(cfg) => cfg,
        Err(e) => unreachable!("expected a valid config, got {e}"),
    }
}

/// The rejection extra produces, or a failure saying it was accepted.
fn err(extra: &str) -> String {
    match load(extra) {
        Ok(_) => unreachable!("expected {extra:?} to be rejected"),
        Err(e) => e,
    }
}

#[test]
fn a_bare_config_is_valid_and_takes_every_default() {
    let cfg = ok("");
    assert_eq!(cfg.agent.chars_per_token, 4);
    assert!(cfg.trace.enabled);
    assert_eq!(cfg.policy.write_roles, vec!["MANAGE_GUILD".to_owned()]);
}

#[test]
fn a_section_budget_above_the_prompt_budget_is_rejected() {
    let e = err("[agent]\nprompt_budget_tokens = 1000\nhistory_budget_tokens = 4000\n");
    assert!(e.contains("history_budget_tokens"), "{e}");
}

#[test]
fn a_sample_ratio_outside_zero_to_one_is_rejected() {
    let e = err("[telemetry]\nsample_ratio = 2.0\n");
    assert!(e.contains("sample_ratio"), "{e}");
}

#[test]
fn an_empty_provider_name_is_rejected() {
    let e = err("[telemetry]\nprovider_name = \" \"\n");
    assert!(e.contains("provider_name"), "{e}");
}

#[test]
fn an_empty_project_name_is_rejected() {
    let e = err("[telemetry]\nproject_name = \" \"\n");
    assert!(e.contains("project_name"), "{e}");
}

#[test]
fn telemetry_exports_nothing_until_phoenix_is_set() {
    use secrecy::ExposeSecret;

    let cfg = ok("");
    assert!(cfg.telemetry.phoenix_url.is_none());
    assert_eq!(cfg.telemetry.phoenix_api_key.expose_secret().len(), 0);
    assert_eq!(cfg.telemetry.project_name, "sparky");
    assert_eq!(cfg.telemetry.provider_name, "llama.cpp");
}

#[test]
fn no_mcp_server_is_configured_by_default_and_an_empty_url_is_dropped() {
    assert!(ok("").mcp.resolved_servers().is_empty());
    let cfg = ok("[[mcp.servers]]\nname = \"a\"\nurl = \"\"\n\
                  [[mcp.servers]]\nname = \"b\"\nurl = \"http://one/mcp\"\n");
    let servers = cfg.mcp.resolved_servers();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "b");
}

#[test]
fn two_servers_sharing_a_name_are_rejected() {
    let e = err("[[mcp.servers]]\nname = \"a\"\nurl = \"http://one\"\n\
                 [[mcp.servers]]\nname = \"a\"\nurl = \"http://two\"\n");
    assert!(e.contains("two servers named"), "{e}");
}

/// The provider request body built from [model], or a failure naming why it was not.
fn params(cfg: &Config) -> serde_json::Value {
    match cfg.model.additional_params() {
        Ok(value) => value,
        Err(e) => unreachable!("expected a request body, got {e}"),
    }
}

#[test]
fn sampling_sends_only_what_is_set() {
    // Everything unset is left to the server defaults.
    let cfg = ok("[model.sampling]\ntop_p = 0.9\nseed = 7\n");
    let sent = params(&cfg);
    assert_eq!(sent["top_p"], 0.9);
    assert_eq!(sent["seed"], 7);
    assert!(sent.get("top_k").is_none(), "unset fields are not sent");
    assert!(
        sent.get("chat_template_kwargs").is_none(),
        "thinking is set per call"
    );
}

#[test]
fn thinking_is_automatic_by_default_and_takes_a_fixed_mode() {
    use crate::core::types::agent::thinking::ThinkingMode;

    assert_eq!(ok("").agent.thinking.mode, ThinkingMode::Auto);
    let cfg = ok("[agent.thinking]\nmode = \"off\"\n");
    assert_eq!(cfg.agent.thinking.mode, ThinkingMode::Off);
    assert!(err("[agent.thinking]\nmode = \"sometimes\"\n").contains("mode"));
}

#[test]
fn a_prompt_estimate_headroom_out_of_range_is_rejected() {
    let e = err("[agent]\nprompt_estimate_headroom = -0.1\n");
    assert!(e.contains("prompt_estimate_headroom"), "{e}");
}

#[test]
fn a_completion_budget_of_zero_is_rejected() {
    let e = err("[model]\nmax_tokens_without_thinking = 0\n");
    assert!(e.contains("max_tokens_without_thinking"), "{e}");
    assert_eq!(ok("").model.max_tokens_without_thinking, 1024);
}

#[test]
fn a_thinking_cue_with_no_word_is_rejected() {
    let e = err("[agent.thinking]\ncues = [\"why\", \" ? \"]\n");
    assert!(e.contains("agent.thinking.cues"), "{e}");
}

#[test]
fn extra_params_json_may_not_set_thinking() {
    let cfg = ok(
        "[model]\nextra_params_json = \"{\\\"chat_template_kwargs\\\": {\\\"enable_thinking\\\": true}}\"\n",
    );
    match cfg.model.additional_params() {
        Ok(value) => unreachable!("thinking is not a static field, got {value}"),
        Err(e) => assert!(e.to_string().contains("agent.thinking.mode"), "{e}"),
    }
    let kept = ok(
        "[model]\nextra_params_json = \"{\\\"chat_template_kwargs\\\": {\\\"custom\\\": 1}}\"\n",
    );
    assert_eq!(params(&kept)["chat_template_kwargs"]["custom"], 1);
}

#[test]
fn extra_params_json_wins_over_sampling() {
    let cfg = ok("[model]\nextra_params_json = \"{\\\"top_p\\\": 0.1}\"\n\
                  [model.sampling]\ntop_p = 0.9\n");
    assert_eq!(params(&cfg)["top_p"], 0.1);
}

#[test]
fn extra_params_that_is_not_a_json_object_is_reported() {
    let cfg = ok("[model]\nextra_params_json = \"[1,2]\"\n");
    match cfg.model.additional_params() {
        Ok(value) => unreachable!("an array is not a request body, got {value}"),
        Err(e) => assert!(e.to_string().contains("JSON object"), "{e}"),
    }
}

/// The system prompt cfg resolves to, or a failure naming why it did not resolve.
fn prompt(cfg: &Config) -> String {
    match cfg.system_prompt("built-in") {
        Ok(text) => text,
        Err(e) => unreachable!("expected a prompt, got {e}"),
    }
}

#[test]
fn the_system_prompt_falls_back_then_yields_to_config_then_to_a_file() {
    assert_eq!(prompt(&ok("")), "built-in");
    assert_eq!(prompt(&ok("[prompt]\nsystem = \"  inline  \"\n")), "inline");

    let dir = std::env::temp_dir().join(format!("sparky-prompt-{}", uuid::Uuid::new_v4()));
    let Ok(()) = std::fs::create_dir_all(&dir) else {
        unreachable!("could not create a temp directory")
    };
    let path = dir.join("system.md");
    let Ok(()) = std::fs::write(&path, "from a file\n") else {
        unreachable!("could not write the prompt file")
    };
    let cfg = ok(&format!(
        "[prompt]\nsystem = \"inline\"\nsystem_file = \"{}\"\n",
        path.display()
    ));
    assert_eq!(prompt(&cfg), "from a file", "the file wins over `system`");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_system_prompt_file_fails_at_boot() {
    // A named system_file that cannot be read fails the boot.
    let cfg = ok("[prompt]\nsystem_file = \"/nonexistent/sparky-prompt.md\"\n");
    assert!(cfg.system_prompt("built-in").is_err());
}

#[test]
fn a_setting_that_moved_sections_fails_the_boot_rather_than_being_ignored() {
    // A key that moved sections is rejected and named; other unknown keys are not.
    let stale = "SPARKY_AGENT__TRACE_DIR";
    match Config::reject_renamed(|key| key == stale) {
        Ok(()) => unreachable!("a moved setting must not be ignored"),
        Err(e) => {
            let message = e.to_string();
            assert!(message.contains(stale), "{message}");
            assert!(
                message.contains("SPARKY_TRACE__DIR"),
                "it names its replacement: {message}"
            );
        }
    }
    assert!(
        Config::reject_renamed(|_| false).is_ok(),
        "nothing stale set"
    );
}

#[test]
fn a_leftover_profile_section_still_loads() {
    let cfg = ok("[profile]\nenabled = true\nlist_limit = 0\n[profile.detector]\nmin_words = 4\n");
    assert_eq!(cfg.memory.max_per_user, 50);
}

#[test]
fn old_search_retrieval_and_embedding_settings_still_load() {
    let cfg = ok(
        "[tools]\nlive_default_source = \"web\"\nstale_note = \"x\"\n\
                  [retrieval]\ndense = false\n[embedding]\nname = \"e\"\n",
    );
    assert!(cfg.tools.search);
}

#[test]
fn sandbox_egress_names_its_network_and_proxy() {
    let e = err("[sandbox]\nenabled = true\negress = true\negress_network = \" \"\n");
    assert!(e.contains("sandbox.egress_network"), "{e}");
    let e = err("[sandbox]\nenabled = true\negress = true\negress_proxy_image = \"\"\n");
    assert!(e.contains("sandbox.egress_proxy_image"), "{e}");
    assert!(load("[sandbox]\nenabled = true\negress = true\n").is_ok());
}

#[test]
fn the_committed_google_calendar_tool_is_off() {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sparky.toml");
    let cfg: Result<Config, String> = {
        use figment::providers::{Format, Toml};
        figment::Figment::new()
            .merge(Toml::string(SECRETS))
            .merge(Toml::file(file))
            .extract()
            .map_err(|e| e.to_string())
    };
    let Ok(cfg) = cfg else {
        unreachable!("sparky.toml loads")
    };
    assert!(!cfg.gcal.enabled);
    assert!(!cfg.mcp.servers.iter().any(|s| s.name == "google_calendar"));
}

#[test]
fn a_risk_pinned_outside_the_exposed_tools_is_rejected() {
    let e = err("[[mcp.servers]]\nname = \"c\"\nurl = \"http://one/mcp\"\n\
                 tools = [\"list_events\"]\n\
                 [mcp.servers.risks]\ncreate_event = \"external_write\"\n");
    assert!(e.contains("create_event"), "{e}");
    ok("[[mcp.servers]]\nname = \"c\"\nurl = \"http://one/mcp\"\n\
        [mcp.servers.risks]\ncreate_event = \"external_write\"\n");
}

#[test]
fn platform_tools_need_an_http_mcp_url() {
    let on = "[platform]\nurl = \"http://127.0.0.1:8000\"\ntoken = \"plat_x\"\n";
    assert_eq!(ok(on).platform.mcp_endpoint(), None);
    let with = format!("{on}mcp_url = \" http://127.0.0.1:8001/mcp \"\n");
    assert_eq!(
        ok(&with).platform.mcp_endpoint(),
        Some("http://127.0.0.1:8001/mcp")
    );
    let e = err(&format!("{on}mcp_url = \"127.0.0.1:8001\"\n"));
    assert!(e.contains("platform.mcp_url"), "{e}");
}

#[test]
fn the_store_defaults_to_the_sqlite_file_with_backups_off() {
    let cfg = ok("");
    assert_eq!(cfg.store.backend, crate::core::config::StoreBackend::Sqlite);
    assert_eq!(cfg.store.path, ".sparky/sparky.db");
    assert_eq!(cfg.store.backup_dir, "");
    assert_eq!(cfg.store.import_days, 30);
}

#[test]
fn a_store_that_cannot_open_or_back_up_is_rejected() {
    let e = err("[store]\npath = \"\"\n");
    assert!(e.contains("store.path"), "{e}");
    let e = err("[store]\nmax_connections = 0\n");
    assert!(e.contains("store.max_connections"), "{e}");
    let e = err("[store]\nbackup_dir = \"b\"\nbackups_kept = 0\n");
    assert!(e.contains("store.backups_kept"), "{e}");
}

#[test]
fn the_memory_key_must_be_32_bytes_of_base64_unless_the_store_is_the_platform() {
    let e = err("[memory]\nkey = \"not base64!\"\n");
    assert!(e.contains("SPARKY_MEMORY__KEY"), "{e}");
    let e = err("[memory]\nkey = \"AAAA\"\n");
    assert!(e.contains("32 bytes"), "{e}");
    let cfg = ok("[store]\nbackend = \"platform\"\n[memory]\nkey = \"AAAA\"\n");
    assert_eq!(
        cfg.store.backend,
        crate::core::config::StoreBackend::Platform
    );
}

#[test]
fn memory_limits_and_the_prune_interval_must_be_positive() {
    let e = err("[memory]\nmax_per_user = 0\n");
    assert!(e.contains("memory.max_per_user"), "{e}");
    let e = err("[memory]\nmax_chars = 0\n");
    assert!(e.contains("memory.max_chars"), "{e}");
    let e = err("[store]\nprune_every_hours = 0\n");
    assert!(e.contains("store.prune_every_hours"), "{e}");
    let cfg = ok("");
    assert_eq!(cfg.store.conversation_idle_days, 0);
    assert!(cfg.memory.tools);
    assert_eq!(cfg.memory.max_per_user, 50);
}

#[test]
fn the_web_sign_in_needs_its_app_key_urls_and_a_listed_site() {
    let on = |extra: &str| {
        format!(
            "[web]\nenabled = true\ndiscord_client_id = \"1\"\ndiscord_client_secret = \"s\"\n\
             session_key = \"BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=\"\n\
             public_url = \"https://engine.example.org\"\nsite_url = \"https://sparky.example.org\"\n{extra}"
        )
    };
    let listed = "[http]\ncors_origins = [\"https://sparky.example.org\"]\n";
    let cfg = ok(&on(listed));
    assert!(cfg.web.enabled);
    assert_eq!(cfg.web.session_hours, 12);
    let e = err(&on(""));
    assert!(e.contains("cors_origins must list"), "{e}");
    let e = err(&on(
        "[http]\ncors_origins = [\"https://sparky.example.org\", \"*\"]\n",
    ));
    assert!(e.contains("must not be *"), "{e}");
    let e = err(&on(listed).replace(
        "discord_client_secret = \"s\"",
        "discord_client_secret = \"\"",
    ));
    assert!(e.contains("SPARKY_WEB__DISCORD_CLIENT_SECRET"), "{e}");
    let e = err(&on(listed).replace("https://engine", "http://engine"));
    assert!(e.contains("SPARKY_WEB__PUBLIC_URL"), "{e}");
    let e = err(&on(listed).replace("BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=", "AAAA"));
    assert!(e.contains("32 bytes") || e.contains("SESSION_KEY"), "{e}");
    let numeric = ok(&on(listed).replace(
        "discord_client_id = \"1\"",
        "discord_client_id = 1302499768966185012",
    ));
    assert_eq!(
        numeric.web.discord_client_id.as_deref(),
        Some("1302499768966185012"),
        "an unquoted id, as an env var parses it, is read as text"
    );
    assert!(!ok("").web.enabled);
}

#[test]
fn a_wasm_sandbox_has_no_egress_and_names_its_folders() {
    let e = err("[sandbox]\nenabled = true\nruntime = \"wasm\"\negress = true\n");
    assert!(e.contains("no network"), "{e}");
    let e = err("[sandbox]\nenabled = true\nruntime = \"wasm\"\nwasm_work = \"\"\n");
    assert!(e.contains("sandbox.wasm_work"), "{e}");
    assert!(load("[sandbox]\nenabled = true\nruntime = \"wasm\"\n").is_ok());
}
