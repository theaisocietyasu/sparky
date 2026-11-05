//! MCP servers as tools, each gated by Policy through a RiskClass derived from its name.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, Tool as RemoteTool};
use rmcp::service::{Peer, RoleClient};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use serde_json::{Map, Value};

use crate::core::traits::tools::Tool;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};

/// Limits applied to the tools of one MCP server.
#[derive(Debug, Clone)]
pub struct McpLimits {
    /// Longest tool result handed back to the model.
    pub max_output_chars: usize,
    /// Longest per-property description kept in a schema.
    pub max_schema_description_chars: usize,
    /// Longest tool description kept.
    pub max_tool_description_chars: usize,
    /// Show the model only the required properties of each tool.
    pub required_props_only: bool,
    /// Per-tool timeout for this server, overriding the default of the agent.
    pub tool_timeout_secs: Option<u64>,
}

impl Default for McpLimits {
    fn default() -> Self {
        Self::from(&crate::core::config::Mcp::default())
    }
}

impl From<&crate::core::config::Mcp> for McpLimits {
    /// Server-level limits, before the overrides of a single server.
    fn from(cfg: &crate::core::config::Mcp) -> Self {
        Self {
            max_output_chars: cfg.max_output_chars,
            max_schema_description_chars: cfg.max_schema_description_chars,
            max_tool_description_chars: cfg.max_tool_description_chars,
            required_props_only: cfg.required_props_only,
            tool_timeout_secs: None,
        }
    }
}

/// Keeps only the required properties of an object schema.
pub fn required_only(value: Value) -> Value {
    let Value::Object(mut map) = value else {
        return value;
    };
    let required: Vec<String> = map
        .get("required")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if let Some(Value::Object(props)) = map.get_mut("properties") {
        props.retain(|k, _| required.contains(k));
    }
    Value::Object(map)
}

/// Drops schema noise the model does not need: long descriptions, titles, examples, $schema.
pub fn compact_schema(value: Value, max_description: usize) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(k, _)| {
                    !matches!(k.as_str(), "title" | "examples" | "$schema" | "default")
                })
                .map(|(k, v)| {
                    if k == "description"
                        && let Value::String(s) = &v
                    {
                        return (k, Value::String(s.chars().take(max_description).collect()));
                    }
                    (k, compact_schema(v, max_description))
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|v| compact_schema(v, max_description))
                .collect(),
        ),
        other => other,
    }
}

/// One remote MCP tool, callable through the harness.
pub struct McpTool {
    peer: Peer<RoleClient>,
    definition: ToolDefinition,
    remote: String,
    fill: Fill,
    max_output_chars: usize,
}

/// What a platform tool adds to the arguments of the model, and where it refuses to run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Fill {
    /// Sends confirm=true. Policy confirmed the call with the member before it reached the tool.
    pub confirm: bool,
    /// Sends the caller as discord_id.
    pub member: bool,
    /// Refuses outside a direct message.
    pub private: bool,
}

impl Fill {
    /// The arguments sent to the server: those of the model with confirm and discord_id set.
    pub fn arguments(
        self,
        ctx: &RequestContext,
        args: Value,
    ) -> Result<Option<Map<String, Value>>, ToolError> {
        if self.private && ctx.visibility != Visibility::Private {
            return Err(ToolError::Failed(
                "This reads the member's own account, so it is available only in a direct \
                 message with me, not in a server."
                    .into(),
            ));
        }
        let mut arguments = match args {
            Value::Object(map) => Some(map),
            Value::Null => None,
            other => {
                return Err(ToolError::InvalidArguments(format!(
                    "expected an object, got {other}"
                )));
            }
        };
        if self.confirm {
            arguments
                .get_or_insert_with(Map::new)
                .insert(CONFIRM_ARG.to_owned(), Value::Bool(true));
        }
        if self.member {
            // The caller replaces any member the model named.
            arguments
                .get_or_insert_with(Map::new)
                .insert(MEMBER_ARG.to_owned(), Value::String(ctx.user_id.clone()));
        }
        Ok(arguments)
    }
}

/// Prefix of the model-facing name of every platform tool.
pub const PLATFORM_PREFIX: &str = "platform_";
/// Argument a platform tool that is hard to undo needs before it runs.
const CONFIRM_ARG: &str = "confirm";
/// Argument that names the member whose account a private platform tool reads.
const MEMBER_ARG: &str = "discord_id";
/// Sentence the platform appends to the description of a tool that takes confirm.
const CONFIRM_NOTE: &str = " Runs only with confirm=true";

/// Model-facing name of a platform tool: the prefix, then the remote name with dots as underscores.
pub fn platform_name(remote: &str) -> String {
    format!("{PLATFORM_PREFIX}{}", remote.replace('.', "_"))
}

/// Risk of a platform tool from its MCP annotations: read-only reads, destructive deletes, else a write.
pub fn platform_risk(read_only: Option<bool>, destructive: Option<bool>) -> RiskClass {
    if read_only == Some(true) {
        RiskClass::ReadPublic
    } else if destructive == Some(true) {
        RiskClass::Destructive
    } else {
        RiskClass::ExternalWrite
    }
}

/// Removes the confirm property from a schema. True when it was there.
pub fn take_confirm(schema: &mut Map<String, Value>) -> bool {
    take_property(schema, CONFIRM_ARG)
}

/// Removes the discord_id property from a schema. True when it was there.
pub fn take_member(schema: &mut Map<String, Value>) -> bool {
    take_property(schema, MEMBER_ARG)
}

/// Removes one property from a schema and from its required list. True when it was there.
fn take_property(schema: &mut Map<String, Value>, name: &str) -> bool {
    let removed = schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .is_some_and(|props| props.remove(name).is_some());
    if let Some(Value::Array(required)) = schema.get_mut("required") {
        required.retain(|v| v.as_str() != Some(name));
    }
    removed
}

/// Whether a platform tool returns one member's own data: its remote name starts with a prefix.
pub fn is_private(remote: &str, prefixes: &[String]) -> bool {
    has_prefix(remote, prefixes)
}

/// Whether a remote tool name starts with one of the non-empty prefixes.
fn has_prefix(remote: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|p| !p.is_empty() && remote.starts_with(p.as_str()))
}

/// Risk by name: reads run, interactions are drafts, anything submitting or unknown needs confirm.
pub fn risk_for(name: &str) -> RiskClass {
    /// Look at the page without changing it.
    const READS: [&str; 10] = [
        "navigate",
        "snapshot",
        "screenshot",
        "find",
        "tabs",
        "wait_for",
        "console",
        "network_requests",
        "resize",
        "install",
    ];
    /// Change what the page holds without committing it.
    const DRAFTS: [&str; 5] = ["type", "fill_form", "select_option", "hover", "drag"];
    /// Can commit the page or run code in it. Listed by name.
    const COMMITS: [&str; 6] = [
        "submit",
        "click",
        "press_key",
        "evaluate",
        "file_upload",
        "handle_dialog",
    ];
    if COMMITS.iter().any(|k| name.contains(k)) {
        return RiskClass::ExternalWrite;
    }
    if DRAFTS.iter().any(|k| name.contains(k)) {
        return RiskClass::PrepareWrite;
    }
    if READS.iter().any(|k| name.contains(k)) {
        return RiskClass::ReadPublic;
    }
    // An unrecognised tool is treated as an external write.
    RiskClass::ExternalWrite
}

/// The pinned risk of a tool, or the risk derived from its name when none is pinned.
pub fn pinned_risk(name: &str, risks: &BTreeMap<String, RiskClass>) -> RiskClass {
    risks.get(name).copied().unwrap_or_else(|| risk_for(name))
}

/// Pinned tool names the server does not list, sorted.
pub fn unoffered(risks: &BTreeMap<String, RiskClass>, offered: &[String]) -> Vec<String> {
    risks
        .keys()
        .filter(|name| !offered.contains(name))
        .cloned()
        .collect()
}

/// Opens a Streamable-HTTP session and lists the server's tools.
async fn open(
    config: StreamableHttpClientTransportConfig,
) -> Result<(Peer<RoleClient>, Vec<RemoteTool>), String> {
    let transport = StreamableHttpClientTransport::from_config(config);
    let service = ().serve(transport).await.map_err(|e| e.to_string())?;
    let peer = service.peer().clone();
    tokio::spawn(async move {
        if let Err(e) = service.waiting().await {
            tracing::warn!(error = %e, "mcp connection ended");
        }
    });
    let remote = peer.list_all_tools().await.map_err(|e| e.to_string())?;
    Ok((peer, remote))
}

/// The schema the model sees, trimmed by the limits.
fn model_schema(schema: Map<String, Value>, limits: &McpLimits) -> Value {
    let schema = compact_schema(Value::Object(schema), limits.max_schema_description_chars);
    if limits.required_props_only {
        required_only(schema)
    } else {
        schema
    }
}

/// The tool description cut to the limit.
fn model_description(remote: &RemoteTool, limits: &McpLimits) -> String {
    remote
        .description
        .as_deref()
        .unwrap_or(&remote.name)
        .chars()
        .take(limits.max_tool_description_chars)
        .collect()
}

/// Connects to a Streamable-HTTP MCP server and wraps its tools.
pub async fn connect(
    url: &str,
    allow: &[String],
    risks: &BTreeMap<String, RiskClass>,
    limits: &McpLimits,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    let (peer, remote) = open(StreamableHttpClientTransportConfig::with_uri(url)).await?;
    let offered: Vec<String> = remote.iter().map(|t| t.name.to_string()).collect();
    let missing = unoffered(risks, &offered);
    if !missing.is_empty() {
        return Err(format!(
            "risks pin tools the server does not list: {}",
            missing.join(", ")
        ));
    }
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
    for t in remote {
        let name = t.name.to_string();
        if !allow.is_empty() && !allow.iter().any(|a| a == &name) {
            continue;
        }
        let definition = ToolDefinition {
            risk: pinned_risk(&name, risks),
            description: model_description(&t, limits),
            parameters: model_schema((*t.input_schema).clone(), limits),
            name: name.clone(),
            private: false,
            sequential: true,
            timeout_secs: limits.tool_timeout_secs,
        };
        tools.push(Arc::new(McpTool {
            peer: peer.clone(),
            definition,
            remote: name,
            fill: Fill::default(),
            max_output_chars: limits.max_output_chars,
        }));
    }
    Ok(tools)
}

/// Connects to the platform MCP server with its machine token and wraps the tools the token allows.
///
/// Each tool is named platform_<name> and takes its risk from the server annotations. A tool that
/// takes confirm loses it from the schema; Policy confirms with the member, and the call sends confirm=true.
/// A tool whose name starts with one of private reads one member's data: a read is ReadAuthenticated,
/// it runs only in a direct message, and it loses discord_id from the schema and gets the caller.
/// A tool whose name starts with one of skip is not registered.
pub async fn connect_platform(
    url: &str,
    token: &str,
    allow: &[String],
    private: &[String],
    skip: &[String],
    limits: &McpLimits,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    let config = StreamableHttpClientTransportConfig::with_uri(url).auth_header(token);
    let (peer, remote) = open(config).await?;
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
    for t in remote {
        let name = t.name.to_string();
        if !allow.is_empty() && !allow.iter().any(|a| a == &name) || has_prefix(&name, skip) {
            continue;
        }
        let annotations = t.annotations.as_ref();
        let mut risk = platform_risk(
            annotations.and_then(|a| a.read_only_hint),
            annotations.and_then(|a| a.destructive_hint),
        );
        let mut schema = (*t.input_schema).clone();
        let private = is_private(&name, private);
        if private && risk == RiskClass::ReadPublic {
            risk = RiskClass::ReadAuthenticated;
        }
        let fill = Fill {
            confirm: take_confirm(&mut schema),
            member: private && take_member(&mut schema),
            private,
        };
        let mut description = model_description(&t, limits);
        if let Some(at) = description.find(CONFIRM_NOTE) {
            description.truncate(at);
        }
        let definition = ToolDefinition {
            risk,
            description,
            parameters: model_schema(schema, limits),
            name: platform_name(&name),
            private: false,
            sequential: true,
            timeout_secs: limits.tool_timeout_secs,
        };
        tools.push(Arc::new(McpTool {
            peer: peer.clone(),
            definition,
            remote: name,
            fill,
            max_output_chars: limits.max_output_chars,
        }));
    }
    Ok(tools)
}

#[async_trait]
impl Tool for McpTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let arguments = self.fill.arguments(ctx, args)?;
        let mut params = CallToolRequestParams::new(self.remote.clone());
        params.arguments = arguments;
        let result = self
            .peer
            .call_tool(params)
            .await
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        let mut text = String::new();
        for block in &result.content {
            if let Some(t) = block.as_text() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&t.text);
            }
        }
        if is_html(&text) {
            return Err(ToolError::Failed(
                "the server behind this tool answered with a web page instead of a result; it may \
                 be starting or down. Tell the user it is unavailable right now."
                    .into(),
            ));
        }
        if result.is_error.unwrap_or(false) {
            return Err(ToolError::Failed(if text.is_empty() {
                "mcp tool reported an error".into()
            } else {
                text
            }));
        }
        if text.chars().count() > self.max_output_chars {
            let cut: String = text.chars().take(self.max_output_chars).collect();
            text = format!("{cut}\n…[truncated]");
        }
        Ok(ToolOutput {
            content: text,
            data: result.structured_content,
            sources: Vec::new(),
        })
    }
}

/// True when text is an HTML page, such as a proxy error page, rather than a tool result.
pub(crate) fn is_html(text: &str) -> bool {
    let head: String = text.trim_start().chars().take(15).collect();
    let head = head.to_ascii_lowercase();
    head.starts_with("<!doctype html") || head.starts_with("<html")
}
