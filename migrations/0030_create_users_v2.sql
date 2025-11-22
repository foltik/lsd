PRAGMA foreign_keys = OFF;

-- New users table
ALTER TABLE users RENAME TO users_old;
CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,

    email TEXT NOT NULL,
    first_name TEXT,
    last_name TEXT,
    phone TEXT,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO users (id, email, first_name, last_name, created_at, updated_at)
    SELECT id, email, first_name, last_name, created_at, created_at
    FROM users_old;
DROP TABLE users_old;

CREATE UNIQUE INDEX users_email_unique ON users(email);


-- Fix list_members to just reference user_id with no fk
ALTER TABLE list_members RENAME TO list_members_old;
CREATE TABLE list_members (
    list_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (list_id, user_id)
);
-- Populate old list_members to users
INSERT INTO users (email, created_at, updated_at)
    SELECT DISTINCT lm.email, lm.created_at, lm.created_at FROM list_members_old lm
    WHERE NOT EXISTS (
        SELECT 1 FROM users u
        WHERE u.email = lm.email
    );
-- Create new list_members
INSERT INTO list_members (list_id, user_id, created_at)
    SELECT lm.list_id, u.id, lm.created_at
    FROM list_members_old lm
    JOIN users u ON u.email = lm.email;
DROP TABLE list_members_old;


-- New user_history table
CREATE TABLE user_history (
    user_id INTEGER NOT NULL,
    version INTEGER NOT NULL,

    email TEXT NOT NULL,
    first_name TEXT,
    last_name TEXT,
    phone TEXT,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, version)
);
-- Populate initial history
INSERT INTO user_history (user_id, version, email, first_name, last_name, phone, created_at)
    SELECT id, 0, email, first_name, last_name, phone, created_at
    FROM users;


-- New user_attrs table
CREATE TABLE user_attrs (
    user_id INTEGER PRIMARY KEY NOT NULL,
    rsvp_guests u64 NOT NULL DEFAULT 0,
    rsvp_credits u64 NOT NULL DEFAULT 0,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
-- Populate initial attrs
INSERT INTO user_attrs (user_id, updated_at)
    SELECT id, created_at FROM users;
