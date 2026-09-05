ALTER TABLE gcal_events ADD COLUMN location TEXT;
ALTER TABLE gcal_events ADD COLUMN attendees TEXT; -- JSON array of {email, name, self, responseStatus}
