CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY NOT NULL,
    event_id INTEGER,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS event_notifications (
    event_id INTEGER NOT NULL,
    notification_id INTEGER NOT NULL,
    mins_before_start INTEGER NOT NULL,
    PRIMARY KEY (event_id, notification_id)
);
