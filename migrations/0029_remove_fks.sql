-- list_members
ALTER TABLE list_members RENAME TO list_members_old;
CREATE TABLE list_members (
    list_id INTEGER NOT NULL,
    email TEXT NOT NULL,
    user_id INTEGER,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (list_id, email)
);
INSERT INTO list_members SELECT * FROM list_members_old;
DROP TABLE list_members_old;

-- session_tokens
ALTER TABLE session_tokens RENAME TO session_tokens_old;
CREATE TABLE session_tokens (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    token TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO session_tokens SELECT * FROM session_tokens_old;
DROP TABLE session_tokens_old;

-- rsvps
ALTER TABLE rsvps RENAME TO rsvps_old;
CREATE TABLE rsvps (
    id INTEGER PRIMARY KEY NOT NULL,
    event_id INTEGER NOT NULL,
    spot_id INTEGER NOT NULL,
    session_id INTEGER NOT NULL,
    contribution INTEGER NOT NULL,
    status TEXT NOT NULL,
    first_name TEXT,
    last_name TEXT,
    email TEXT,
    user_id INTEGER,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checkin_at TIMESTAMP
);
INSERT INTO rsvps SELECT * FROM rsvps_old;
DROP TABLE rsvps_old;

ALTER TABLE event_flyers RENAME TO event_flyers_old;
CREATE TABLE event_flyers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id INTEGER NOT NULL,
    width INTEGER NOT NULL DEFAULT 0,
    height INTEGER NOT NULL DEFAULT 0,
    image_full BLOB NOT NULL,
    image_lg BLOB NOT NULL,
    image_md BLOB NOT NULL,
    image_sm BLOB NOT NULL,
    image_thumb BLOB NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (event_id)
);
INSERT INTO event_flyers SELECT * FROM event_flyers_old;
DROP TABLE event_flyers_old;
