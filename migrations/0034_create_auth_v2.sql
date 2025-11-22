DROP TABLE login_tokens;
CREATE TABLE login_tokens (
    user_id INTEGER PRIMARY KEY NOT NULL,
    token TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    used_at TIMESTAMP
);

ALTER TABLE session_tokens RENAME TO session_tokens_old;
CREATE TABLE session_tokens (
    user_id INTEGER PRIMARY KEY NOT NULL,
    token TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO session_tokens (user_id, token, created_at)
    SELECT user_id, token, created_at
    FROM session_tokens_old;
DROP TABLE session_tokens_old;
