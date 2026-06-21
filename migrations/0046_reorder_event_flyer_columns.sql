-- SQLite stores a row's columns in declaration order and must walk the record's overflow pages to reach later columns.
-- Reorder event_flyers so the smaller images come first, and are faster to serve.
ALTER TABLE event_flyers RENAME TO event_flyers_old;

CREATE TABLE event_flyers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id INTEGER NOT NULL,
    width INTEGER NOT NULL DEFAULT 0,
    height INTEGER NOT NULL DEFAULT 0,
    image_thumb BLOB NOT NULL,
    image_sm BLOB NOT NULL,
    image_md BLOB NOT NULL,
    image_lg BLOB NOT NULL,
    image_full BLOB NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (event_id)
);

INSERT INTO event_flyers
    (id, event_id, width, height, image_thumb, image_sm, image_md, image_lg, image_full, created_at, updated_at)
SELECT
    id, event_id, width, height, image_thumb, image_sm, image_md, image_lg, image_full, created_at, updated_at
FROM event_flyers_old;

DROP TABLE event_flyers_old;
