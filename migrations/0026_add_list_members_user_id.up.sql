ALTER TABLE list_members RENAME TO list_members_old;
CREATE TABLE list_members (
    list_id     INTEGER NOT NULL,
    email       TEXT NOT NULL,
    user_id     INTEGER,
    created_at  TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (list_id, email),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO list_members (list_id, email, created_at)
    SELECT list_id, email, created_at
    FROM list_members_old;
DROP TABLE list_members_old;

CREATE INDEX list_members_user_id ON list_members(user_id);
