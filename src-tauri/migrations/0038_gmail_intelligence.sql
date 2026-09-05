ALTER TABLE gmail_threads ADD COLUMN last_analyzed_message_at INTEGER;
ALTER TABLE gmail_threads ADD COLUMN last_analyzed_message_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE gmail_threads ADD COLUMN graph_context_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE gmail_threads ADD COLUMN effective_priority TEXT NOT NULL DEFAULT 'low';
ALTER TABLE gmail_threads ADD COLUMN priority_reasons_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE gmail_threads ADD COLUMN intelligence_updated_at TEXT;
ALTER TABLE gmail_threads ADD COLUMN last_analysis_error TEXT;

ALTER TABLE gmail_thread_deliverables ADD COLUMN confidence REAL;
ALTER TABLE gmail_thread_deliverables ADD COLUMN rationale TEXT NOT NULL DEFAULT '';

ALTER TABLE gmail_thread_initiatives ADD COLUMN confidence REAL;
ALTER TABLE gmail_thread_initiatives ADD COLUMN rationale TEXT NOT NULL DEFAULT '';

CREATE TABLE IF NOT EXISTS gmail_thread_stakeholders (
  thread_id      TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  stakeholder_id TEXT NOT NULL REFERENCES stakeholders(id) ON DELETE CASCADE,
  linked_at      TEXT NOT NULL,
  source         TEXT NOT NULL DEFAULT 'auto',
  confidence     REAL,
  rationale      TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (thread_id, stakeholder_id)
);

CREATE INDEX IF NOT EXISTS idx_gmail_thread_stakeholders_stakeholder
  ON gmail_thread_stakeholders (stakeholder_id, linked_at DESC);

CREATE TABLE IF NOT EXISTS gmail_thread_link_suggestions (
  id            TEXT PRIMARY KEY,
  thread_id     TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  target_kind   TEXT NOT NULL CHECK (target_kind IN ('stakeholder','deliverable','initiative')),
  target_id     TEXT NOT NULL,
  target_title  TEXT NOT NULL DEFAULT '',
  confidence    REAL NOT NULL DEFAULT 0,
  rationale     TEXT NOT NULL DEFAULT '',
  status        TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','accepted','rejected')),
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL,
  resolved_at   TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_gmail_thread_link_suggestions_unique_pending
  ON gmail_thread_link_suggestions (thread_id, target_kind, target_id, status);

CREATE INDEX IF NOT EXISTS idx_gmail_thread_link_suggestions_thread
  ON gmail_thread_link_suggestions (thread_id, status, confidence DESC);

CREATE INDEX IF NOT EXISTS idx_gmail_threads_effective_priority
  ON gmail_threads (effective_priority, last_message_at DESC);
