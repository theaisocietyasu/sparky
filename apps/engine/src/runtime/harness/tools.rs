//! ToolSet: the tools available to one agent, keyed by name.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use crate::core::traits::tools::Tool;
use crate::core::types::conversation::Visibility;
use crate::core::types::model::tokens::estimate;
use crate::core::types::tools::ToolDefinition;

/// The tools available to one agent, keyed by name. Clones share which tools are switched off.
#[derive(Default, Clone)]
pub struct ToolSet {
    tools: BTreeMap<String, Arc<dyn Tool>>,
    off: Arc<RwLock<BTreeSet<String>>>,
}

impl ToolSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a tool, replacing any with the same name.
    pub fn with(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.insert(tool.definition().name, tool);
        self
    }

    /// Looks a tool up by name. A tool an operator switched off is not found.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        if !self.is_on(name) {
            return None;
        }
        self.tools.get(name)
    }

    /// Every registered tool in name order, switched off or not.
    pub fn all(&self) -> impl Iterator<Item = &Arc<dyn Tool>> {
        self.tools.values()
    }

    /// Whether an operator left the tool on. An unknown name reads as on.
    pub fn is_on(&self, name: &str) -> bool {
        self.off.read().map_or(true, |off| !off.contains(name))
    }

    /// Switches a registered tool on or off for every clone of this set; false for an unknown name.
    pub fn switch(&self, name: &str, on: bool) -> bool {
        if !self.tools.contains_key(name) {
            return false;
        }
        let Ok(mut off) = self.off.write() else {
            return false;
        };
        if on {
            off.remove(name);
        } else {
            off.insert(name.to_owned());
        }
        true
    }

    /// Definitions in name order, for the model. A tool switched off is left out.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .filter(|(name, t)| t.available() && self.is_on(name))
            .map(|(_, t)| t.definition())
            .collect()
    }

    /// Definitions offered in a turn of visibility: a public turn leaves private tools out.
    pub fn definitions_for(&self, visibility: Visibility) -> Vec<ToolDefinition> {
        let mut out = self.definitions();
        if visibility != Visibility::Private {
            out.retain(|d| !d.private);
        }
        out
    }

    /// Estimated tokens the tool schemas add to every model request.
    pub fn estimated_tokens(&self, chars_per_token: usize) -> usize {
        self.definitions()
            .iter()
            .map(|d| {
                let schema = d.parameters.to_string();
                estimate(&d.name, chars_per_token)
                    + estimate(&d.description, chars_per_token)
                    + estimate(&schema, chars_per_token)
            })
            .sum()
    }
}

impl std::fmt::Debug for ToolSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.tools.keys()).finish()
    }
}
