CREATE TABLE IF NOT EXISTS flyers (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    x INTEGER NOT NULL,
    y INTEGER NOT NULL,
    rotation INTEGER NOT NULL DEFAULT 0,
    -- TODO(sam) this needs to be sequentialized
    z_index INTEGER NOT NULL DEFAULT 0,
    image_url TEXT NOT NULL,
    event_name TEXT,
    event_url TEXT,
    event_end TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
