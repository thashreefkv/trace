-- Work Mail read/review checkpoints.
--
-- Gmail read state remains mirrored on gmail_messages/gmail_threads. This
-- table stores Trace-only seen and review progress at thread granularity.

CREATE TABLE IF NOT EXISTS gmail_work_mail_thread_reviews (
  thread_id                   TEXT PRIMARY KEY,
  review_state                TEXT NOT NULL DEFAULT 'unreviewed',
  trace_seen_at               TEXT,
  seen_through_message_id     TEXT,
  seen_through_message_at     INTEGER,
  reviewed_through_message_id TEXT,
  reviewed_through_message_at INTEGER,
  handled_at                  TEXT,
  deferred_until              TEXT,
  reopened_at                 TEXT,
  updated_at                  TEXT NOT NULL,
  FOREIGN KEY (thread_id) REFERENCES gmail_threads(thread_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_gmail_work_mail_thread_reviews_state
  ON gmail_work_mail_thread_reviews (review_state, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_gmail_work_mail_thread_reviews_seen
  ON gmail_work_mail_thread_reviews (trace_seen_at, updated_at DESC);
