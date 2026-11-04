//! Memory doubles: a memory store and a profile graph that count their recalls.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use uuid::Uuid;

use crate::core::traits::memory::MemoryStore;
use crate::core::traits::memory::profile::ProfileGraph;
use crate::core::types::agent::context::RequestContext;
use crate::core::types::memory::profile::{
    ProfileEntity, ProfileError, ProfileFact, ProfileNode, ProfileRelation,
};
use crate::core::types::memory::{Memory, MemoryKind, NewMemory};
use crate::core::types::store::StoreError;

/// A memory store in memory, starting with one memory, counting recalls.
pub struct Recalling {
    calls: AtomicUsize,
    held: Mutex<Vec<Memory>>,
}

impl Default for Recalling {
    fn default() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            held: Mutex::new(vec![Memory {
                id: Uuid::nil(),
                kind: MemoryKind::Fact,
                content: "studies CSE 310".into(),
                sensitive: false,
                created_at: chrono::Utc::now(),
                expires_at: None,
            }]),
        }
    }
}

impl Recalling {
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    /// The memories held now.
    pub fn held(&self) -> Vec<Memory> {
        self.held.lock().map(|h| h.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl MemoryStore for Recalling {
    async fn recall(&self, _ctx: &RequestContext) -> Result<Vec<Memory>, StoreError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(self.held())
    }

    async fn remember(
        &self,
        _ctx: &RequestContext,
        memory: &NewMemory,
    ) -> Result<Memory, StoreError> {
        let stored = Memory {
            id: Uuid::new_v4(),
            kind: memory.kind,
            content: memory.text.clone(),
            sensitive: memory.sensitive,
            created_at: chrono::Utc::now(),
            expires_at: None,
        };
        if let Ok(mut held) = self.held.lock() {
            held.push(stored.clone());
        }
        Ok(stored)
    }

    async fn forget(&self, _ctx: &RequestContext, id: Uuid) -> Result<bool, StoreError> {
        let Ok(mut held) = self.held.lock() else {
            return Ok(false);
        };
        let before = held.len();
        held.retain(|m| m.id != id);
        Ok(held.len() < before)
    }

    async fn forget_all(&self, _ctx: &RequestContext) -> Result<u64, StoreError> {
        let Ok(mut held) = self.held.lock() else {
            return Ok(0);
        };
        let count = held.len() as u64;
        held.clear();
        Ok(count)
    }
}

/// A profile graph holding one node and one relation, counting recalls.
#[derive(Default)]
pub struct Known {
    recalls: AtomicUsize,
}

impl Known {
    pub fn recalls(&self) -> usize {
        self.recalls.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl ProfileGraph for Known {
    async fn upsert(
        &self,
        _ctx: &RequestContext,
        _facts: &[ProfileFact],
    ) -> Result<(), ProfileError> {
        Ok(())
    }

    async fn recall(
        &self,
        _ctx: &RequestContext,
        _limit: usize,
    ) -> Result<Vec<ProfileNode>, ProfileError> {
        self.recalls.fetch_add(1, Ordering::Relaxed);
        Ok(vec![ProfileNode {
            id: Uuid::nil(),
            kind: "course".into(),
            label: "CSE 310".into(),
            confidence: 0.9,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }])
    }

    async fn relations(
        &self,
        _ctx: &RequestContext,
        _limit: usize,
    ) -> Result<Vec<ProfileRelation>, ProfileError> {
        Ok(vec![ProfileRelation {
            subject: ProfileEntity {
                kind: "person".into(),
                label: "the user".into(),
            },
            relation: "studies".into(),
            object: ProfileEntity {
                kind: "course".into(),
                label: "CSE 310".into(),
            },
            confidence: 0.8,
        }])
    }

    async fn matching(
        &self,
        _ctx: &RequestContext,
        _subject: &str,
        _relation: &str,
    ) -> Result<Vec<ProfileRelation>, ProfileError> {
        Ok(Vec::new())
    }

    async fn drop_relation(
        &self,
        _ctx: &RequestContext,
        _relation: &ProfileRelation,
    ) -> Result<bool, ProfileError> {
        Ok(false)
    }

    async fn forget(&self, _ctx: &RequestContext, _label: &str) -> Result<u64, ProfileError> {
        Ok(0)
    }

    async fn forget_all(&self, _ctx: &RequestContext) -> Result<u64, ProfileError> {
        Ok(0)
    }
}
