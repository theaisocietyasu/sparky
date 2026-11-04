//! The configurable RiskPolicy and payload hashing for confirmations.

use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::core::traits::safety::policy::Policy;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::conversation::Visibility;
use crate::core::types::safety::policy::{ConfirmationRequest, Decision, ProposedAction};
use crate::core::types::tools::RiskClass;

/// Stable hash of the canonical JSON of arguments. A changed payload needs a new confirmation.
pub fn payload_hash(arguments: &Value) -> String {
    use std::hash::{Hash, Hasher};
    // Without preserve_order, serde_json serializes map keys in sorted order.
    let canonical = arguments.to_string();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// What the policy allows, denies, and holds. Comes from the policy configuration section.
#[derive(Debug, Clone)]
pub struct RiskPolicy {
    write_roles: Vec<String>,
    allow_authenticated_reads: bool,
    confirm_from: RiskClass,
}

impl Default for RiskPolicy {
    fn default() -> Self {
        Self::from(&crate::core::config::Policy::default())
    }
}

impl From<&crate::core::config::Policy> for RiskPolicy {
    fn from(cfg: &crate::core::config::Policy) -> Self {
        Self::new(
            cfg.write_roles.clone(),
            cfg.allow_authenticated_reads,
            cfg.confirm_from,
        )
    }
}

impl RiskPolicy {
    /// Builds the policy over its role and risk gates.
    pub fn new(
        write_roles: Vec<String>,
        allow_authenticated_reads: bool,
        confirm_from: RiskClass,
    ) -> Self {
        Self {
            write_roles,
            allow_authenticated_reads,
            confirm_from,
        }
    }

    /// Whether the roles of the request include one that may run write-class tools.
    fn may_write(&self, ctx: &RequestContext) -> bool {
        self.write_roles.iter().any(|role| ctx.has_role(role))
    }

    /// How the confirmation describes what is about to happen.
    fn summary(action: &ProposedAction) -> String {
        format!(
            "Run `{}` with {}. {}",
            action.tool,
            action.arguments,
            if action.risk == RiskClass::Destructive {
                "This cannot be undone."
            } else {
                "This posts or submits externally."
            }
        )
    }
}

#[async_trait]
impl Policy for RiskPolicy {
    async fn authorize(&self, ctx: &RequestContext, action: &ProposedAction) -> Decision {
        if action.risk == RiskClass::Forbidden {
            return Decision::Deny {
                reason: format!("`{}` is forbidden", action.tool),
            };
        }
        // A private tool runs only in a private turn, whatever the other gates allow.
        if action.private && ctx.visibility != Visibility::Private {
            return Decision::Deny {
                reason: format!("`{}` runs only in a direct message", action.tool),
            };
        }
        if action.risk == RiskClass::ReadAuthenticated
            && !self.allow_authenticated_reads
            && ctx.visibility != Visibility::Private
        {
            return Decision::Deny {
                reason: "authenticated reads run only in a direct message".into(),
            };
        }
        if action.risk >= RiskClass::ExternalWrite && !self.may_write(ctx) {
            return Decision::Deny {
                reason: if self.write_roles.is_empty() {
                    format!("`{}` is disabled: no role may run write tools", action.tool)
                } else {
                    format!(
                        "`{}` requires one of these roles: {}",
                        action.tool,
                        self.write_roles.join(", ")
                    )
                },
            };
        }
        if action.risk >= self.confirm_from {
            return Decision::Confirm(ConfirmationRequest {
                token: Uuid::new_v4(),
                tool: action.tool.clone(),
                payload_hash: payload_hash(&action.arguments),
                summary: Self::summary(action),
            });
        }
        Decision::Allow
    }
}
