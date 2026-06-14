-- Add a per-email token used for unguessable unsubscribe and open-pixel URLs.
-- Table rebuild (no ALTER ADD COLUMN); existing rows get a random token backfilled in the INSERT.
-- Uses an emails_new staging table because a stale emails_old from migration 0033 still exists.
CREATE TABLE emails_new (
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
    opened_at TIMESTAMP
);

INSERT INTO emails_new (id, token, kind, user_id, user_version, post_id, list_id, event_id, notification_id, error, created_at, sent_at, opened_at)
    SELECT id, lower(hex(randomblob(8))), kind, user_id, user_version, post_id, list_id, event_id, notification_id, error, created_at, sent_at, opened_at
    FROM emails;

DROP TABLE emails;
ALTER TABLE emails_new RENAME TO emails;

CREATE INDEX emails_post_list_address ON emails(user_id, post_id, list_id);
CREATE INDEX emails_token ON emails(token);
