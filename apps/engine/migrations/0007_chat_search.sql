-- Full text search over what students and Sparky said, for the search_chats tool. Only user and
-- assistant text is indexed; tool results and summaries are not. The index follows messages.

CREATE VIRTUAL TABLE messages_fts USING fts5(text, tokenize = 'porter unicode61');

INSERT INTO messages_fts (rowid, text)
SELECT seq, json_extract(content, '$.content') FROM messages
WHERE role IN ('user', 'assistant') AND json_extract(content, '$.content') != '';

CREATE TRIGGER messages_fts_insert AFTER INSERT ON messages
WHEN new.role IN ('user', 'assistant') AND json_extract(new.content, '$.content') != ''
BEGIN
    INSERT INTO messages_fts (rowid, text) VALUES (new.seq, json_extract(new.content, '$.content'));
END;

CREATE TRIGGER messages_fts_delete AFTER DELETE ON messages
BEGIN
    DELETE FROM messages_fts WHERE rowid = old.seq;
END;
