ALTER TABLE gmail_threads ADD COLUMN ai_category TEXT NOT NULL DEFAULT 'other';
ALTER TABLE gmail_threads ADD COLUMN ai_priority TEXT NOT NULL DEFAULT 'low';
ALTER TABLE gmail_threads ADD COLUMN ai_category_confidence REAL;
ALTER TABLE gmail_threads ADD COLUMN ai_category_reasons TEXT NOT NULL DEFAULT '[]';
ALTER TABLE gmail_threads ADD COLUMN ai_triaged_at TEXT;

CREATE INDEX IF NOT EXISTS idx_gmail_threads_ai_category
  ON gmail_threads (ai_category, last_message_at DESC);

CREATE TABLE IF NOT EXISTS work_intake_suggestions (
  id                    TEXT PRIMARY KEY,
  source_kind           TEXT NOT NULL,
  source_id             TEXT,
  source_title          TEXT NOT NULL DEFAULT '',
  source_route          TEXT,
  item_kind             TEXT NOT NULL CHECK (item_kind IN ('task','deliverable','initiative')),
  title                 TEXT NOT NULL,
  body                  TEXT NOT NULL DEFAULT '',
  target_deliverable_id TEXT,
  target_initiative_id  TEXT,
  due_date              TEXT,
  suggested_type        TEXT,
  confidence            REAL,
  rationale             TEXT NOT NULL DEFAULT '',
  status                TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','dismissed')),
  payload               TEXT NOT NULL DEFAULT '{}',
  created_at            TEXT NOT NULL,
  updated_at            TEXT NOT NULL,
  applied_at            TEXT
);

CREATE INDEX IF NOT EXISTS idx_work_intake_status
  ON work_intake_suggestions (status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_work_intake_source
  ON work_intake_suggestions (source_kind, source_id, status);
