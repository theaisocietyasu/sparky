//! Store adapters. The only place a database connection or a platform client is opened.
//!
//! Imports only core. platform holds the adapters over the platform HTTP API; sqlite holds the
//! conversations, memories and pending actions in one SQLite file; discord holds the web sign-in.

pub mod discord;
pub mod platform;
pub mod sqlite;
