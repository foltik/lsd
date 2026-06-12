DROP TABLE IF EXISTS events_old;
ALTER TABLE events RENAME TO events_old;
CREATE TABLE events (
    id INTEGER PRIMARY KEY NOT NULL,
    token TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    url TEXT,
    start TIMESTAMP NOT NULL,
    end TIMESTAMP,
    capacity INTEGER NOT NULL,
    unlisted BOOLEAN NOT NULL,
    closed BOOLEAN NOT NULL DEFAULT FALSE,
    guest_list_id INTEGER,
    spots_per_person INTEGER,
    artist_share INTEGER NOT NULL,

    description_html TEXT,
    description_updated_at TIMESTAMP,

    invite_subject TEXT,
    invite_html TEXT,
    invite_updated_at TIMESTAMP,
    invite_sent_at TIMESTAMP,

    confirmation_subject TEXT,
    confirmation_html TEXT,
    confirmation_updated_at TIMESTAMP,

    dayof_subject TEXT,
    dayof_html TEXT,
    dayof_updated_at TIMESTAMP,
    dayof_sent_at TIMESTAMP,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO events (id, token, kind, title, slug, url, start, end, capacity, unlisted, closed, guest_list_id, spots_per_person, artist_share, description_html, description_updated_at, invite_subject, invite_html, invite_updated_at, invite_sent_at, confirmation_subject, confirmation_html, confirmation_updated_at, dayof_subject, dayof_html, dayof_updated_at, dayof_sent_at, created_at, updated_at)
    SELECT id, token, 'internal', title, slug, NULL, start, end, capacity, unlisted, closed, guest_list_id, spots_per_person, artist_share, description_html, description_updated_at, invite_subject, invite_html, invite_updated_at, invite_sent_at, confirmation_subject, confirmation_html, confirmation_updated_at, dayof_subject, dayof_html, dayof_updated_at, dayof_sent_at, created_at, updated_at
    FROM events_old;

DROP TABLE events_old;
