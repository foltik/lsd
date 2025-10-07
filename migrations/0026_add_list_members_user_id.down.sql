ALTER TABLE list_members RENAME TO list_members_old;

CREATE TABLE list_members (
    list_id     INTEGER NOT NULL,
    email       TEXT NOT NULL,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (list_id, email)
);

INSERT INTO list_members (list_id, email, created_at)
    SELECT list_id, email, created_at
    FROM list_members_old;
