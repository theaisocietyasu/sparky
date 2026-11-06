//! The tool set: search, sandbox, MCP servers, and the enabled integrations.

use std::sync::Arc;
use std::time::Duration;

use chrono::FixedOffset;

use secrecy::ExposeSecret;

use crate::core::config::{Config, WASM_RUNTIME};
use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::knowledge::discovery::Discovery;
use crate::core::traits::knowledge::query::SourceQueries;
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::nudge::NudgeStore;
use crate::core::traits::oauth::OAuthStore;
use crate::core::traits::reminder::ReminderStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::core::traits::tools::sandbox::Sandbox;
use crate::core::traits::tools::upcoming::UpcomingSource;
use crate::core::traits::trace::TraceSink;
use crate::core::types::nudge::NudgePrefs;
use crate::core::types::tools::sandbox::Limits as SandboxLimits;
use crate::runtime::harness::tools::ToolSet;
use crate::runtime::tools::account::{canvas, gcal};
use crate::runtime::tools::knowledge::search::Search;
use crate::runtime::tools::mcp::{self, McpLimits};
use crate::runtime::tools::memory::{self, MemoryRules, MemoryTasks};
use crate::runtime::tools::remind::{self, RemindRules};
use crate::runtime::tools::sandbox::{
    ContainerSandbox, SandboxTool, WasmLimits, WasmSandbox, Wording as SandboxWording,
    reap_sessions,
};
use crate::runtime::tools::settings::{self, Accounts, SettingsStores};
use crate::runtime::tools::upcoming::{UpcomingTool, Window as UpcomingWindow};
use crate::runtime::tools::{Built, calculate, chats, page, papers, wiki};

/// Builds the tools and upcoming sources of one integration, or says why it could not.
type Build<'a> = Box<dyn FnOnce() -> Result<Built, String> + 'a>;

/// One integration: its name, whether it is enabled, and how its tools are built.
type Integration<'a> = (&'static str, bool, Build<'a>);

/// The per-student stores the account tools read: Canvas links, reminders, and memories with their trace.
#[derive(Default)]
pub(crate) struct Personal {
    /// Canvas calendar feed links.
    pub canvas: Option<Arc<dyn CanvasFeeds>>,
    /// Reminders and notifications.
    pub reminders: Option<Arc<dyn ReminderStore>>,
    /// Memories and the trace their writes report on.
    pub memory: Option<(Arc<dyn MemoryStore>, Arc<dyn TraceSink>)>,
    /// Conversations in the local file, for the chat search.
    pub chats: Option<Arc<dyn ConversationStore>>,
    /// Where read_page suggests the pages it reads.
    pub pages: Option<Arc<dyn Discovery>>,
    /// Nudge settings and their defaults, for the settings tool.
    pub nudges: Option<(Arc<dyn NudgeStore>, NudgePrefs)>,
}

/// Every tool the model may call, with tools.disabled removed at registration.
pub(crate) async fn build_tools(
    cfg: &Config,
    queries: Arc<dyn SourceQueries>,
    sandbox: Option<Arc<dyn Sandbox>>,
    oauth_store: Arc<dyn OAuthStore>,
    personal: Personal,
) -> anyhow::Result<(ToolSet, Vec<String>, Vec<Arc<dyn UpcomingSource>>)> {
    let disabled = |name: &str| cfg.tools.disabled.iter().any(|d| d == name);
    let mut tools = ToolSet::new();
    let mut mcp_names = Vec::new();
    if cfg.tools.search {
        let tool = search_tool(cfg, queries).await?;
        if !disabled(&tool.definition().name) {
            tools = tools.with(tool);
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
    let mut personal = personal;
    if cfg.settings.enabled {
        let stores = settings_stores(
            cfg,
            &oauth_store,
            personal.canvas.as_ref(),
            personal.nudges.take(),
        );
        tools = register(tools, settings::tools(stores), &disabled, "settings");
    }
    let mut sources: Vec<Arc<dyn UpcomingSource>> = Vec::new();
    for (integration, enabled, build) in integrations(cfg, &oauth_store, personal) {
        if !enabled {
            continue;
        }
        let built = build().map_err(|e| anyhow::anyhow!("{integration}: {e}"))?;
        sources.extend(built.upcoming);
        tools = register(tools, built.tools, &disabled, integration);
    }
    sources.retain(|s| !disabled(s.tool()));
    if let Some(tool) = upcoming(cfg, sources.clone())? {
        tools = register(tools, vec![tool], &disabled, "upcoming");
    }
    tracing::info!(tools = ?tools, "tool set");
    Ok((tools, mcp_names, sources))
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
    personal: Personal,
) -> Vec<Integration<'a>> {
    let Personal {
        canvas: canvas_feeds,
        reminders,
        memory,
        chats,
        pages,
        nudges: _,
    } = personal;
    let mut all: Vec<Integration<'a>> = Vec::new();
    all.extend::<[Integration<'a>; 9]>([
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
            "reminders",
            cfg.reminders.enabled && reminders.is_some(),
            Box::new(move || {
                let Some(store) = reminders else {
                    return Ok(Built::default());
                };
                let offset = FixedOffset::east_opt(cfg.prompt.utc_offset_hours * 3600)
                    .ok_or("prompt.utc_offset_hours is out of range")?;
                let rules = RemindRules {
                    max_chars: cfg.reminders.max_chars,
                    max_days_ahead: cfg.reminders.max_days_ahead,
                    offset,
                };
                Ok(remind::tools(&store, rules))
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
            "search_chats",
            cfg.chat_search.enabled && chats.is_some(),
            Box::new(move || {
                Ok(chats
                    .map(|store| Built::from(chats::tools(store, cfg.chat_search.max_hits)))
                    .unwrap_or_default())
            }),
        ),
        (
            "calculate",
            cfg.calculate.enabled,
            Box::new(move || calculate::tools(cfg.prompt.utc_offset_hours).map(Built::from)),
        ),
        (
            "read_page",
            cfg.read_page.enabled,
            Box::new(move || Ok(Built::from(page::tools(&cfg.read_page, pages)))),
        ),
        (
            "wikipedia",
            cfg.wikipedia.enabled,
            Box::new(move || wiki::tools(&cfg.wikipedia).map(Built::from)),
        ),
    ]);
    all
}

/// What the settings tools read and change: check-ins, the Canvas link, and accounts with a sign-in.
fn settings_stores(
    cfg: &Config,
    oauth_store: &Arc<dyn OAuthStore>,
    canvas: Option<&Arc<dyn CanvasFeeds>>,
    nudges: Option<(Arc<dyn NudgeStore>, NudgePrefs)>,
) -> SettingsStores {
    let names = [("google", "google", cfg.gcal.enabled)]
        .into_iter()
        .filter(|(_, _, on)| *on)
        .map(|(name, provider, _)| (name.to_owned(), provider.to_owned()))
        .collect();
    SettingsStores {
        nudge_defaults: nudges.as_ref().map(|(_, defaults)| defaults.clone()),
        nudges: nudges.map(|(store, _)| store),
        canvas: canvas.filter(|_| cfg.canvas.enabled).cloned(),
        accounts: Some(Accounts {
            store: Arc::clone(oauth_store),
            names,
        }),
    }
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

/// The search tool, as Platform describes it.
async fn search_tool(
    cfg: &Config,
    queries: Arc<dyn SourceQueries>,
) -> anyhow::Result<Arc<dyn Tool>> {
    let spec = queries
        .spec()
        .await
        .map_err(|e| anyhow::anyhow!("search tool: {e}"))?;
    tracing::info!("search tool registered");
    Ok(Arc::new(Search::new(queries, spec, cfg.query.timeout_secs)))
}

/// The sandbox, when it is enabled and its runtime answers. Starts the session sweeper.
pub(super) async fn sandbox(cfg: &Config) -> anyhow::Result<Option<Arc<dyn Sandbox>>> {
    if !cfg.sandbox.enabled {
        return Ok(None);
    }
    if cfg.sandbox.runtime == WASM_RUNTIME {
        return wasm_sandbox(cfg).await;
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

/// The wasm sandbox, when its packages are in place. The child it runs is this engine binary.
async fn wasm_sandbox(cfg: &Config) -> anyhow::Result<Option<Arc<dyn Sandbox>>> {
    let program = std::env::current_exe()?;
    let sandbox = WasmSandbox::new(WasmLimits::from(&cfg.sandbox), program);
    if let Err(error) = sandbox.probe().await {
        if cfg.sandbox.required {
            anyhow::bail!(
                "sandbox.runtime is wasm but its packages are missing: {error}. \
                 Put them in sandbox.wasm_packages, or set sandbox.required = false."
            );
        }
        tracing::warn!(%error, "the wasm packages are missing; run_sandbox is not offered");
        return Ok(None);
    }
    Ok(Some(Arc::new(sandbox)))
}
