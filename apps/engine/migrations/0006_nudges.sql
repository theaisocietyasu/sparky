-- Nudges: notifications Sparky sends on its own. Each notification says what made it and whether the
-- student found it useful. nudges_sent keeps what was already said, so nothing is said twice.

ALTER TABLE notifications ADD COLUMN kind TEXT NOT NULL DEFAULT 'reminder';
ALTER TABLE notifications ADD COLUMN helpful INTEGER;

CREATE TABLE nudges_sent (
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    key TEXT NOT NULL,
    kind TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, user_id, key)
);

CREATE INDEX nudges_sent_age ON nudges_sent (created_at);

CREATE TABLE nudge_prefs (
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    quiet_start INTEGER NOT NULL,
    quiet_end INTEGER NOT NULL,
    off TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, user_id)
);
