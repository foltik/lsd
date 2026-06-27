-- Add optional note column to manual RSVPs.
DROP TABLE IF EXISTS manual_rsvps_old;
ALTER TABLE manual_rsvps RENAME TO manual_rsvps_old;

CREATE TABLE manual_rsvps (
    event_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    creator_user_id INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checkin_at TIMESTAMP,
    note TEXT,
    PRIMARY KEY (event_id, user_id)
);

INSERT INTO manual_rsvps (event_id, user_id, creator_user_id, created_at, updated_at, checkin_at, note)
SELECT event_id, user_id, creator_user_id, created_at, updated_at, checkin_at, NULL
FROM manual_rsvps_old;

DROP TABLE manual_rsvps_old;
