//! The agent and its helpers: loop limits, task models, compaction, guardrail, traces.

use std::sync::Arc;
use std::time::Duration;

use crate::core::config::Config;
use crate::core::traits::conversation::compaction::Compactor;
use crate::core::traits::model::ModelProvider;
use crate::core::traits::safety::guardrail::Guardrail;
use crate::core::traits::trace::TraceSink;
use crate::core::types::agent::AgentConfig;
use crate::routes::home::HomeRules;
use crate::runtime::harness::agent::ground::Grounding;
use crate::runtime::harness::agent::task::{Task, TaskConfig};
use crate::runtime::harness::compact::{self, ChatCompactor};
use crate::runtime::harness::safety::guardrail::{RuleGuardrail, Rules};
use crate::runtime::harness::trace::{JsonlSink, NullSink};

/// The loop limits and budgets, gathered from the sections that own them.
pub(super) fn agent_config(cfg: &Config) -> AgentConfig {
    AgentConfig {
        provider_name: cfg.telemetry.provider_name.as_str().into(),
        model_name: cfg.model.name.as_str().into(),
        max_steps: cfg.agent.max_steps,
        max_model_retries: cfg.agent.max_model_retries,
        tool_timeout: Duration::from_secs(cfg.agent.tool_timeout_secs),
        answer_reserve: Duration::from_secs(cfg.agent.answer_reserve_secs),
        max_tool_calls: cfg.agent.max_tool_calls,
        confirmation_ttl: Duration::from_secs(cfg.agent.confirmation_ttl_secs),
        max_tokens: cfg.model.max_tokens,
        max_tokens_without_thinking: cfg.model.max_tokens_without_thinking,
        temperature: cfg.agent.temperature,
        upload_preview_chars: cfg.agent.upload_preview_chars,
        upload_match_chars: cfg.agent.upload_match_chars,
        upload_ocr_pages: cfg.agent.upload_ocr_pages,
        history_turns: cfg.agent.history_turns,
        history_keep: cfg.compaction.keep_tokens(cfg.agent.history_budget_tokens),
        retry_base_ms: cfg.agent.retry_base_ms,
        retry_cap_ms: cfg.agent.retry_cap_ms,
        max_span_value_chars: cfg.agent.max_span_value_chars,
        tool_result_to_file_chars: cfg.agent.tool_result_to_file_chars,
        usd_per_m_prompt: cfg.model.usd_per_m_prompt,
        usd_per_m_completion: cfg.model.usd_per_m_completion,
        budget: cfg.agent.budget(),
        thinking: cfg.agent.thinking.clone(),
        stream: cfg.agent.stream,
        stream_block_chars: cfg.agent.stream_block_chars,
    }
}

/// A task config carrying the span settings, with the call settings at their defaults.
pub(super) fn task_config(cfg: &Config) -> TaskConfig {
    TaskConfig {
        provider_name: cfg.telemetry.provider_name.as_str().into(),
        model_name: cfg.model.name.as_str().into(),
        max_span_value_chars: cfg.agent.max_span_value_chars,
        ..TaskConfig::default()
    }
}

/// The chat agent, when compaction is on. Shares the model the loop calls.
pub(super) fn compactor(
    cfg: &Config,
    model: &Arc<dyn ModelProvider>,
) -> Option<Arc<dyn Compactor>> {
    if !cfg.compaction.enabled {
        return None;
    }
    let instructions = cfg
        .compaction
        .instructions
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(compact::INSTRUCTIONS);
    let task = Task::new(
        Arc::clone(model),
        "compaction",
        instructions,
        TaskConfig {
            max_tokens: cfg.compaction.max_tokens,
            temperature: cfg.compaction.temperature,
            timeout: Duration::from_secs(cfg.compaction.timeout_secs),
            ..task_config(cfg)
        },
    );
    Some(Arc::new(ChatCompactor::new(task)))
}

/// The check on answers written without tools, when it is enabled. Shares the model the loop calls.
pub(super) fn grounding(cfg: &Config, model: &Arc<dyn ModelProvider>) -> Option<Grounding> {
    let set = &cfg.grounding;
    set.enabled.then(|| {
        let task = Task::new(
            Arc::clone(model),
            "grounding",
            set.prompt.trim(),
            TaskConfig {
                max_tokens: set.max_tokens,
                temperature: 0.0,
                timeout: Duration::from_secs(set.timeout_secs),
                ..task_config(cfg)
            },
        );
        Grounding::new(task, set.retry_line.trim())
    })
}

/// The response gate, when it is enabled. The registered tool names are protected from answers.
pub(super) fn guardrail(cfg: &Config, tool_names: &[String]) -> Option<Arc<dyn Guardrail>> {
    cfg.guardrail.enabled.then(|| {
        let mut rules = Rules::from(&cfg.guardrail);
        rules.protect_tools(tool_names);
        Arc::new(RuleGuardrail::new(rules)) as Arc<dyn Guardrail>
    })
}

/// Where traces are recorded, or a sink dropping them if off; old traces are pruned here once.
pub(super) fn trace_sink(cfg: &Config) -> anyhow::Result<Arc<dyn TraceSink>> {
    if !cfg.trace.enabled {
        tracing::info!("jsonl traces are off");
        return Ok(Arc::new(NullSink));
    }
    let sink = JsonlSink::new(&cfg.trace.dir, cfg.trace.max_file_bytes)?;
    if cfg.trace.retention_hours > 0 {
        let older_than = Duration::from_secs(cfg.trace.retention_hours * 3_600);
        let pruner = JsonlSink::new(&cfg.trace.dir, cfg.trace.max_file_bytes)?;
        let dir = cfg.trace.dir.clone();
        tokio::spawn(async move {
            loop {
                match pruner.prune(older_than) {
                    Ok(removed) if removed > 0 => {
                        tracing::info!(removed, dir = %dir, "pruned old traces");
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, dir = %dir, "trace prune failed"),
                }
                tokio::time::sleep(Duration::from_hours(1)).await;
            }
        });
    }
    Ok(Arc::new(sink))
}

/// The home chat rules: the welcome, and the choice that moves a new topic into a thread.
pub(super) fn home_rules(cfg: &Config, model: &Arc<dyn ModelProvider>) -> HomeRules {
    let router = cfg.home.threads.then(|| {
        Arc::new(Task::new(
            Arc::clone(model),
            "home_route",
            cfg.home.route_prompt.trim(),
            TaskConfig {
                max_tokens: cfg.home.route_max_tokens,
                temperature: 0.0,
                timeout: Duration::from_secs(cfg.home.route_timeout_secs),
                ..task_config(cfg)
            },
        ))
    });
    HomeRules {
        welcome: cfg.home.welcome.clone(),
        router,
    }
}
