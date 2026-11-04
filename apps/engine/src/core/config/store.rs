//! Where the engine keeps conversations, memories and pending actions.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use super::ConfigError;

/// Which adapter holds conversations, memories and pending actions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreBackend {
    /// The SQLite file at store.path.
    #[default]
    Sqlite,
    /// The platform agents routes, kept for one release as the rollback.
    Platform,
}

/// The local SQLite store and its backups.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Store {
    /// Which adapter holds conversations, memories and pending actions.
    pub backend: StoreBackend,
    /// Path of the SQLite file. The parent folder is created at boot.
    pub path: String,
    /// Pool size.
    pub max_connections: u32,
    /// How long a write waits on a lock, in milliseconds.
    pub busy_timeout_ms: u64,
    /// Folder for backup copies. Empty turns backups off.
    pub backup_dir: String,
    /// Hours between backup copies.
    pub backup_every_hours: u64,
    /// Newest backup copies kept.
    pub backups_kept: usize,
    /// The import from the platform leaves out conversations last updated longer ago, in days.
    pub import_days: u32,
    /// Rows asked for in one page of the platform export.
    pub import_page_size: usize,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            backend: StoreBackend::Sqlite,
            path: ".sparky/sparky.db".into(),
            max_connections: 4,
            busy_timeout_ms: 5000,
            backup_dir: String::new(),
            backup_every_hours: 24,
            backups_kept: 7,
            import_days: 30,
            import_page_size: 100,
        }
    }
}

/// The key that seals sensitive memories at rest.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct MemorySeal {
    /// Base64 of 32 random bytes, from SPARKY_MEMORY__KEY.
    pub key: Option<SecretString>,
}

/// Length of the memory key in bytes.
pub const MEMORY_KEY_BYTES: usize = 32;

impl MemorySeal {
    /// The decoded key, or why it is unusable.
    pub fn bytes(&self) -> Result<[u8; MEMORY_KEY_BYTES], ConfigError> {
        let invalid = |m: &str| ConfigError::Invalid(m.to_owned());
        let Some(key) = self.key.as_ref() else {
            return Err(invalid(
                "set SPARKY_MEMORY__KEY, base64 of 32 random bytes (openssl rand -base64 32)",
            ));
        };
        let decoded = STANDARD
            .decode(key.expose_secret().trim())
            .map_err(|_| invalid("SPARKY_MEMORY__KEY is not base64"))?;
        decoded
            .try_into()
            .map_err(|_| invalid("SPARKY_MEMORY__KEY must decode to 32 bytes"))
    }
}

/// Rejects a store that cannot open or back up, and a missing or malformed key.
pub fn validate_store(store: &Store, seal: &MemorySeal) -> Result<(), ConfigError> {
    let invalid = |m: &str| Err(ConfigError::Invalid(m.to_owned()));
    if store.backend != StoreBackend::Sqlite {
        return Ok(());
    }
    if store.path.trim().is_empty() {
        return invalid("store.path must not be empty");
    }
    if store.max_connections == 0 {
        return invalid("store.max_connections must be at least 1");
    }
    if !(1..=500).contains(&store.import_page_size) {
        return invalid("store.import_page_size must be from 1 to 500");
    }
    if !store.backup_dir.trim().is_empty() {
        if store.backups_kept == 0 {
            return invalid("store.backups_kept must be at least 1 when store.backup_dir is set");
        }
        if store.backup_every_hours == 0 {
            return invalid(
                "store.backup_every_hours must be at least 1 when store.backup_dir is set",
            );
        }
    }
    seal.bytes().map(|_| ())
}
