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

    /// The error a call to an unknown tool gets back: the names the model meant, or every name offered.
    pub fn unknown(&self, name: &str) -> String {
        let offered: Vec<String> = self.definitions().into_iter().map(|d| d.name).collect();
        let wanted = bare(name);
        let mut near: Vec<&String> = offered
            .iter()
            .filter(|o| {
                let have = bare(o);
                have.contains(&wanted) || wanted.contains(&have) || distance(&have, &wanted) <= 3
            })
            .collect();
        if near.is_empty() {
            near = offered.iter().collect();
        }
        let names: Vec<&str> = near.iter().map(|n| n.as_str()).collect();
        format!(
            "error: no tool named `{name}`. Call one of these by its exact name: {}",
            names.join(", ")
        )
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

/// A tool name in lower case with the platform prefix and separators taken out.
fn bare(name: &str) -> String {
    name.trim_start_matches("platform_")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Edit distance between two short names.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut last = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let next = (row[j + 1] + 1)
                .min(row[j] + 1)
                .min(last + usize::from(ca != *cb));
            last = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

impl std::fmt::Debug for ToolSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.tools.keys()).finish()
    }
}
