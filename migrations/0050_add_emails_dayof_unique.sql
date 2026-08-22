CREATE UNIQUE INDEX emails_event_dayof_unique
ON emails(event_id, user_id) WHERE kind = 'event/dayof';
