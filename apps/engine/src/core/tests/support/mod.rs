//! Test doubles shared across the suite, one file per domain. Agent builders live here.

mod admin;
mod conversation;
mod health;
mod knowledge;
mod memory;
mod model;
mod oauth;
mod safety;
mod tools;
mod trace;

use std::sync::Arc;
use std::time::Duration;

use crate::core::traits::conversation::ConversationStore;
use crate::core::traits::memory::MemoryStore;
use crate::core::traits::safety::confirmation::ConfirmationStore;
use crate::core::types::agent::AgentConfig;
use crate::core::types::agent::context::RequestContext;
use crate::runtime::harness::agent::{Agent, AgentDeps};
use crate::runtime::harness::safety::policy::RiskPolicy;
use crate::runtime::harness::tools::ToolSet;

pub use self::admin::Ledger;
pub use self::conversation::{Recording, Rooms, Row, history_of, push_row};
pub use self::health::Fixed;
pub use self::knowledge::{FakeQueries, FixedEmbedder, Stored};
pub use self::memory::Recalling;
pub use self::model::{Scripted, calls, only_thought, text};
pub use self::oauth::{HostedLogins, Linked};
pub use self::safety::Held;
pub use self::tools::{Boom, Echo, Named, Ordered, Slow};
pub use self::trace::MemorySink;

/// An agent over a scripted model, with an in-memory trace to inspect.
pub fn agent(model: Scripted, tools: ToolSet, cfg: AgentConfig) -> (Agent, Arc<MemorySink>) {
    let sink = Arc::new(MemorySink::default());
    let deps = AgentDeps {
        model: Arc::new(model),
        tools,
        policy: Arc::new(RiskPolicy::default()),
        trace: sink.clone(),
        conversations: None,
        memory: None,
        confirmations: None,
        compactor: None,
        guardrail: None,
        sandbox: None,
        files: None,
    };
    (Agent::new(deps, cfg, "sys"), sink)
}

pub fn agent_with_store(
    model: Scripted,
    tools: ToolSet,
    cfg: AgentConfig,
    conversations: Arc<dyn ConversationStore>,
) -> Agent {
    let deps = AgentDeps {
        model: Arc::new(model),
        tools,
        policy: Arc::new(RiskPolicy::default()),
        trace: Arc::new(MemorySink::default()),
        conversations: Some(conversations),
        memory: None,
        confirmations: None,
        compactor: None,
        guardrail: None,
        sandbox: None,
        files: None,
    };
    Agent::new(deps, cfg, "sys")
}

/// An agent that holds actions and keeps its turns, for the approval path.
pub fn agent_holding(
    model: Scripted,
    tools: ToolSet,
    conversations: Arc<dyn ConversationStore>,
    confirmations: Arc<dyn ConfirmationStore>,
) -> Agent {
    let deps = AgentDeps {
        model: Arc::new(model),
        tools,
        policy: Arc::new(RiskPolicy::default()),
        trace: Arc::new(MemorySink::default()),
        conversations: Some(conversations),
        memory: None,
        confirmations: Some(confirmations),
        compactor: None,
        guardrail: None,
        sandbox: None,
        files: None,
    };
    Agent::new(deps, AgentConfig::default(), "sys")
}

pub fn ctx() -> RequestContext {
    RequestContext::new("g", "u", Duration::from_secs(5))
}

/// An agent that recalls memory.
pub fn agent_recalling(model: Scripted, cfg: AgentConfig, memory: Arc<dyn MemoryStore>) -> Agent {
    let deps = AgentDeps {
        model: Arc::new(model),
        tools: ToolSet::new(),
        policy: Arc::new(RiskPolicy::default()),
        trace: Arc::new(MemorySink::default()),
        conversations: None,
        memory: Some(memory),
        confirmations: None,
        compactor: None,
        guardrail: None,
        sandbox: None,
        files: None,
    };
    Agent::new(deps, cfg, "sys")
}
