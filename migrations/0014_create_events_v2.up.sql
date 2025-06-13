DROP TABLE IF EXISTS events;

CREATE TABLE events (
    id INTEGER PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT NOT NULL,
    flyer TEXT,

    start TIMESTAMP NOT NULL,
    end TIMESTAMP,

    unlisted BOOLEAN NOT NULL,
    guest_list_id INTEGER,
    target_revenue INTEGER,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS events_slug_unique ON events(slug);
