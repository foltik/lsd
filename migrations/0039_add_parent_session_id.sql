ALTER TABLE rsvp_sessions ADD COLUMN parent_session_id INTEGER REFERENCES rsvp_sessions(id);
