DROP TABLE IF EXISTS meeting_actions_new;

CREATE TABLE meeting_actions_new (
  id              TEXT PRIMARY KEY,
  meeting_id      TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  kind            TEXT NOT NULL,
  target_id       TEXT,
  target_title    TEXT,
  body            TEXT NOT NULL,
  applied         INTEGER NOT NULL DEFAULT 0,
  created_at      TEXT NOT NULL
);

INSERT INTO meeting_actions_new
  (id, meeting_id, kind, target_id, target_title, body, applied, created_at)
SELECT
  id, meeting_id, kind, target_id, target_title, body, applied, created_at
FROM meeting_actions;

DROP TABLE meeting_actions;

ALTER TABLE meeting_actions_new RENAME TO meeting_actions;

CREATE INDEX IF NOT EXISTS idx_meeting_actions_meeting ON meeting_actions (meeting_id);
