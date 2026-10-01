DROP TABLE IF EXISTS emails_old;
ALTER TABLE emails RENAME TO emails_old;

CREATE TABLE emails (
    id INTEGER PRIMARY KEY NOT NULL,
    token TEXT NOT NULL,
    kind TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    user_version INTEGER NOT NULL,

    post_id INTEGER,
    list_id INTEGER,
    event_id INTEGER,
    notification_id INTEGER,

    error TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    sent_at TIMESTAMP,
    delivered_at TIMESTAMP, -- Added column
    opened_at TIMESTAMP
);

-- Backfill with delivered_at = sent_at
INSERT INTO emails (id, token, kind, user_id, user_version, post_id, list_id, event_id, notification_id, error, created_at, sent_at, delivered_at, opened_at)
SELECT id, token, kind, user_id, user_version, post_id, list_id, event_id, notification_id, error, created_at, sent_at, sent_at, opened_at
FROM emails_old;

DROP TABLE emails_old;

CREATE INDEX emails_post_list_address ON emails(user_id, post_id, list_id);
CREATE INDEX emails_token ON emails(token);
CREATE UNIQUE INDEX emails_event_confirmation_unique
ON emails(event_id, user_id) WHERE kind = 'event/confirmation';
CREATE UNIQUE INDEX emails_event_dayof_unique
ON emails(event_id, user_id) WHERE kind = 'event/dayof';
