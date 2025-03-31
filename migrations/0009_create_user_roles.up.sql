CREATE TABLE IF NOT EXISTS user_roles (
    user_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, role)
);