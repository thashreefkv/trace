CREATE TABLE IF NOT EXISTS captures (
  id                      TEXT PRIMARY KEY,
  kind                    TEXT NOT NULL CHECK (kind IN ('thought','claude_link','artifact_link')),
  body                    TEXT NOT NULL,
  status                  TEXT NOT NULL DEFAULT 'inbox' CHECK (status IN ('inbox','promoted','dismissed')),
  promoted_deliverable_id TEXT REFERENCES deliverables(id) ON DELETE SET NULL,
  created_at              TEXT NOT NULL,
  updated_at              TEXT NOT NULL,
  promoted_at             TEXT
);

CREATE INDEX IF NOT EXISTS idx_captures_status_created
  ON captures (status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_captures_kind_status
  ON captures (kind, status);

CREATE INDEX IF NOT EXISTS idx_captures_promoted_deliverable
  ON captures (promoted_deliverable_id);
