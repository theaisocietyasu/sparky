//! The SQLite store through its public adapters, on a private in-memory database.

mod backup;
mod confirmation;
mod conversation;
mod import;
mod memory;
mod seal;

use std::time::Duration;

use crate::core::types::agent::context::RequestContext;
use crate::stores::sqlite::{Seal, SqliteStore, StoreOptions};

/// A fresh in-memory store with its migrations applied.
async fn store() -> SqliteStore {
    let options = StoreOptions {
        path: "sqlite::memory:".into(),
        max_connections: 1,
        busy_timeout: Duration::from_secs(1),
    };
    match SqliteStore::open(&options).await {
        Ok(store) => store,
        Err(e) => unreachable!("an in-memory store opens: {e}"),
    }
}

/// A seal over a fixed test key.
fn seal() -> Seal {
    Seal::new(&[7; 32])
}

/// User 111 in tenant guild.
fn caller() -> RequestContext {
    RequestContext::new("guild", "111", Duration::from_secs(30))
}

/// User 222 in tenant guild.
fn other() -> RequestContext {
    RequestContext::new("guild", "222", Duration::from_secs(30))
}
