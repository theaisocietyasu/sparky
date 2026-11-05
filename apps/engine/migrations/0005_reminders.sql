-- Reminders a student sets, and the notifications they become when due. Text is sealed with the memory key.

CREATE TABLE reminders (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    nonce BLOB NOT NULL,
    sealed BLOB NOT NULL,
    due_at TEXT NOT NULL,
    repeat TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX reminders_due ON reminders (due_at);
CREATE INDEX reminders_user ON reminders (tenant_id, user_id);

CREATE TABLE notifications (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    nonce BLOB NOT NULL,
    sealed BLOB NOT NULL,
    created_at TEXT NOT NULL,
    read_at TEXT,
    sent_at TEXT
);

CREATE INDEX notifications_user ON notifications (tenant_id, user_id, created_at);
CREATE INDEX notifications_unsent ON notifications (sent_at, created_at);
