CREATE TABLE event_links (
    id INTEGER PRIMARY KEY NOT NULL,
    event_id INTEGER NOT NULL,
    slug TEXT NOT NULL,
    password TEXT,
    capacity INTEGER NOT NULL,
    bypass_capacity BOOLEAN NOT NULL,
    bypass_closed BOOLEAN NOT NULL,
    spots_per_person INTEGER,
    closed BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX event_links_event_id_slug ON event_links(event_id, slug);


DROP TABLE IF EXISTS rsvp_sessions_old;
ALTER TABLE rsvp_sessions RENAME TO rsvp_sessions_old;
CREATE TABLE rsvp_sessions (
    id INTEGER PRIMARY KEY NOT NULL,
    event_id INTEGER NOT NULL,
    token TEXT NOT NULL,
    status TEXT NOT NULL,

    user_id INTEGER,
    user_version INTEGER,
    parent_session_id INTEGER REFERENCES rsvp_sessions(id),
    coupon_id INTEGER,
    link_id INTEGER, -- new

    stripe_checkout_session_id TEXT,
    stripe_client_secret TEXT,
    stripe_payment_intent_id TEXT,
    stripe_charge_id INTEGER,
    stripe_refund_id TEXT,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO rsvp_sessions (id, event_id, token, status, user_id, user_version, parent_session_id, coupon_id, link_id, stripe_checkout_session_id, stripe_client_secret, stripe_payment_intent_id, stripe_charge_id, stripe_refund_id, created_at, updated_at)
    SELECT id, event_id, token, status, user_id, user_version, parent_session_id, coupon_id, NULL, stripe_checkout_session_id, stripe_client_secret, stripe_payment_intent_id, stripe_charge_id, stripe_refund_id, created_at, updated_at
    FROM rsvp_sessions_old;

DROP TABLE rsvp_sessions_old;
CREATE INDEX rsvp_sessions_user_id ON rsvp_sessions(user_id);
