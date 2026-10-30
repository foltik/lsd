CREATE TABLE clicker_deltas (
    id INTEGER PRIMARY KEY NOT NULL,
    time TIMESTAMP NOT NULL,
    delta INTEGER NOT NULL
);
CREATE INDEX clicker_deltas_time ON clicker_deltas(time);

CREATE TABLE clicker_stats (
    id INTEGER PRIMARY KEY NOT NULL,
    time TIMESTAMP NOT NULL,
    count INTEGER NOT NULL,
    voltage INTEGER NOT NULL,
    time_awake INTEGER NOT NULL,
    time_doze INTEGER NOT NULL,
    time_asleep INTEGER NOT NULL,
    rssi INTEGER NOT NULL,
    backlog INTEGER NOT NULL,
    connects INTEGER NOT NULL,
    disconnects INTEGER NOT NULL,
    errors INTEGER NOT NULL,
    error_resets INTEGER NOT NULL,
    last_error INTEGER NOT NULL,
    boot TIMESTAMP NOT NULL,
    firmware TEXT NOT NULL
);
