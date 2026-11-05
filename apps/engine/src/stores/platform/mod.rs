//! Adapters over the platform HTTP API: every store the engine needs, behind the same traits.
//!
//! The platform takes the organization from the machine token, so tenant_id is never sent.
//! The member is user_id, which the platform requires to be a numeric Discord user id.

mod accounts;
mod client;
mod confirmation;
mod conversation;
mod discovery;
mod export;
mod health;
mod memory;
mod query;
mod retrieval;

pub use accounts::PlatformAccounts;
pub use client::PlatformClient;
pub use confirmation::PlatformConfirmations;
pub use conversation::PlatformConversations;
pub use discovery::PlatformDiscovery;
pub use export::PlatformExport;
pub use health::PlatformProbe;
pub use memory::PlatformMemory;
pub use query::PlatformQueries;
pub use retrieval::{PlatformRetriever, SearchTuning};
