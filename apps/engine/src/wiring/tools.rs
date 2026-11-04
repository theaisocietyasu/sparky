//! The tool set: search, sandbox, MCP servers, and the enabled integrations.

use std::sync::Arc;
use std::time::Duration;

use chrono::FixedOffset;

use secrecy::ExposeSecret;

use crate::core::config::Config;
use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::traits::knowledge::retrieval::Retriever;
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::oauth::OAuthStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::traits::trace::TraceSink;
use crate::core::types::tools::sandbox::Limits as SandboxLimits;
use crate::runtime::harness::tools::ToolSet;
use crate::runtime::tools::account::{canvas, gcal, outlook};
use crate::runtime::tools::knowledge::search;
use crate::runtime::tools::knowledge::search::live::{LiveSearch, Wording as LiveWording};
use crate::runtime::tools::knowledge::search::stored::{StoredSearch, Wording as StoredWording};
use crate::runtime::tools::mcp::{self, McpLimits};
use crate::runtime::tools::memory::{self, MemoryRules, MemoryTasks};
use crate::runtime::tools::sandbox::{
    ContainerSandbox, SandboxTool, Wording as SandboxWording, reap_sessions,
};
use crate::runtime::tools::upcoming::{UpcomingTool, Window as UpcomingWindow};
use crate::runtime::tools::{Built, papers, wiki};

/// Builds the tools and upcoming sources of one integration, or says why it could not.
type Build<'a> = Box<dyn FnOnce() -> Result<Built, String> + 'a>;

/// One integration: its name, whether it is enabled, and how its tools are built.
type Integration<'a> = (&'static str, bool, Build<'a>);

/// Every tool the model may call, with tools.disabled removed at registration.
pub(crate) async fn build_tools(
    cfg: &Config,
    queries: Arc<dyn SourceQueries>,
    retriever: Arc<dyn Retriever>,
    sandbox: Option<Arc<ContainerSandbox>>,
    oauth_store: Arc<dyn OAuthStore>,
    canvas: Option<Arc<dyn CanvasFeeds>>,
    memory: Option<(Arc<dyn MemoryStore>, Arc<dyn TraceSink>)>,
) -> anyhow::Result<(ToolSet, Vec<String>)> {
    let disabled = |name: &str| cfg.tools.disabled.iter().any(|d| d == name);
    let mut tools = ToolSet::new();
    let mut mcp_names = Vec::new();
    if cfg.tools.search {
        for tool in search_tools(cfg, queries, retriever).await? {
            if !disabled(&tool.definition().name) {
                tools = tools.with(tool);
            }
        }
    }
    if let Some(sandbox) = sandbox {
        let tool: Arc<dyn Tool> = Arc::new(SandboxTool::new(
            sandbox,
            cfg.sandbox.risk,
            SandboxWording::from(&cfg.sandbox),
        ));
        if !disabled(&tool.definition().name) {
            tracing::info!(
                runtime = %cfg.sandbox.runtime,
                image = %cfg.sandbox.image,
                "sandbox registered"
            );
            tools = tools.with(tool);
        }
    }
    for server in cfg.mcp.resolved_servers() {
        let limits = McpLimits {
            required_props_only: server
                .required_props_only
                .unwrap_or(cfg.mcp.required_props_only),
            tool_timeout_secs: server.tool_timeout_secs,
            ..McpLimits::from(&cfg.mcp)
        };
        let remote = mcp::connect(&server.url, &server.tools, &server.risks, &limits)
            .await
            .map_err(|e| anyhow::anyhow!("mcp server {} at {}: {e}", server.name, server.url))?;
        let mut registered = 0;
        for tool in remote {
            if disabled(&tool.definition().name) {
                continue;
            }
            mcp_names.push(tool.definition().name);
            tools = tools.with(tool);
            registered += 1;
        }
        tracing::info!(
            server = %server.name,
            url = %server.url,
            count = registered,
            "mcp tools registered"
        );
    }
    if let (Some(url), Some(token)) = (cfg.platform.mcp_endpoint(), cfg.platform.token.as_ref()) {
        let remote = mcp::connect_platform(
            url,
            token.expose_secret(),
            &cfg.platform.mcp_tools,
            &cfg.platform.mcp_private,
            &cfg.platform.mcp_skip,
            &McpLimits::from(&cfg.mcp),
        )
        .await
        .map_err(|e| anyhow::anyhow!("platform mcp at {url}: {e}"))?;
        let mut registered = 0;
        for tool in remote {
            if disabled(&tool.definition().name) {
                continue;
            }
            mcp_names.push(tool.definition().name);
            tools = tools.with(tool);
            registered += 1;
        }
        tracing::info!(url = %url, count = registered, "platform mcp tools registered");
    }
    let mut sources: Vec<Arc<dyn UpcomingSource>> = Vec::new();
    for (integration, enabled, build) in integrations(cfg, &oauth_store, canvas, memory) {
        if !enabled {
            continue;
        }
        let built = build().map_err(|e| anyhow::anyhow!("{integration}: {e}"))?;
        sources.extend(built.upcoming);
        tools = register(tools, built.tools, &disabled, integration);
    }
    sources.retain(|s| !disabled(s.tool()));
    if let Some(tool) = upcoming(cfg, sources)? {
        tools = register(tools, vec![tool], &disabled, "upcoming");
    }
    tracing::info!(tools = ?tools, "tool set");
    Ok((tools, mcp_names))
}

/// The upcoming tool over the sources, when it is on and has at least one source.
fn upcoming(
    cfg: &Config,
    sources: Vec<Arc<dyn UpcomingSource>>,
) -> anyhow::Result<Option<Arc<dyn Tool>>> {
    if !cfg.upcoming.enabled || sources.is_empty() {
        return Ok(None);
    }
    let offset = FixedOffset::east_opt(cfg.prompt.utc_offset_hours * 3600)
        .ok_or_else(|| anyhow::anyhow!("prompt.utc_offset_hours is out of range"))?;
    let window = UpcomingWindow {
        default_days: cfg.upcoming.default_days,
        max_days: cfg.upcoming.max_days,
        max_items: cfg.upcoming.max_items,
    };
    let labels: Vec<&str> = sources.iter().map(|s| s.label()).collect();
    tracing::info!(sources = %labels.join(", "), "upcoming sources");
    Ok(Some(Arc::new(UpcomingTool::new(sources, window, offset))))
}

/// Every integration that adds tools, in registration order.
fn integrations<'a>(
    cfg: &'a Config,
    oauth_store: &'a Arc<dyn OAuthStore>,
    canvas_feeds: Option<Arc<dyn CanvasFeeds>>,
    memory: Option<(Arc<dyn MemoryStore>, Arc<dyn TraceSink>)>,
) -> Vec<Integration<'a>> {
    let mut all: Vec<Integration<'a>> = Vec::new();
    all.extend::<[Integration<'a>; 6]>([
        (
            "memory",
            memory.is_some(),
            Box::new(move || {
                let Some((store, trace)) = memory else {
                    return Ok(Built::default());
                };
                let rules = MemoryRules::new(&cfg.memory, cfg.prompt.utc_offset_hours);
                Ok(Built {
                    tools: memory::tools(&store, &trace, rules),
                    upcoming: vec![Arc::new(MemoryTasks::new(store))],
                })
            }),
        ),
        (
            "outlook",
            cfg.outlook.enabled,
            Box::new(move || {
                outlook::tools(&cfg.outlook, oauth_store.clone()).map_err(|e| e.to_string())
            }),
        ),
        (
            "gcal",
            cfg.gcal.enabled,
            Box::new(move || {
                gcal::tools(&cfg.gcal, oauth_store.clone(), cfg.prompt.utc_offset_hours)
                    .map_err(|e| e.to_string())
            }),
        ),
        (
            "canvas",
            cfg.canvas.enabled && canvas_feeds.is_some(),
            Box::new(move || match canvas_feeds {
                Some(feeds) => canvas::tools(&cfg.canvas, feeds, cfg.prompt.utc_offset_hours),
                None => Ok(Built::default()),
            }),
        ),
        (
            "papers",
            cfg.papers.enabled,
            Box::new(move || papers::tools(&cfg.papers).map(Built::from)),
        ),
        (
            "wikipedia",
            cfg.wikipedia.enabled,
            Box::new(move || wiki::tools(&cfg.wikipedia).map(Built::from)),
        ),
    ]);
    all
}

/// Adds each built tool that is not disabled to the set, logging how many an integration added.
fn register(
    mut tools: ToolSet,
    built: Vec<Arc<dyn Tool>>,
    disabled: &dyn Fn(&str) -> bool,
    integration: &str,
) -> ToolSet {
    let mut count = 0;
    for tool in built {
        if disabled(&tool.definition().name) {
            continue;
        }
        tools = tools.with(tool);
        count += 1;
    }
    tracing::info!(integration, count, "tools registered");
    tools
}

/// The stored and live search tools, after checking the catalog against the published registry.
async fn search_tools(
    cfg: &Config,
    queries: Arc<dyn SourceQueries>,
    retriever: Arc<dyn Retriever>,
) -> anyhow::Result<Vec<Arc<dyn Tool>>> {
    let sources = search::catalog();
    let fallback = cfg.tools.live_default_source.trim();
    if !sources.iter().any(|s| s.key() == fallback) {
        anyhow::bail!(
            "tools.live_default_source is {fallback:?}, which is not a source the engine offers: {}",
            search::source_keys(&sources, false).join(", ")
        );
    }
    // A mismatch with the published registry is logged, not fatal.
    let published = queries.sources().await?;
    let mut unpublished = Vec::new();
    for source in &sources {
        let Some(served) = published.iter().find(|p| p.key == source.key()) else {
            unpublished.push(source.key());
            continue;
        };
        if let Err(difference) = search::conforms(source.as_ref(), served) {
            tracing::warn!(
                source = source.key(),
                %difference,
                "the tool and the platform registry disagree"
            );
        }
    }
    // One line for every unpublished source.
    if !unpublished.is_empty() {
        tracing::info!(
            count = unpublished.len(),
            sources = %unpublished.join(", "),
            "the platform publishes no live query for these sources; search_live does not offer them"
        );
    }
    let stored: Arc<dyn Tool> = Arc::new(StoredSearch::new(
        &sources,
        retriever,
        cfg.retrieval.top_k,
        &StoredWording {
            tool: cfg.tools.knowledge_description.clone(),
            query: cfg.tools.query_description.clone(),
            source: cfg.tools.source_description.clone(),
            empty: cfg.tools.nothing_stored.clone(),
        },
    ));
    // search_live offers only the sources the platform publishes, and the fallback.
    let live_sources = search::published_only(sources, &published, fallback);
    let live: Arc<dyn Tool> = Arc::new(LiveSearch::new(
        live_sources,
        queries,
        fallback,
        cfg.prompt.utc_offset_hours,
        &LiveWording {
            tool: cfg.tools.live_description.clone(),
            query: cfg.tools.query_description.clone(),
            source: cfg.tools.source_description.clone(),
        },
        cfg.query.timeout_secs,
    ));
    tracing::info!(fallback, "search tools registered");
    Ok(vec![stored, live])
}

/// The sandbox, when it is enabled and its runtime answers. Starts the session sweeper.
pub(super) async fn sandbox(cfg: &Config) -> anyhow::Result<Option<Arc<ContainerSandbox>>> {
    if !cfg.sandbox.enabled {
        return Ok(None);
    }
    let sandbox = ContainerSandbox::new(SandboxLimits::from(&cfg.sandbox));
    if let Err(error) = sandbox.probe().await {
        if cfg.sandbox.required {
            anyhow::bail!(
                "sandbox.enabled is on but the container runtime is unreachable: {error}. \
                 Give the engine a runtime, or set sandbox.required = false to run without it."
            );
        }
        tracing::warn!(%error, "the container runtime is unreachable; run_sandbox is not offered");
        return Ok(None);
    }
    // The sweep interval is the idle budget: a session lives at most twice it.
    let every = Duration::from_secs(cfg.sandbox.session_idle_secs.max(1));
    // Session containers a previous engine left running are nobody's to reap but this one's.
    sandbox.remove_orphans().await;
    tokio::spawn(reap_sessions(sandbox.clone(), every));
    Ok(Some(Arc::new(sandbox)))
}
