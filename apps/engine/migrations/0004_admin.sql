-- Every change a moderator makes from the admin surface. The newest row per kind and key is the current value.

CREATE TABLE admin_changes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tenant_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('tool', 'instructions')),
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    actor_user_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX admin_changes_current ON admin_changes (tenant_id, kind, key, id);
