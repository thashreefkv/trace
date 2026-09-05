-- Explicit junction tables linking files and calendar events to initiatives.
-- These power scope-strict report generation: the resolver consults these tables
-- to decide whether a file/event belongs to a chosen initiative. Without them
-- there is no schema-level way to associate a Drive file or a calendar event
-- with the initiative it's about, so the report generator used to grab every
-- file and event in the workspace.

CREATE TABLE IF NOT EXISTS file_initiatives (
  file_id       TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  initiative_id TEXT NOT NULL REFERENCES initiatives(id) ON DELETE CASCADE,
  source        TEXT NOT NULL DEFAULT 'auto'
                CHECK (source IN ('auto', 'manual')),
  confidence    REAL NOT NULL DEFAULT 0.0,
  linked_at     TEXT NOT NULL,
  PRIMARY KEY (file_id, initiative_id)
);

CREATE INDEX IF NOT EXISTS idx_file_initiatives_initiative
  ON file_initiatives (initiative_id, confidence DESC);

CREATE TABLE IF NOT EXISTS gcal_event_initiatives (
  event_id      TEXT NOT NULL REFERENCES gcal_events(id) ON DELETE CASCADE,
  initiative_id TEXT NOT NULL REFERENCES initiatives(id) ON DELETE CASCADE,
  source        TEXT NOT NULL DEFAULT 'auto'
                CHECK (source IN ('auto', 'manual')),
  confidence    REAL NOT NULL DEFAULT 0.0,
  linked_at     TEXT NOT NULL,
  PRIMARY KEY (event_id, initiative_id)
);

CREATE INDEX IF NOT EXISTS idx_gcal_event_initiatives_initiative
  ON gcal_event_initiatives (initiative_id, confidence DESC);
