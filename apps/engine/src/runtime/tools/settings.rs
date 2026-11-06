//! settings and disconnect: the student changes their own Sparky settings by asking in chat.

use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::core::traits::nudge::NudgeStore;
use crate::core::traits::oauth::OAuthStore;
use crate::core::traits::tools::Tool;
use crate::core::traits::tools::canvas::CanvasFeeds;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::nudge::{NudgeKind, NudgePrefs, Person};
use crate::core::types::tools::oauth::USER_SCOPE;
use crate::core::types::tools::{RiskClass, ToolDefinition, ToolError, ToolOutput};

/// Name of the settings tool.
pub const SETTINGS: &str = "settings";
/// Name of the tool that unlinks an account.
pub const DISCONNECT: &str = "disconnect";

/// What the settings tools read and write. Each part is None when that feature is off.
#[derive(Clone, Default)]
pub struct SettingsStores {
    /// Nudge quiet time and kinds.
    pub nudges: Option<Arc<dyn NudgeStore>>,
    /// Nudge settings of a student who has set none.
    pub nudge_defaults: Option<NudgePrefs>,
    /// Canvas calendar feed links.
    pub canvas: Option<Arc<dyn CanvasFeeds>>,
    /// Linked accounts whose tool is on.
    pub accounts: Option<Accounts>,
}

/// The accounts a student can link through a sign-in, and where their grants are kept.
#[derive(Clone)]
pub struct Accounts {
    /// Where grants are kept and sign-ins start.
    pub store: Arc<dyn OAuthStore>,
    /// Each account as the model names it, with its provider.
    pub names: Vec<(String, String)>,
}

/// The arguments of settings.
#[derive(Debug, Deserialize)]
struct SettingsArgs {
    /// What to do.
    action: String,
    /// Local hour quiet time starts.
    #[serde(default)]
    quiet_start: Option<u8>,
    /// Local hour quiet time ends.
    #[serde(default)]
    quiet_end: Option<u8>,
    /// Nudge kinds to switch off or on.
    #[serde(default)]
    kinds: Vec<String>,
    /// Account to connect.
    #[serde(default)]
    account: Option<String>,
}

/// Reads and changes the caller's settings.
pub struct SettingsTool {
    stores: SettingsStores,
}

/// Unlinks one of the caller's accounts.
pub struct DisconnectTool {
    stores: SettingsStores,
}

fn person(ctx: &RequestContext) -> Person {
    Person {
        tenant: ctx.tenant_id.clone(),
        user: ctx.user_id.clone(),
    }
}

fn text(content: String) -> ToolOutput {
    ToolOutput {
        content,
        data: None,
        sources: Vec::new(),
    }
}

fn failed(error: impl std::fmt::Display, what: &str) -> ToolError {
    tracing::error!(%error, what, "a settings store failed");
    ToolError::Failed(format!(
        "could not {what} right now; say so and offer to try again"
    ))
}

fn kind_names() -> Vec<&'static str> {
    NudgeKind::ALL.iter().map(|k| k.as_str()).collect()
}

impl SettingsStores {
    /// The accounts a student can link, as named to the model.
    fn account_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if self.canvas.is_some() {
            names.push("canvas".to_owned());
        }
        if let Some(accounts) = &self.accounts {
            names.extend(accounts.names.iter().map(|(name, _)| name.clone()));
        }
        names
    }

    fn provider(&self, account: &str) -> Option<(&Arc<dyn OAuthStore>, &str)> {
        let accounts = self.accounts.as_ref()?;
        accounts
            .names
            .iter()
            .find(|(name, _)| name == account)
            .map(|(_, provider)| (&accounts.store, provider.as_str()))
    }

    async fn prefs(&self, who: &Person) -> Result<Option<NudgePrefs>, ToolError> {
        let Some(store) = &self.nudges else {
            return Ok(None);
        };
        let prefs = store
            .prefs(who)
            .await
            .map_err(|e| failed(e, "read nudge settings"))?;
        Ok(prefs.or_else(|| self.nudge_defaults.clone()))
    }

    /// Every setting of the caller, one per line.
    async fn summary(&self, ctx: &RequestContext) -> Result<String, ToolError> {
        let who = person(ctx);
        let mut out = String::new();
        if let Some(prefs) = self.prefs(&who).await? {
            let quiet = if prefs.quiet_start == prefs.quiet_end {
                "no quiet time".to_owned()
            } else {
                format!(
                    "quiet from {}:00 to {}:00",
                    prefs.quiet_start, prefs.quiet_end
                )
            };
            let off: Vec<&str> = prefs.off.iter().map(|k| k.as_str()).collect();
            let _ = writeln!(
                out,
                "Check-ins: {quiet}; switched off: {}.",
                if off.is_empty() {
                    "none".to_owned()
                } else {
                    off.join(", ")
                }
            );
        }
        if let Some(feeds) = &self.canvas {
            let linked = feeds
                .load(&ctx.user_id)
                .await
                .map_err(|e| failed(e, "read the Canvas link"))?
                .is_some();
            let _ = writeln!(
                out,
                "Canvas calendar: {}.",
                if linked {
                    "linked"
                } else {
                    "not linked; the student pastes their Canvas Calendar Feed link in this chat and it is saved"
                }
            );
        }
        if let Some(accounts) = &self.accounts {
            for (name, provider) in &accounts.names {
                let grant = accounts
                    .store
                    .load_grant(USER_SCOPE, &ctx.user_id, provider)
                    .await
                    .map_err(|e| failed(e, "read linked accounts"))?;
                let state = if grant.is_some() {
                    "linked"
                } else {
                    "not linked"
                };
                let _ = writeln!(out, "{name}: {state}.");
            }
        }
        if out.is_empty() {
            out.push_str("This student has no settings to show.");
        }
        Ok(out.trim_end().to_owned())
    }

    async fn set_quiet(
        &self,
        ctx: &RequestContext,
        start: Option<u8>,
        end: Option<u8>,
    ) -> Result<String, ToolError> {
        let (Some(start), Some(end)) = (start, end) else {
            return Err(ToolError::InvalidArguments(
                "quiet_hours needs quiet_start and quiet_end, local hours 0 to 23; equal hours mean no quiet time".into(),
            ));
        };
        if start > 23 || end > 23 {
            return Err(ToolError::InvalidArguments("hours are 0 to 23".into()));
        }
        self.change(ctx, |prefs| {
            prefs.quiet_start = start;
            prefs.quiet_end = end;
        })
        .await
    }

    async fn switch(
        &self,
        ctx: &RequestContext,
        kinds: &[String],
        on: bool,
    ) -> Result<String, ToolError> {
        let mut parsed = Vec::new();
        for kind in kinds {
            let name = kind.trim().to_ascii_lowercase().replace([' ', '-'], "_");
            if name == "all" {
                parsed.extend(NudgeKind::ALL);
                continue;
            }
            let found = NudgeKind::parse(&name).ok_or_else(|| {
                ToolError::InvalidArguments(format!(
                    "{kind} is not a check-in kind; use all or some of: {}",
                    kind_names().join(", ")
                ))
            })?;
            parsed.push(found);
        }
        if parsed.is_empty() {
            return Err(ToolError::InvalidArguments(format!(
                "kinds is empty; use all or some of: {}",
                kind_names().join(", ")
            )));
        }
        self.change(ctx, |prefs| {
            prefs.off.retain(|k| !parsed.contains(k));
            if !on {
                prefs.off.extend(parsed.iter().copied());
            }
        })
        .await
    }

    async fn change(
        &self,
        ctx: &RequestContext,
        edit: impl FnOnce(&mut NudgePrefs),
    ) -> Result<String, ToolError> {
        let Some(store) = &self.nudges else {
            return Err(ToolError::Failed("check-ins are not on for Sparky".into()));
        };
        let who = person(ctx);
        let mut prefs = self.prefs(&who).await?.unwrap_or(NudgePrefs {
            quiet_start: 0,
            quiet_end: 0,
            off: Vec::new(),
        });
        edit(&mut prefs);
        store
            .set_prefs(&who, &prefs)
            .await
            .map_err(|e| failed(e, "keep nudge settings"))?;
        Ok(format!("Saved. {}", self.summary(ctx).await?))
    }

    async fn connect(
        &self,
        ctx: &RequestContext,
        account: Option<&str>,
    ) -> Result<String, ToolError> {
        let account = account.unwrap_or_default().trim().to_ascii_lowercase();
        if account == "canvas" && self.canvas.is_some() {
            return Ok("Canvas links by its calendar feed. Ask the student to open Canvas, go to \
                       Calendar, choose Calendar Feed at the bottom right, copy the link, and paste \
                       it here. It is saved the moment they send it, and you never see it."
                .to_owned());
        }
        let Some((store, provider)) = self.provider(&account) else {
            return Err(ToolError::InvalidArguments(format!(
                "account is one of: {}",
                self.account_names().join(", ")
            )));
        };
        match store.login_link(&ctx.user_id, provider).await {
            Ok(Some(url)) => Ok(format!(
                "Give the student this link to sign in and allow access: {url} \
                 Once they finish, their {account} account is linked."
            )),
            Ok(None) => Err(ToolError::Failed(format!(
                "{account} sign-in is not set up for Sparky"
            ))),
            Err(error) => Err(failed(error, "start the sign-in")),
        }
    }
}

#[async_trait]
impl Tool for SettingsTool {
    fn definition(&self) -> ToolDefinition {
        let accounts = self.stores.account_names();
        let mut kinds = kind_names();
        kinds.push("all");
        ToolDefinition {
            name: SETTINGS.to_owned(),
            description: format!(
                "Show or change the student's own Sparky settings, so they never have to open a \
                 settings page. Actions: show; quiet_hours with quiet_start and quiet_end, local \
                 hours 0 to 23, when check-ins stay silent; nudges_off or nudges_on with kinds; \
                 connect with account ({}), which gives the steps or sign-in link to hand them. \
                 Examples: no check-ins after 10pm is quiet_hours 22 to 8; stop the morning \
                 brief is nudges_off with kinds brief.",
                accounts.join(", ")
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["show", "quiet_hours", "nudges_off", "nudges_on", "connect"]},
                    "quiet_start": {"type": "integer", "minimum": 0, "maximum": 23},
                    "quiet_end": {"type": "integer", "minimum": 0, "maximum": 23},
                    "kinds": {"type": "array", "items": {"type": "string", "enum": kinds}},
                    "account": {"type": "string", "enum": accounts}
                },
                "required": ["action"]
            }),
            risk: RiskClass::PrepareWrite,
            private: true,
            sequential: true,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let args: SettingsArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArguments(format!("settings arguments: {e}")))?;
        let s = &self.stores;
        let content = match args.action.as_str() {
            "show" => s.summary(ctx).await?,
            "quiet_hours" => s.set_quiet(ctx, args.quiet_start, args.quiet_end).await?,
            "nudges_off" => s.switch(ctx, &args.kinds, false).await?,
            "nudges_on" => s.switch(ctx, &args.kinds, true).await?,
            "connect" => s.connect(ctx, args.account.as_deref()).await?,
            other => {
                return Err(ToolError::InvalidArguments(format!(
                    "{other} is not an action; use show, quiet_hours, nudges_off, nudges_on or connect"
                )));
            }
        };
        Ok(text(content))
    }
}

#[async_trait]
impl Tool for DisconnectTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: DISCONNECT.to_owned(),
            description:
                "Unlink one of the student's accounts when they ask. Sparky stops reading \
                          it until they link it again. The student approves it with a button."
                    .to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {"account": {"type": "string", "enum": self.stores.account_names()}},
                "required": ["account"]
            }),
            risk: RiskClass::Destructive,
            private: true,
            sequential: true,
            timeout_secs: None,
        }
    }

    async fn call(&self, ctx: &RequestContext, args: Value) -> Result<ToolOutput, ToolError> {
        let account = args
            .get("account")
            .and_then(Value::as_str)
            .map(|a| a.trim().to_ascii_lowercase())
            .unwrap_or_default();
        let s = &self.stores;
        let removed = if account == "canvas" {
            let Some(feeds) = &s.canvas else {
                return Err(ToolError::InvalidArguments("Canvas is not on".into()));
            };
            feeds
                .delete(&ctx.user_id)
                .await
                .map_err(|e| failed(e, "unlink Canvas"))?
        } else {
            let Some((store, provider)) = s.provider(&account) else {
                return Err(ToolError::InvalidArguments(format!(
                    "account is one of: {}",
                    s.account_names().join(", ")
                )));
            };
            store
                .delete_grant(USER_SCOPE, &ctx.user_id, provider)
                .await
                .map_err(|e| failed(e, "unlink the account"))?
        };
        Ok(text(if removed {
            format!("Unlinked {account}.")
        } else {
            format!("{account} was not linked.")
        }))
    }
}

/// The settings tools over stores, or none when no setting can be changed.
pub fn tools(stores: SettingsStores) -> Vec<Arc<dyn Tool>> {
    if stores.nudges.is_none() && stores.account_names().is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Arc<dyn Tool>> = vec![Arc::new(SettingsTool {
        stores: stores.clone(),
    })];
    if !stores.account_names().is_empty() {
        out.push(Arc::new(DisconnectTool { stores }));
    }
    out
}
