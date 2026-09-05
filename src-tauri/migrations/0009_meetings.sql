CREATE TABLE IF NOT EXISTS meetings (
  id              TEXT PRIMARY KEY,
  title           TEXT NOT NULL,
  date            TEXT NOT NULL,
  duration_secs   INTEGER,
  transcript      TEXT,
  summary         TEXT,
  key_decisions   TEXT,
  status          TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'done', 'error')),
  error_message   TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_meetings_date ON meetings (date DESC);

CREATE TABLE IF NOT EXISTS meeting_actions (
  id              TEXT PRIMARY KEY,
  meeting_id      TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  kind            TEXT NOT NULL CHECK (kind IN ('deliverable_note', 'initiative_note', 'capture')),
  target_id       TEXT,
  target_title    TEXT,
  body            TEXT NOT NULL,
  applied         INTEGER NOT NULL DEFAULT 0,
  created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_meeting_actions_meeting ON meeting_actions (meeting_id);

CREATE TABLE IF NOT EXISTS initiative_notes (
  id              TEXT PRIMARY KEY,
  initiative_id   TEXT NOT NULL REFERENCES initiatives(id) ON DELETE CASCADE,
  body            TEXT NOT NULL,
  created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_initiative_notes_initiative ON initiative_notes (initiative_id);
