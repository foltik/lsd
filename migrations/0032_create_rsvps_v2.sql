DROP TABLE rsvps;
DROP TABLE rsvp_sessions;

CREATE TABLE rsvps (
    id INTEGER PRIMARY KEY NOT NULL,
    session_id INTEGER NOT NULL,

    spot_id INTEGER NOT NULL,
    contribution INTEGER NOT NULL,
    user_id INTEGER,
    user_version INTEGER,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checkin_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS rsvp_sessions (
    id INTEGER PRIMARY KEY NOT NULL,
    event_id INTEGER NOT NULL,
    token TEXT NOT NULL,
    status TEXT NOT NULL,

    user_id INTEGER,
    user_version INTEGER,

    stripe_client_secret TEXT,
    stripe_payment_intent_id INTEGER,
    stripe_charge_id INTEGER,
    stripe_refund_id INTEGER,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
