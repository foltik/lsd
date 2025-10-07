ALTER TABLE events RENAME TO events_old;
CREATE TABLE events (
    id INTEGER PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT NOT NULL,

    start TIMESTAMP NOT NULL,
    end TIMESTAMP,

    capacity INTEGER NOT NULL,
    unlisted BOOLEAN NOT NULL,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
);

INSERT INTO events (id, title, slug, description, start, end, capacity, unlisted, created_at, updated_at)
    SELECT id, title, slug, description, start, end, capacity, unlisted, created_at, updated_at
    FROM events_old;
DROP TABLE events_old;

CREATE UNIQUE INDEX events_slug_unique ON events(slug);
