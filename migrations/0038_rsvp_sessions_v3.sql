CREATE TABLE rsvp_sessions_new (
    id INTEGER PRIMARY KEY NOT NULL,
    event_id INTEGER NOT NULL,
    token TEXT NOT NULL,
    status TEXT NOT NULL,

    user_id INTEGER,
    user_version INTEGER,

    stripe_client_secret TEXT,
    stripe_payment_intent_id TEXT,
    stripe_charge_id INTEGER,
    stripe_refund_id TEXT,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO rsvp_sessions_new SELECT * FROM rsvp_sessions;
DROP TABLE rsvp_sessions;
ALTER TABLE rsvp_sessions_new RENAME TO rsvp_sessions;
