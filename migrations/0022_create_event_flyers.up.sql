CREATE TABLE event_flyers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id INTEGER NOT NULL,
    image_full BLOB NOT NULL,
    image_lg BLOB NOT NULL,
    image_md BLOB NOT NULL,
    image_sm BLOB NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE,
    UNIQUE (event_id)
);
