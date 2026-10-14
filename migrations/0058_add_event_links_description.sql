DROP TABLE IF EXISTS event_links_old;
ALTER TABLE event_links RENAME TO event_links_old;
CREATE TABLE event_links (
    id INTEGER PRIMARY KEY NOT NULL,
    event_id INTEGER NOT NULL,
    slug TEXT NOT NULL,
    description TEXT NOT NULL, -- added
    password TEXT,
    capacity INTEGER NOT NULL,
    bypass_capacity BOOLEAN NOT NULL,
    bypass_closed BOOLEAN NOT NULL,
    spots_per_person INTEGER,
    closed BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO event_links (id, event_id, slug, description, password, capacity, bypass_capacity, bypass_closed, spots_per_person, closed, created_at, updated_at)
    SELECT id, event_id, slug, '', password, capacity, bypass_capacity, bypass_closed, spots_per_person, closed, created_at, updated_at
    FROM event_links_old;

DROP TABLE event_links_old;
CREATE UNIQUE INDEX event_links_event_id_slug ON event_links(event_id, slug);
