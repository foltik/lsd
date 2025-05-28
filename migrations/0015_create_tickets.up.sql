CREATE TABLE IF NOT EXISTS tickets (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS event_tickets (
    event_id INTEGER NOT NULL,
    ticket_id INTEGER NOT NULL,
    price INTEGER NOT NULL,
    quantity INTEGER NOT NULL,
    sort INTEGER NOT NULL,
    PRIMARY KEY (event_id, ticket_id)
);
