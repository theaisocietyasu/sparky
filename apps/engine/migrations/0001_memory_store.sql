-- Conversations, messages, memories and pending actions of the engine. Times are RFC 3339 UTC text.

CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    visibility TEXT NOT NULL CHECK (visibility IN ('public', 'private')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    ended_at TEXT
);
CREATE INDEX conversations_owner
    ON conversations (tenant_id, user_id, channel_id, visibility, updated_at);

CREATE TABLE messages (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    covers INTEGER,
    created_at TEXT NOT NULL
);
CREATE INDEX messages_conversation ON messages (conversation_id, seq);

CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('episodic', 'semantic', 'profile', 'task')),
    content BLOB NOT NULL,
    sensitive INTEGER NOT NULL DEFAULT 0,
    nonce BLOB,
    confidence REAL NOT NULL,
    source_conversation TEXT,
    source_seq INTEGER,
    created_at TEXT NOT NULL,
    expires_at TEXT
);
CREATE INDEX memories_owner ON memories (tenant_id, user_id, expires_at);

CREATE TABLE pending_actions (
    token TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'confirmed', 'denied')),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    resolved_at TEXT
);

CREATE TABLE imports (
    source TEXT PRIMARY KEY,
    finished_at TEXT NOT NULL,
    conversations INTEGER NOT NULL,
    messages INTEGER NOT NULL,
    memories INTEGER NOT NULL
);
