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
    guest_list_id INTEGER,

    invite_html TEXT,
    invite_updated_at TIMESTAMP,
    invite_sent_at TIMESTAMP,

    confirmation_html TEXT,
    confirmation_updated_at TIMESTAMP,

    dayof_html TEXT,
    dayof_updated_at TIMESTAMP,
    dayof_sent_at TIMESTAMP,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO events (id, title, slug, description, start, end, capacity, unlisted, created_at, updated_at)
    SELECT id, title, slug, description, start, end, capacity, unlisted, created_at, updated_at
    FROM events_old;
