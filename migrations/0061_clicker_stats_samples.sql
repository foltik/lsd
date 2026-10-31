DROP TABLE IF EXISTS clicker_stats_old;
ALTER TABLE clicker_stats RENAME TO clicker_stats_old;
CREATE TABLE clicker_stats (
    id INTEGER PRIMARY KEY NOT NULL,
    time TIMESTAMP NOT NULL,
    boot TIMESTAMP NOT NULL,
    ticks INTEGER NOT NULL, -- added
    voltage INTEGER NOT NULL,
    rssi INTEGER NOT NULL,
    time_awake INTEGER NOT NULL,
    time_doze INTEGER NOT NULL,
    time_asleep INTEGER NOT NULL,
    firmware TEXT NOT NULL
);

INSERT INTO clicker_stats (id, time, boot, ticks, voltage, rssi, time_awake, time_doze, time_asleep, firmware)
    SELECT id, time, boot, -id, voltage, rssi, time_awake, time_doze, time_asleep, firmware
    FROM clicker_stats_old;

DROP TABLE clicker_stats_old;
CREATE UNIQUE INDEX clicker_stats_boot_ticks ON clicker_stats(boot, ticks);
CREATE INDEX clicker_stats_time ON clicker_stats(time);
