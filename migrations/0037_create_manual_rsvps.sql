-- Manual RSVPs for admin-added attendees (without checkout flow)
CREATE TABLE manual_rsvps (
    event_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    creator_user_id INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checkin_at TIMESTAMP,
    PRIMARY KEY (event_id, user_id)
);
