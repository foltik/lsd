CREATE TABLE IF NOT EXISTS emails (
    id INTEGER PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    address TEXT NOT NULL,
    post_id INTEGER,
    list_id INTEGER,
    error TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    sent_at TIMESTAMP,
    opened_at TIMESTAMP
);