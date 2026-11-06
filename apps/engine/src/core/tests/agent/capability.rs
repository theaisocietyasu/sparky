//! The capabilities section: what the model is told it can do, and how each entry is kinded.

use crate::core::types::tools::{RiskClass, ToolDefinition};
use crate::runtime::harness::agent::prompt::capability::{
    Capability, Kind, from_definitions, kind_of, render,
};

fn definition(name: &str, risk: RiskClass) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        description: "does a thing".into(),
        parameters: serde_json::json!({"type": "object"}),
        risk,
        private: false,
        sequential: false,
        timeout_secs: None,
    }
}

#[test]
fn a_remote_tool_is_kinded_by_the_server_it_came_from() {
    let mcp = vec!["remote_lookup".to_owned()];
    assert_eq!(kind_of("remote_lookup", &mcp), Kind::Mcp);
    // A built-in of the same name is still a built-in when no server offers it.
    assert_eq!(kind_of("remote_lookup", &[]), Kind::Tool);
    assert_eq!(kind_of("search_library_hours", &mcp), Kind::Tool);
    assert_eq!(kind_of("run_sandbox", &mcp), Kind::Sandbox);
}

#[test]
fn every_offered_tool_becomes_one_line_naming_its_kind() {
    let defs = vec![
        definition("search_library_hours", RiskClass::ReadPublic),
        definition("remote_lookup", RiskClass::ExternalWrite),
    ];
    let caps = from_definitions(&defs, &["remote_lookup".to_owned()]);
    let text = render(&caps);
    assert!(text.contains("search_library_hours (tool)"), "{text}");
    assert!(text.contains("remote_lookup (mcp)"), "{text}");
    assert_eq!(
        text.lines().count(),
        2,
        "one line per capability, and the heading is written by assembly"
    );
}

#[test]
fn a_capability_that_needs_approval_says_so() {
    // A write tool is listed as needing approval; a read tool is not.
    let caps = from_definitions(&[definition("post", RiskClass::ExternalWrite)], &[]);
    assert!(render(&caps).contains("needs the user to approve"));

    let caps = from_definitions(&[definition("look", RiskClass::ReadPublic)], &[]);
    assert!(!render(&caps).contains("needs the user to approve"));
}

#[test]
fn nothing_offered_writes_no_section() {
    // No capabilities render as an empty string.
    assert_eq!(render(&[]).len(), 0);
}

#[test]
fn the_section_reaches_the_prompt_and_is_capped_by_its_budget() {
    use crate::core::tests::support::ctx;
    use crate::core::types::agent::assemble::{Budget, Sections};
    use crate::runtime::harness::agent::prompt::assemble::assemble;

    let caps = vec![Capability {
        name: "search_library_hours".into(),
        kind: Kind::Tool,
        description: "search the index".into(),
        risk: RiskClass::ReadPublic,
    }];
    let text = render(&caps);

    let out = assemble(
        &ctx(),
        &Sections {
            system: "sys",
            capabilities: &text,
            input: "hi",
            date: "Friday 11 September 2026",
            ..Sections::default()
        },
        Budget::default(),
    );
    let joined: String = out.messages.iter().map(|m| m.content.clone()).collect();
    assert!(joined.contains("search_library_hours (tool)"), "{joined}");

    // A section that does not fit is left out whole.
    let tight = assemble(
        &ctx(),
        &Sections {
            system: "sys",
            capabilities: &text,
            input: "hi",
            date: "Friday 11 September 2026",
            ..Sections::default()
        },
        Budget {
            capabilities: 1,
            ..Budget::default()
        },
    );
    let joined: String = tight.messages.iter().map(|m| m.content.clone()).collect();
    assert!(!joined.contains("search_library_hours (tool)"), "{joined}");
}

#[test]
fn a_public_turn_is_offered_no_private_tool() {
    use std::sync::Arc;

    use crate::core::config::MemorySettings;
    use crate::core::tests::support::{MemorySink, Recalling};
    use crate::core::traits::memory::MemoryStore;
    use crate::core::traits::trace::TraceSink;
    use crate::core::types::conversation::Visibility;
    use crate::runtime::harness::tools::ToolSet;
    use crate::runtime::tools::memory::{MemoryRules, tools};

    let store: Arc<dyn MemoryStore> = Arc::new(Recalling::default());
    let trace: Arc<dyn TraceSink> = Arc::new(MemorySink::new());
    let mut set = ToolSet::new();
    for tool in tools(
        &store,
        &trace,
        MemoryRules::new(&MemorySettings::default(), 0),
    ) {
        set = set.with(tool);
    }
    assert_eq!(set.definitions_for(Visibility::Public), Vec::new());
    assert_eq!(set.definitions_for(Visibility::Private).len(), 2);
}

#[test]
fn an_unknown_tool_name_points_at_the_tools_it_was_close_to() {
    use std::sync::Arc;

    use crate::core::config::MemorySettings;
    use crate::core::tests::support::{MemorySink, Recalling};
    use crate::core::traits::memory::MemoryStore;
    use crate::core::traits::trace::TraceSink;
    use crate::runtime::harness::tools::ToolSet;
    use crate::runtime::tools::memory::{MemoryRules, tools};

    let store: Arc<dyn MemoryStore> = Arc::new(Recalling::default());
    let trace: Arc<dyn TraceSink> = Arc::new(MemorySink::new());
    let mut set = ToolSet::new();
    for tool in tools(
        &store,
        &trace,
        MemoryRules::new(&MemorySettings::default(), 0),
    ) {
        set = set.with(tool);
    }
    let near = set.unknown("forget");
    assert!(
        near.contains("forget_memory") && !near.contains("remember"),
        "{near}"
    );
    let none = set.unknown("search_live");
    assert!(
        none.contains("forget_memory") && none.contains("remember"),
        "{none}"
    );
}
