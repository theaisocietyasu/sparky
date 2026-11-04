//! The profile graph types and /profile wire shapes; a fact is a node and edge per entity.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Full confidence, the value an extraction that says nothing about it carries.
fn certain() -> f32 {
    1.0
}

/// One entity named in a fact, before it is matched to a stored node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEntity {
    /// What sort of thing it is: person, course, club, place, topic.
    pub kind: String,
    /// How it is named, as the text the node is embedded from.
    pub label: String,
}

/// One stored entity in a user's graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileNode {
    /// Row id.
    pub id: Uuid,
    /// What sort of thing it is.
    pub kind: String,
    /// How it is named.
    pub label: String,
    /// Confidence at write time, 0 to 1.
    pub confidence: f32,
    /// When it first entered the graph.
    pub created_at: DateTime<Utc>,
    /// When extraction last confirmed it.
    pub updated_at: DateTime<Utc>,
}

/// What extraction produces from one turn, before it is stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileFact {
    /// Entity the relation starts at.
    pub subject: ProfileEntity,
    /// What the relation is.
    pub relation: String,
    /// Entity the relation points to.
    pub object: ProfileEntity,
    /// How sure extraction is, 0 to 1.
    #[serde(default = "certain")]
    pub confidence: f32,
}

/// Why a profile graph read, write, or extraction did not happen.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    /// The database rejected or could not run the operation.
    #[error("profile graph store: {0}")]
    Store(String),
    /// The model answered with something the extraction shape does not fit.
    #[error("profile extraction malformed: {0}")]
    Malformed(String),
    /// The model call itself failed.
    #[error(transparent)]
    Model(#[from] crate::core::types::model::ModelError),
}

impl From<&ProfileNode> for crate::core::types::memory::Memory {
    /// A recalled node reaches the prompt through the memory section.
    fn from(node: &ProfileNode) -> Self {
        Self {
            id: node.id,
            kind: crate::core::types::memory::MemoryKind::Fact,
            content: format!("{}: {}", node.kind, node.label),
            sensitive: false,
            created_at: node.created_at,
            expires_at: None,
        }
    }
}

/// One recalled relation, with both ends resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileRelation {
    /// Where the relation starts.
    pub subject: ProfileEntity,
    /// What it asserts.
    pub relation: String,
    /// Where it ends.
    pub object: ProfileEntity,
    /// How sure the extraction was.
    pub confidence: f32,
}

impl std::fmt::Display for ProfileRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.subject.label, self.relation, self.object.label
        )
    }
}

impl From<&ProfileRelation> for crate::core::types::memory::Memory {
    /// A recalled relation reaches the prompt through the memory section.
    fn from(relation: &ProfileRelation) -> Self {
        Self {
            id: uuid::Uuid::nil(),
            kind: crate::core::types::memory::MemoryKind::Fact,
            content: relation.to_string(),
            sensitive: false,
            created_at: chrono::Utc::now(),
            expires_at: None,
        }
    }
}

/// POST /profile/forget: who to forget, and optionally what.
#[derive(Debug, Deserialize)]
pub struct ForgetRequest {
    /// The user whose graph this is.
    pub user_id: String,
    /// Guild the request belongs to.
    #[serde(default)]
    pub tenant_id: Option<String>,
    /// One label to remove. Absent removes everything this user carries.
    #[serde(default)]
    pub label: Option<String>,
}

/// How much a forget removed.
#[derive(Debug, Serialize)]
pub struct ForgetResponse {
    /// Nodes removed. Relations through them go with them.
    pub removed: u64,
}

/// POST /profile/list: whose graph to show.
#[derive(Debug, Deserialize)]
pub struct ListRequest {
    /// The user whose graph this is.
    pub user_id: String,
    /// Guild the request belongs to.
    #[serde(default)]
    pub tenant_id: Option<String>,
}

/// One node as the list shows it.
#[derive(Debug, Serialize)]
pub struct ListedNode {
    /// What sort of thing it is.
    pub kind: String,
    /// How it is named.
    pub label: String,
    /// Confidence, 0 to 1.
    pub confidence: f32,
}

impl From<ProfileNode> for ListedNode {
    fn from(node: ProfileNode) -> Self {
        Self {
            kind: node.kind,
            label: node.label,
            confidence: node.confidence,
        }
    }
}

/// One relation as the list shows it, each end by label.
#[derive(Debug, Serialize)]
pub struct ListedRelation {
    /// Label of the entity the relation starts at.
    pub subject: String,
    /// What it asserts.
    pub relation: String,
    /// Label of the entity it points to.
    pub object: String,
    /// Confidence, 0 to 1.
    pub confidence: f32,
}

impl From<ProfileRelation> for ListedRelation {
    fn from(relation: ProfileRelation) -> Self {
        Self {
            subject: relation.subject.label,
            relation: relation.relation,
            object: relation.object.label,
            confidence: relation.confidence,
        }
    }
}

/// What the graph holds about the caller.
#[derive(Debug, Serialize)]
pub struct ListResponse {
    /// Nodes, most confident and most recently confirmed first.
    pub nodes: Vec<ListedNode>,
    /// Relations, most confident first.
    pub relations: Vec<ListedRelation>,
}
