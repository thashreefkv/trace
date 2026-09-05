ALTER TABLE gcal_events ADD COLUMN conference_data TEXT;   -- JSON {solution_name, video_uri, phone_uri, conference_id}
ALTER TABLE gcal_events ADD COLUMN organizer TEXT;         -- JSON {email, name, self}
ALTER TABLE gcal_events ADD COLUMN recurring_event_id TEXT;
ALTER TABLE gcal_events ADD COLUMN recurrence TEXT;        -- JSON array of RRULE strings
ALTER TABLE gcal_events ADD COLUMN color_id TEXT;
ALTER TABLE gcal_events ADD COLUMN transparency TEXT;      -- "opaque" | "transparent"
ALTER TABLE gcal_events ADD COLUMN event_updated_at TEXT;
ALTER TABLE gcal_events ADD COLUMN attachments TEXT;       -- JSON array {fileUrl, title, mimeType, fileId}
ALTER TABLE gcal_events ADD COLUMN reminders TEXT;         -- JSON {useDefault, overrides:[{method,minutes}]}
ALTER TABLE gcal_events ADD COLUMN visibility TEXT;        -- "default" | "public" | "private" | "confidential"
