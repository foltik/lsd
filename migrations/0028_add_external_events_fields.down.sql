DELETE FROM events WHERE external_event_url IS NOT NULL;
ALTER TABLE events DROP COLUMN external_event_url;