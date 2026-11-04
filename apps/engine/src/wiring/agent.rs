//! The agent and its helpers: loop limits, task models, profile, compaction, guardrail, traces.

use std::sync::Arc;
use std::time::Duration;

use crate::core::config::Config;
use crate::core::traits::conversation::compaction::Compactor;
use crate::core::traits::memory::detector::FactDetector;
use crate::core::traits::memory::profile::ProfileGraph;
use crate::core::traits::model::ModelProvider;
use crate::core::traits::safety::guardrail::Guardrail;
use crate::core::traits::trace::TraceSink;
use crate::core::types::agent::AgentConfig;
use crate::runtime::harness::agent::task::{Task, TaskConfig};
use crate::runtime::harness::compact::{self, ChatCompactor};
use crate::runtime::harness::memory::detect::{RuleDetector, Rules as DetectorRules};
use crate::runtime::harness::memory::profile::{self, GraphAgent, ProfileWriter, Reconciler};
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
        confirmation_ttl: Duration::from_secs(cfg.agent.confirmation_ttl_secs),
        max_tokens: cfg.model.max_tokens,
        max_tokens_without_thinking: cfg.model.max_tokens_without_thinking,
        temperature: cfg.agent.temperature,
        upload_preview_chars: cfg.agent.upload_preview_chars,
        upload_match_chars: cfg.agent.upload_match_chars,
        upload_ocr_pages: cfg.agent.upload_ocr_pages,
        history_turns: cfg.agent.history_turns,
        history_keep: cfg.compaction.keep_tokens(cfg.agent.history_budget_tokens),
        memory_recall_limit: cfg.agent.memory_recall_limit,
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

/// The classifier and the graph agent, when profile recording is on.
pub(super) fn profile_writer(
    cfg: &Config,
    model: &Arc<dyn ModelProvider>,
    graph: Option<Arc<dyn ProfileGraph>>,
) -> Option<Arc<ProfileWriter>> {
    let graph = graph?;
    let budget = Duration::from_secs(cfg.profile.timeout_secs);
    let instructions = |set: Option<&String>, fallback: &'static str| {
        set.map(String::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    };
    // The gate is rule based.
    let detector: Arc<dyn FactDetector> = Arc::new(RuleDetector::new(DetectorRules::from(
        &cfg.profile.detector,
    )));
    let agent = GraphAgent::new(Task::new(
        Arc::clone(model),
        "profile.extract",
        instructions(
            cfg.profile.graph_instructions.as_ref(),
            profile::GRAPH_INSTRUCTIONS,
        ),
        TaskConfig {
            max_tokens: cfg.profile.max_tokens,
            temperature: 0.0,
            timeout: budget,
            ..task_config(cfg)
        },
    ));
    let reconciler = cfg.profile.reconcile.then(|| {
        Reconciler::new(Task::new(
            Arc::clone(model),
            "profile.reconcile",
            instructions(
                cfg.profile.reconcile_instructions.as_ref(),
                profile::RECONCILE_INSTRUCTIONS,
            ),
            TaskConfig {
                // A list of numbers, or the word none.
                max_tokens: 32,
                temperature: 0.0,
                timeout: budget,
                ..task_config(cfg)
            },
        ))
    });
    Some(Arc::new(ProfileWriter::new(
        detector,
        agent,
        reconciler,
        graph,
        budget,
        cfg.profile.min_confidence,
    )))
}

/// The response gate, when it is enabled. The registered tool names are protected from answers.
pub(super) fn guardrail(cfg: &Config, tool_names: &[String]) -> Option<Arc<dyn Guardrail>> {
    cfg.guardrail.enabled.then(|| {
        let mut rules = Rules::from(&cfg.guardrail);
        rules.protect(tool_names.iter().cloned());
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
