-- Each student's Canvas calendar feed link, sealed with the memory key. One row per student.

CREATE TABLE canvas_feeds (
    user_id TEXT PRIMARY KEY,
    nonce BLOB NOT NULL,
    sealed BLOB NOT NULL,
    created_at TEXT NOT NULL
);
