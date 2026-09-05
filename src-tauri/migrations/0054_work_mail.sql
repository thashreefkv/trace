-- Canonical Work Mail dimensions and correction controls.
--
-- Keep the existing ai_category fields for compatibility while the Email
-- workspace migrates to Work Mail views and filters.

ALTER TABLE gmail_threads ADD COLUMN work_relevance TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE gmail_threads ADD COLUMN work_relevance_reasons_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE gmail_threads ADD COLUMN work_relevance_confidence REAL;
ALTER TABLE gmail_threads ADD COLUMN attention_state TEXT NOT NULL DEFAULT 'fyi';
ALTER TABLE gmail_threads ADD COLUMN attention_reasons_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE gmail_threads ADD COLUMN attention_confidence REAL;
ALTER TABLE gmail_threads ADD COLUMN message_type TEXT NOT NULL DEFAULT 'other';
ALTER TABLE gmail_threads ADD COLUMN message_type_reasons_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE gmail_threads ADD COLUMN message_type_confidence REAL;
ALTER TABLE gmail_threads ADD COLUMN work_mail_updated_at TEXT;

ALTER TABLE gmail_user_classifications ADD COLUMN work_relevance TEXT;
ALTER TABLE gmail_user_classifications ADD COLUMN attention_state TEXT;
ALTER TABLE gmail_user_classifications ADD COLUMN message_type TEXT;

ALTER TABLE gmail_sender_rules ADD COLUMN work_relevance TEXT;
ALTER TABLE gmail_sender_rules ADD COLUMN attention_state TEXT;
ALTER TABLE gmail_sender_rules ADD COLUMN message_type TEXT;

CREATE INDEX IF NOT EXISTS idx_gmail_threads_work_relevance
  ON gmail_threads (work_relevance, last_message_at DESC);
CREATE INDEX IF NOT EXISTS idx_gmail_threads_attention_state
  ON gmail_threads (attention_state, last_message_at DESC);
CREATE INDEX IF NOT EXISTS idx_gmail_threads_message_type
  ON gmail_threads (message_type, last_message_at DESC);

CREATE TABLE IF NOT EXISTS gmail_work_domains (
  domain      TEXT PRIMARY KEY,
  enabled     INTEGER NOT NULL DEFAULT 1,
  source      TEXT NOT NULL DEFAULT 'seed',
  note        TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gmail_work_domains_enabled
  ON gmail_work_domains (enabled, domain);

CREATE TABLE IF NOT EXISTS gmail_work_mail_agent_events (
  id              TEXT PRIMARY KEY,
  thread_id       TEXT,
  event_kind      TEXT NOT NULL,
  actor           TEXT NOT NULL,
  summary         TEXT NOT NULL,
  reason_json     TEXT NOT NULL DEFAULT '[]',
  payload_json    TEXT NOT NULL DEFAULT '{}',
  undo_payload_json TEXT,
  created_at      TEXT NOT NULL,
  FOREIGN KEY (thread_id) REFERENCES gmail_threads(thread_id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_gmail_work_mail_agent_events_created
  ON gmail_work_mail_agent_events (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_gmail_work_mail_agent_events_thread
  ON gmail_work_mail_agent_events (thread_id, created_at DESC);
