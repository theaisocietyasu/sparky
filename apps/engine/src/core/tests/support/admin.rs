//! Admin store double: changes and held actions kept in memory.

use std::sync::Mutex;

use async_trait::async_trait;

use crate::core::traits::admin::AdminStore;
use crate::core::types::admin::{AdminChange, HeldAction, NewChange};
use crate::core::types::store::StoreError;

/// Records changes in order and holds the actions it is given.
#[derive(Default)]
pub struct Ledger {
    changes: Mutex<Vec<(String, AdminChange)>>,
    pub held: Mutex<Vec<HeldAction>>,
}

#[async_trait]
impl AdminStore for Ledger {
    async fn record(&self, new: NewChange) -> Result<AdminChange, StoreError> {
        let mut changes = self
            .changes
            .lock()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let change = AdminChange {
            id: i64::try_from(changes.len()).unwrap_or(0) + 1,
            kind: new.kind,
            key: new.key,
            value: new.value,
            actor: new.actor,
            created_at: "2026-11-04T00:00:00Z".into(),
        };
        changes.push((new.tenant_id, change.clone()));
        Ok(change)
    }

    async fn current(&self, tenant: &str) -> Result<Vec<AdminChange>, StoreError> {
        let changes = self
            .changes
            .lock()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let mut newest: Vec<AdminChange> = Vec::new();
        for (t, c) in changes.iter().rev() {
            if t == tenant && !newest.iter().any(|n| n.kind == c.kind && n.key == c.key) {
                newest.push(c.clone());
            }
        }
        Ok(newest)
    }

    async fn history(&self, tenant: &str, limit: usize) -> Result<Vec<AdminChange>, StoreError> {
        let changes = self
            .changes
            .lock()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(changes
            .iter()
            .rev()
            .filter(|(t, _)| t == tenant)
            .take(limit)
            .map(|(_, c)| c.clone())
            .collect())
    }

    async fn change(&self, tenant: &str, id: i64) -> Result<Option<AdminChange>, StoreError> {
        let changes = self
            .changes
            .lock()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(changes
            .iter()
            .find(|(t, c)| t == tenant && c.id == id)
            .map(|(_, c)| c.clone()))
    }

    async fn held(&self, _tenant: &str) -> Result<Vec<HeldAction>, StoreError> {
        Ok(self.held.lock().map(|h| h.clone()).unwrap_or_default())
    }

    async fn deny(&self, _tenant: &str, token: &str) -> Result<bool, StoreError> {
        let mut held = self
            .held
            .lock()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let before = held.len();
        held.retain(|h| h.token != token);
        Ok(held.len() < before)
    }
}
