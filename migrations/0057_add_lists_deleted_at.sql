DROP TABLE IF EXISTS lists_old;
ALTER TABLE lists RENAME TO lists_old;
CREATE TABLE lists (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP
);

INSERT INTO lists (id, name, description, created_at, updated_at, deleted_at)
    SELECT id, name, description, created_at, updated_at, NULL
    FROM lists_old;

DROP TABLE lists_old;
