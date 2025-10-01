CREATE TABLE IF NOT EXISTS spots (
    id INTEGER PRIMARY KEY NOT NULL,

    name TEXT NOT NULL,
    description TEXT NOT NULL,
    qty_total INTEGER NOT NULL,
    qty_per_person INTEGER NOT NULL,
    kind TEXT NOT NULL,
    sort INTEGER NOT NULL,

    -- kind = 'fixed'
    required_contribution INTEGER,
    -- kind = 'variable'
    min_contribution INTEGER,
    max_contribution INTEGER,
    suggested_contribution INTEGER,
    -- kind = 'work'
    required_notice_hours INTEGER,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS event_spots (
    event_id INTEGER NOT NULL,
    spot_id INTEGER NOT NULL,
    PRIMARY KEY (event_id, spot_id)
);
