CREATE TABLE IF NOT EXISTS reservation_types (
    event_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    details TEXT,
    min_contribution INTEGER NOT NULL,
    max_contribution INTEGER NOT NULL,
    recommended_contribution INTEGER NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events(id),
    PRIMARY KEY (event_id, name)
);