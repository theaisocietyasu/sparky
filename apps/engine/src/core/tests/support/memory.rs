//! Memory doubles: a memory store that counts its recalls.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use uuid::Uuid;

use crate::core::traits::memory::MemoryStore;
use crate::core::types::agent::context::RequestContext;
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
