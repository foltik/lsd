CREATE TABLE coupons (
    id INTEGER PRIMARY KEY NOT NULL,
    creator_user_id INTEGER NOT NULL,
    event_id INTEGER,
    code TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    expires_at TIMESTAMP,
    max_uses INTEGER,
    max_uses_per_user INTEGER,
    max_uses_per_event INTEGER,
    kind TEXT NOT NULL,
    percent_off INTEGER,
    dollars_off INTEGER,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE coupon_users (
    coupon_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (coupon_id, user_id)
);


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
    coupon_id INTEGER, -- new

    stripe_checkout_session_id TEXT,
    stripe_client_secret TEXT,
    stripe_payment_intent_id TEXT,
    stripe_charge_id INTEGER,
    stripe_refund_id TEXT,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO rsvp_sessions (id, event_id, token, status, user_id, user_version, parent_session_id, coupon_id, stripe_checkout_session_id, stripe_client_secret, stripe_payment_intent_id, stripe_charge_id, stripe_refund_id, created_at, updated_at)
    SELECT id, event_id, token, status, user_id, user_version, parent_session_id, NULL, stripe_checkout_session_id, stripe_client_secret, stripe_payment_intent_id, stripe_charge_id, stripe_refund_id, created_at, updated_at
    FROM rsvp_sessions_old;

DROP TABLE rsvp_sessions_old;
CREATE INDEX rsvp_sessions_user_id ON rsvp_sessions(user_id);


DROP TABLE IF EXISTS rsvps_old;
ALTER TABLE rsvps RENAME TO rsvps_old;
CREATE TABLE rsvps (
    id INTEGER PRIMARY KEY NOT NULL,
    session_id INTEGER NOT NULL,

    spot_id INTEGER NOT NULL,
    contribution INTEGER NOT NULL,
    discount INTEGER NOT NULL, -- new
    user_id INTEGER,
    user_version INTEGER,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checkin_at TIMESTAMP,
    note TEXT
);

INSERT INTO rsvps (id, session_id, spot_id, contribution, discount, user_id, user_version, created_at, updated_at, checkin_at, note)
    SELECT id, session_id, spot_id, contribution, 0, user_id, user_version, created_at, updated_at, checkin_at, note
    FROM rsvps_old;

DROP TABLE rsvps_old;
CREATE INDEX rsvps_user_id ON rsvps(user_id);
