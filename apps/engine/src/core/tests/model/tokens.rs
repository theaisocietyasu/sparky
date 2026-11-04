//! The prompt token estimator, and the settings the harness types are built from.

use crate::core::config::Agent;
use crate::core::types::agent::AgentConfig;
use crate::core::types::agent::assemble::Budget;
use crate::core::types::conversation::message::Message;
use crate::core::types::model::tokens::estimate;

#[test]
fn assembly_and_a_message_price_the_same_text_the_same_way() {
    let text = "Hayden Library is open until 2am on weekdays during finals week.";
    for cpt in [2, 4, 8] {
        assert_eq!(
            estimate(text, cpt),
            Message::user(text).estimated_tokens(cpt),
            "a message with no tool calls costs what the estimator says at {cpt} chars per token"
        );
    }
}

#[test]
fn tool_call_arguments_are_counted_on_top_of_the_content() {
    let mut with_call = Message::assistant("");
    with_call.tool_calls = vec![crate::core::types::conversation::message::ToolCall {
        id: "1".into(),
        name: "search_library_hours".into(),
        arguments: serde_json::json!({"query": "library hours"}),
    }];
    assert!(with_call.estimated_tokens(4) > Message::assistant("").estimated_tokens(4));
}

#[test]
fn a_zero_divisor_does_not_divide_by_zero() {
    assert!(estimate("anything", 0) > 0);
}

#[test]
fn the_loop_config_carries_the_agent_settings_unchanged() {
    // The loop config defaults match the agent settings defaults.
    let settings = Agent::default();
    let cfg = AgentConfig::default();
    assert_eq!(cfg.max_steps, settings.max_steps);
    assert_eq!(cfg.max_model_retries, settings.max_model_retries);
    assert!((cfg.temperature - settings.temperature).abs() < f32::EPSILON);
    assert_eq!(cfg.history_turns, settings.history_turns);
    assert_eq!(cfg.retry_base_ms, settings.retry_base_ms);
    assert_eq!(cfg.retry_cap_ms, settings.retry_cap_ms);
    assert_eq!(cfg.max_span_value_chars, settings.max_span_value_chars);
    assert_eq!(cfg.tool_timeout.as_secs(), settings.tool_timeout_secs);
    assert_eq!(
        cfg.confirmation_ttl.as_secs(),
        settings.confirmation_ttl_secs
    );
}

#[test]
fn the_prompt_budget_carries_the_agent_settings_unchanged() {
    let settings = Agent::default();
    let budget = Budget::default();
    assert_eq!(budget.total, settings.prompt_budget_tokens);
    assert_eq!(budget.history, settings.history_budget_tokens);
    assert_eq!(budget.memory, settings.memory_budget_tokens);
    assert_eq!(budget.chars_per_token, settings.chars_per_token);
}
