//! MCP tool risk mapping.

use serde_json::json;

use crate::core::tests::support::ctx;
use crate::core::types::conversation::Visibility;
use crate::core::types::tools::{RiskClass, ToolError};
use crate::runtime::tools::mcp::{
    Fill, compact_schema, is_private, platform_name, platform_risk, required_only, risk_for,
    take_confirm, take_member,
};

#[test]
fn reads_and_inspection_run_freely() {
    for name in [
        "page_navigate",
        "page_snapshot",
        "take_screenshot",
        "find_text",
    ] {
        assert_eq!(risk_for(name), RiskClass::ReadPublic, "{name}");
    }
}

#[test]
fn filling_a_page_in_is_a_draft() {
    for name in ["type_text", "fill_form", "select_option", "hover"] {
        assert_eq!(risk_for(name), RiskClass::PrepareWrite, "{name}");
    }
}

#[test]
fn anything_that_can_commit_the_page_needs_confirmation() {
    // Clicks, key presses, uploads, dialogs, and script evaluation are external writes.
    for name in [
        "click",
        "press_key",
        "evaluate",
        "file_upload",
        "handle_dialog",
    ] {
        assert_eq!(risk_for(name), RiskClass::ExternalWrite, "{name}");
    }
}

#[test]
fn submits_and_unknowns_need_confirmation() {
    assert_eq!(risk_for("submit_form"), RiskClass::ExternalWrite);
    assert_eq!(risk_for("route"), RiskClass::ExternalWrite);
}

#[test]
fn required_only_drops_optional_properties() {
    let schema = json!({
        "type": "object",
        "properties": {
            "url": {"type": "string"},
            "filename": {"type": "string"},
            "target": {"type": "string"}
        },
        "required": ["url"]
    });
    let out = required_only(schema);
    let props = out["properties"]
        .as_object()
        .map(|p| p.keys().cloned().collect::<Vec<_>>());
    assert_eq!(props, Some(vec!["url".to_owned()]));
}

#[test]
fn compact_schema_trims_descriptions_and_noise() {
    let schema = json!({
        "$schema": "x",
        "title": "T",
        "properties": {"a": {"description": "y".repeat(500), "default": 1}}
    });
    let out = compact_schema(schema, 80);
    assert!(out.get("$schema").is_none());
    assert!(out.get("title").is_none());
    assert!(out["properties"]["a"].get("default").is_none());
    assert!(
        out["properties"]["a"]["description"]
            .as_str()
            .is_some_and(|d| d.len() <= 80)
    );
}

#[test]
fn a_pinned_risk_replaces_the_one_derived_from_the_name() {
    use std::collections::BTreeMap;

    use crate::runtime::tools::mcp::pinned_risk;

    let risks = BTreeMap::from([
        ("update_event".to_owned(), RiskClass::Destructive),
        ("list_events".to_owned(), RiskClass::ReadAuthenticated),
    ]);
    assert_eq!(pinned_risk("update_event", &risks), RiskClass::Destructive);
    assert_eq!(
        pinned_risk("list_events", &risks),
        RiskClass::ReadAuthenticated
    );
    assert_eq!(
        pinned_risk("create_event", &risks),
        RiskClass::ExternalWrite
    );
    assert_eq!(pinned_risk("page_snapshot", &risks), RiskClass::ReadPublic);
}

#[test]
fn a_pin_the_server_does_not_list_is_reported() {
    use std::collections::BTreeMap;

    use crate::runtime::tools::mcp::unoffered;

    let risks = BTreeMap::from([
        ("create_event".to_owned(), RiskClass::ExternalWrite),
        ("list_events".to_owned(), RiskClass::ReadAuthenticated),
    ]);
    let offered = vec!["list_events".to_owned()];
    assert_eq!(unoffered(&risks, &offered), vec!["create_event".to_owned()]);
    assert_eq!(unoffered(&BTreeMap::new(), &offered).len(), 0);
}

#[test]
fn platform_tools_get_a_model_safe_name() {
    assert_eq!(platform_name("org.set_modules"), "platform_org_set_modules");
    assert_eq!(platform_name("apps.list"), "platform_apps_list");
}

#[test]
fn platform_risk_follows_the_annotations() {
    assert_eq!(
        platform_risk(Some(true), Some(false)),
        RiskClass::ReadPublic
    );
    assert_eq!(
        platform_risk(Some(false), Some(true)),
        RiskClass::Destructive
    );
    assert_eq!(
        platform_risk(Some(false), Some(false)),
        RiskClass::ExternalWrite
    );
    assert_eq!(platform_risk(None, None), RiskClass::ExternalWrite);
}

#[test]
fn the_confirm_argument_is_hidden_from_the_model() {
    let mut schema = json!({
        "type": "object",
        "properties": {"key": {"type": "string"}, "confirm": {"type": "boolean"}},
        "required": ["key", "confirm"]
    });
    let Some(map) = schema.as_object_mut() else {
        unreachable!("schema is an object")
    };
    assert!(take_confirm(map));
    assert_eq!(
        schema,
        json!({"type": "object", "properties": {"key": {"type": "string"}}, "required": ["key"]})
    );
    let mut plain = json!({"type": "object", "properties": {}});
    let Some(map) = plain.as_object_mut() else {
        unreachable!("schema is an object")
    };
    assert!(!take_confirm(map));
}

#[test]
fn a_private_prefix_matches_the_tools_it_names() {
    let prefixes = vec!["canvas.".to_owned()];
    assert!(is_private("canvas.grades", &prefixes));
    assert!(is_private("canvas.assignment_grades", &prefixes));
    assert!(!is_private("asu.clubs", &prefixes));
    assert!(!is_private("org.info", &prefixes));
    assert!(!is_private("canvas.grades", &[String::new()]));
}

#[test]
fn the_member_argument_is_hidden_from_the_model() {
    let mut schema = json!({
        "type": "object",
        "properties": {"discord_id": {"type": "string"}},
        "required": ["discord_id"],
        "additionalProperties": false
    });
    let Some(map) = schema.as_object_mut() else {
        unreachable!("schema is an object")
    };
    assert!(take_member(map));
    assert_eq!(
        schema,
        json!({"type": "object", "properties": {}, "required": [], "additionalProperties": false})
    );
}

#[test]
fn a_private_platform_tool_refuses_outside_a_direct_message() {
    let fill = Fill {
        member: true,
        private: true,
        ..Fill::default()
    };
    let out = fill.arguments(&ctx(), json!({}));
    assert!(
        matches!(&out, Err(ToolError::Failed(m)) if m.contains("direct message")),
        "{out:?}"
    );
}

#[test]
fn a_private_platform_tool_reads_only_the_caller() {
    let fill = Fill {
        member: true,
        private: true,
        ..Fill::default()
    };
    let dm = ctx().with_visibility(Visibility::Private);
    let Ok(Some(args)) = fill.arguments(&dm, json!({"discord_id": "999"})) else {
        unreachable!("a direct message passes the gate")
    };
    assert_eq!(args.get("discord_id"), Some(&json!("u")));
    let Ok(Some(args)) = fill.arguments(&dm, serde_json::Value::Null) else {
        unreachable!("no arguments still names the caller")
    };
    assert_eq!(args.get("discord_id"), Some(&json!("u")));
}

#[test]
fn a_public_platform_tool_sends_the_arguments_as_given() {
    let Ok(args) = Fill::default().arguments(&ctx(), json!({"discord_id": "999"})) else {
        unreachable!("a public tool runs in a server")
    };
    assert_eq!(
        args.and_then(|a| a.get("discord_id").cloned()),
        Some(json!("999"))
    );
    let confirm = Fill {
        confirm: true,
        ..Fill::default()
    };
    let Ok(Some(args)) = confirm.arguments(&ctx(), serde_json::Value::Null) else {
        unreachable!("a confirmed tool gets confirm")
    };
    assert_eq!(args.get("confirm"), Some(&json!(true)));
}

#[test]
fn a_proxy_error_page_reads_as_html_and_a_result_does_not() {
    use crate::runtime::tools::mcp::is_html;
    assert!(is_html(
        "  <!DOCTYPE html> <html lang=\"en\"><title>Waiting</title>"
    ));
    assert!(is_html("<HTML><body>502</body></HTML>"));
    assert!(!is_html("{\"items\": []}"));
    assert!(!is_html("Hayden Library is open until midnight. <b>"));
}
