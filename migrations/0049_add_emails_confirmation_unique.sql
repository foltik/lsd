CREATE UNIQUE INDEX emails_event_confirmation_unique
ON emails(event_id, user_id) WHERE kind = 'event/confirmation';
