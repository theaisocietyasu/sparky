//! Construct concrete adapters, hand them to the harness, build the router, serve.
//!
//! Knowledge, live queries and linked accounts are the platform's. Conversations, memories and
//! pending actions are in the SQLite file unless store.backend is platform. Everything past
//! Stores sees traits alone.

mod agent;
mod boot;
mod http;
mod import;
mod platform;
mod prompt;
mod store;
pub(crate) mod tools;

use std::sync::Arc;
use std::time::Duration;

use crate::core::config::{Config, StoreBackend};
use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::health::Probe;
use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::traits::knowledge::retrieval::{Embedder, Retriever};
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::memory::profile::ProfileGraph;
use crate::core::traits::model::ModelProvider;
use crate::core::traits::oauth::OAuthStore;
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::traits::tools::files::FileSource;
use crate::core::traits::tools::sandbox::Sandbox;
use crate::core::traits::trace::TraceSink;
use crate::runtime::harness::agent::prompt::capability;
use crate::runtime::harness::agent::{Agent, AgentDeps, PromptText};
use crate::runtime::harness::safety::policy::RiskPolicy;
use crate::runtime::harness::trace::Fanout;
use crate::runtime::model::limit::Limited;
use crate::runtime::model::rig_openai::{self, RigChat, RigEmbedder};
use crate::runtime::tools::files::HttpFiles;

use self::agent::{agent_config, compactor, guardrail, profile_writer, trace_sink};
use self::boot::{fits_the_prompt, fits_the_slot};
use self::http::{chat_state, http_router, memory_state, oauth_state, profile_state};
use self::prompt::SYSTEM_PROMPT;
use self::tools::{build_tools, sandbox};

pub use self::import::import_platform;

/// Every store the harness, tools and routes read, from whichever mode is on.
struct Stores {
    /// Stored knowledge search.
    retriever: Arc<dyn Retriever>,
    /// Live source queries.
    queries: Arc<dyn SourceQueries>,
    /// Conversation history.
    conversations: Arc<dyn ConversationStore>,
    /// Cross-conversation memories.
    memory: Arc<dyn MemoryStore>,
    /// Actions waiting on approval.
    confirmations: Arc<dyn ConfirmationStore>,
    /// The profile graph, when profile recording is on.
    profile_graph: Option<Arc<dyn ProfileGraph>>,
    /// Per-user account grants.
    oauth_store: Arc<dyn OAuthStore>,
    /// What readiness checks besides the model.
    probes: Vec<Arc<dyn Probe>>,
}

/// Serves until shutdown.
pub async fn serve(cfg: Config) -> anyhow::Result<()> {
    let chat_client = rig_openai::client(&cfg.model.base_url, &cfg.model.api_key)
        .map_err(|e| anyhow::anyhow!("model client: {e}"))?;
    let chat = Arc::new(RigChat::new(
        chat_client,
        &cfg.model.name,
        cfg.model.additional_params()?,
    ));
    fits_the_slot(&cfg).await?;
    let model: Arc<dyn ModelProvider> = if cfg.agent.model_slots == 0 {
        chat
    } else {
        Arc::new(Limited::new(
            chat,
            cfg.agent.model_slots,
            Duration::from_secs(cfg.agent.model_queue_wait_secs),
        ))
    };

    let trace: Arc<dyn TraceSink> =
        Arc::new(Fanout::new(trace_sink(&cfg)?).with_style(cfg.agent.progress_style()));

    let embed_client = rig_openai::client(&cfg.embedding.base_url, &cfg.embedding.api_key)
        .map_err(|e| anyhow::anyhow!("embedding client: {e}"))?;
    let embedder: Arc<dyn Embedder> = Arc::new(RigEmbedder::new(
        embed_client,
        &cfg.embedding.name,
        usize::try_from(cfg.embedding.dim)?,
    ));

    // Every configured dependency must be reachable at boot.
    let mut stores = platform::stores(&cfg, &embedder)?;
    store::localize(&cfg, &mut stores).await?;
    let sandbox = sandbox(&cfg).await?;
    let (tools, mcp_names) = build_tools(
        &cfg,
        Arc::clone(&stores.queries),
        Arc::clone(&stores.retriever),
        sandbox.clone(),
        Arc::clone(&stores.oauth_store),
        memory_tools(&cfg, &stores, &trace),
    )
    .await?;
    // Measured at boot with every tool offered, which is the largest the section ever gets.
    let definitions = tools.definitions();
    let capabilities = capability::render(&capability::from_definitions(&definitions, &mcp_names));
    fits_the_prompt(&cfg, &tools, &capabilities)?;
    // The tool names the guardrail keeps out of answers.
    let tool_names: Vec<String> = definitions.iter().map(|d| d.name.clone()).collect();

    let agent_cfg = agent_config(&cfg);

    let profile_graph = stores.profile_graph.clone();
    let deps = AgentDeps {
        profile: profile_writer(&cfg, &model, profile_graph.clone()),
        profile_graph: profile_graph.clone(),
        compactor: compactor(&cfg, &model),
        guardrail: guardrail(&cfg, &tool_names),
        model,
        tools,
        policy: Arc::new(RiskPolicy::from(&cfg.policy)),
        trace,
        conversations: Some(Arc::clone(&stores.conversations)),
        memory: Some(Arc::clone(&stores.memory)),
        confirmations: Some(Arc::clone(&stores.confirmations)),
        sandbox: sandbox.clone().map(|s| s as Arc<dyn Sandbox>),
        files: Some(Arc::new(
            HttpFiles::new(cfg.agent.file_hosts.clone(), cfg.agent.max_file_bytes)
                .map_err(|e| anyhow::anyhow!("attachments: {e}"))?,
        ) as Arc<dyn FileSource>),
    };
    let system_prompt = cfg.system_prompt(SYSTEM_PROMPT)?;
    let agent = Agent::new(deps, agent_cfg, system_prompt)
        .with_prompt_text(PromptText::from(&cfg.prompt))
        .with_mcp_names(mcp_names);

    let state = chat_state(&cfg, agent, stores.conversations, stores.confirmations);
    let profile = profile_state(&cfg, profile_graph, state.rate_limit.clone());
    let memories = memory_state(&cfg, Arc::clone(&stores.memory), state.rate_limit.clone());
    let oauth = oauth_state(&cfg, stores.oauth_store);
    let router = http_router(
        &cfg,
        state,
        profile,
        memories,
        oauth,
        stores.probes,
        sandbox,
    );

    let listener = tokio::net::TcpListener::bind(&cfg.app.http_addr).await?;
    tracing::info!(addr = %cfg.app.http_addr, "listening");
    until_shutdown(
        listener,
        router,
        Duration::from_secs(cfg.http.shutdown_grace_secs),
    )
    .await
}

/// The store and trace the memory tools write to, when memory.tools is on over the SQLite store.
fn memory_tools(
    cfg: &Config,
    stores: &Stores,
    trace: &Arc<dyn TraceSink>,
) -> Option<(Arc<dyn MemoryStore>, Arc<dyn TraceSink>)> {
    if !cfg.memory.tools || cfg.store.backend != StoreBackend::Sqlite {
        return None;
    }
    Some((Arc::clone(&stores.memory), Arc::clone(trace)))
}

/// Serves until the shutdown signal, then gives in-flight requests grace to finish.
async fn until_shutdown(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    grace: Duration,
) -> anyhow::Result<()> {
    let (signalled, wait) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, router).with_graceful_shutdown(async move {
        shutdown().await;
        // The receiver is gone only when the server has already stopped.
        let _ = signalled.send(());
    });
    // The grace period bounds how long in-flight requests may finish.
    tokio::select! {
        result = server => result?,
        () = expire(wait, grace) => {
            tracing::warn!(grace_secs = grace.as_secs(), "grace expired; dropping in-flight requests");
        }
    }
    Ok(())
}

/// Resolves grace after the shutdown signal, and never if it does not arrive.
async fn expire(wait: tokio::sync::oneshot::Receiver<()>, grace: Duration) {
    if wait.await.is_err() {
        std::future::pending::<()>().await;
    }
    tokio::time::sleep(grace).await;
}

/// Resolves on Ctrl-C or SIGTERM.
async fn shutdown() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "could not listen for ctrl-c");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "could not listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}
