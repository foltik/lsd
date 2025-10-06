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
    checkin_at TIMESTAMP,
    FOREIGN KEY (session_id) REFERENCES rsvp_sessions(id) ON DELETE CASCADE
);

INSERT INTO rsvps SELECT * FROM rsvps_old;
DROP TABLE rsvps_old;

CREATE INDEX rsvps_session_id ON rsvps(session_id);
