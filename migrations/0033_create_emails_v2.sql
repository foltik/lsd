DROP INDEX emails_post_list_address;

ALTER TABLE emails RENAME TO emails_old;
CREATE TABLE IF NOT EXISTS emails (
    id INTEGER PRIMARY KEY NOT NULL,
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

INSERT INTO emails (id, kind, user_id, user_version, list_id, post_id, event_id, notification_id, error, created_at, sent_at, opened_at)
    SELECT e.id, e.kind, u.id, 0, e.list_id, e.post_id, e.event_id, e.notification_id, e.error, e.created_at, e.sent_at, e.opened_at
    FROM emails_old e
    JOIN users u ON u.email = e.address;

CREATE INDEX emails_post_list_address
ON emails(user_id, post_id, list_id);
