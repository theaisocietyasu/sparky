//! Where the engine keeps conversations, memories and pending actions, and what it remembers.

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
    /// Hours between prunes of expired tasks, settled confirmations and idle conversations.
    pub prune_every_hours: u64,
    /// A conversation idle this many days is deleted with its messages. 0 keeps every one.
    pub conversation_idle_days: u32,
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
            prune_every_hours: 24,
            conversation_idle_days: 0,
        }
    }
}

/// What Sparky remembers about a student, and the key that seals sensitive memories at rest.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct MemorySettings {
    /// Base64 of 32 random bytes, from SPARKY_MEMORY__KEY.
    pub key: Option<SecretString>,
    /// Register the remember and forget_memory tools. They need store.backend sqlite.
    pub tools: bool,
    /// Memories kept per student; at the cap the oldest fact goes first.
    pub max_per_user: usize,
    /// Longest memory, in characters.
    pub max_chars: usize,
    /// A task due within this many days is recalled first.
    pub task_soon_days: i64,
    /// Lowercase fragments that make remember refuse the text.
    pub refuse_terms: Vec<String>,
    /// Lowercase fragments that make remember seal the text.
    pub sensitive_terms: Vec<String>,
}

impl Default for MemorySettings {
    fn default() -> Self {
        let terms = |t: &[&str]| t.iter().map(|s| (*s).to_owned()).collect();
        Self {
            key: None,
            tools: true,
            max_per_user: 50,
            max_chars: 200,
            task_soon_days: 7,
            refuse_terms: terms(&["password", "passcode", "ssn", "gpa", "grade", "diagnos"]),
            sensitive_terms: terms(&["address", "phone", "birthday"]),
        }
    }
}

/// Length of the memory key in bytes.
pub const MEMORY_KEY_BYTES: usize = 32;

impl MemorySettings {
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
pub fn validate_store(store: &Store, memory: &MemorySettings) -> Result<(), ConfigError> {
    let invalid = |m: &str| Err(ConfigError::Invalid(m.to_owned()));
    if memory.max_per_user == 0 {
        return invalid("memory.max_per_user must be at least 1");
    }
    if memory.max_chars == 0 {
        return invalid("memory.max_chars must be at least 1");
    }
    if memory.task_soon_days < 0 {
        return invalid("memory.task_soon_days must not be negative");
    }
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
    if store.prune_every_hours == 0 {
        return invalid("store.prune_every_hours must be at least 1");
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
    memory.bytes().map(|_| ())
}
