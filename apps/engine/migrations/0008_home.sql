-- Threads: a conversation started from one message of another, mostly the home conversation.

ALTER TABLE conversations ADD COLUMN parent_id TEXT;
ALTER TABLE conversations ADD COLUMN parent_seq INTEGER;

CREATE INDEX conversations_parent ON conversations (parent_id);
