-- Same columns as rsvps, except:
CREATE TABLE deleted_rsvps (
    id INTEGER NOT NULL, -- no PRIMARY KEY, just mirrors rsvps row's id
    session_id INTEGER NOT NULL,

    spot_id INTEGER NOT NULL,
    contribution INTEGER NOT NULL,
    discount INTEGER NOT NULL,
    user_id INTEGER,
    user_version INTEGER,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    checkin_at TIMESTAMP,
    note TEXT,

    deleted_at TIMESTAMP NOT NULL -- plus deletion timestamp
);
