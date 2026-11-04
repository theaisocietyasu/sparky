-- Memory kinds become fact, preference and task, and confidence goes. SQLite cannot change a
-- CHECK in place, so the table is rebuilt.

CREATE TABLE memories_new (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('fact', 'preference', 'task')),
    content BLOB NOT NULL,
    sensitive INTEGER NOT NULL DEFAULT 0,
    nonce BLOB,
    source_conversation TEXT,
    source_seq INTEGER,
    created_at TEXT NOT NULL,
    expires_at TEXT
);

INSERT INTO memories_new (id, tenant_id, user_id, kind, content, sensitive, nonce,
                          source_conversation, source_seq, created_at, expires_at)
SELECT id, tenant_id, user_id,
       CASE kind WHEN 'profile' THEN 'preference' WHEN 'task' THEN 'task' ELSE 'fact' END,
       content, sensitive, nonce, source_conversation, source_seq, created_at, expires_at
FROM memories;

DROP TABLE memories;
ALTER TABLE memories_new RENAME TO memories;
CREATE INDEX memories_owner ON memories (tenant_id, user_id, expires_at);
