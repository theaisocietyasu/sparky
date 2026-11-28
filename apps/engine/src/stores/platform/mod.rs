//! Adapters over the platform HTTP API: search, saved pages and readiness.
//!
//! The platform takes the organization from the machine token, so tenant_id is never sent.

mod client;
mod discovery;
mod health;
mod query;

pub use client::PlatformClient;
pub use discovery::PlatformDiscovery;
pub use health::PlatformProbe;
pub use query::PlatformQueries;
