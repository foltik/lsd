CREATE TABLE IF NOT EXISTS rsvp_sessions (
    id INTEGER PRIMARY KEY NOT NULL,
    event_id INTEGER NOT NULL,
    token TEXT NOT NULL,
    status TEXT NOT NULL,

    first_name TEXT,
    last_name TEXT,
    email TEXT,
    user_id INTEGER,

    stripe_client_secret TEXT,
    stripe_payment_intent_id INTEGER,
    stripe_charge_id INTEGER,
    stripe_refund_id INTEGER,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
