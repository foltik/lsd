CREATE TABLE IF NOT EXISTS list_members (
    list_id INTEGER NOT NULL,
    email TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (list_id, email)
);