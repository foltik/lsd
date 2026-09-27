DROP TABLE IF EXISTS manual_rsvps_old;
ALTER TABLE manual_rsvps RENAME TO manual_rsvps_old;


CREATE TABLE manual_rsvps (
    id INTEGER PRIMARY KEY NOT NULL,  -- Added
    event_id INTEGER NOT NULL,
    creator_user_id INTEGER NOT NULL,
    user_id INTEGER,                  -- Made nullable
    first_name TEXT,                  -- Added
    last_name TEXT,                   -- Added
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checkin_at TIMESTAMP,
    note TEXT
);
CREATE UNIQUE INDEX manual_rsvps_event_id_user_id ON manual_rsvps(event_id, user_id);


INSERT INTO manual_rsvps (event_id, creator_user_id, user_id, first_name, last_name, created_at, updated_at, checkin_at, note)
SELECT event_id, creator_user_id, user_id, NULL, NULL, created_at, updated_at, checkin_at, note
FROM manual_rsvps_old;


DROP TABLE manual_rsvps_old;
