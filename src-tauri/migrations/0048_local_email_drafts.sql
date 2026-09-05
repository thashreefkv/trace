-- Local-first email drafts. Persisted before any Gmail draft sync so the
-- composer survives window closes, app restarts, and disconnects.
-- One draft per thread (UNIQUE(thread_id)); a NULL thread_id is allowed for a
-- single "new message" draft (UNIQUE index below).

CREATE TABLE IF NOT EXISTS local_email_drafts (
  id              TEXT PRIMARY KEY,
  thread_id       TEXT,
  to_json         TEXT NOT NULL DEFAULT '[]',
  cc_json         TEXT NOT NULL DEFAULT '[]',
  bcc_json        TEXT NOT NULL DEFAULT '[]',
  subject         TEXT NOT NULL DEFAULT '',
  body_html       TEXT NOT NULL DEFAULT '',
  body_text       TEXT NOT NULL DEFAULT '',
  gmail_draft_id  TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_local_email_drafts_thread
  ON local_email_drafts(thread_id)
  WHERE thread_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS local_email_draft_attachments (
  id          TEXT PRIMARY KEY,
  draft_id    TEXT NOT NULL REFERENCES local_email_drafts(id) ON DELETE CASCADE,
  filename    TEXT NOT NULL,
  mime_type   TEXT NOT NULL,
  file_size   INTEGER NOT NULL,
  file_path   TEXT NOT NULL,
  created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_local_email_draft_attachments_draft
  ON local_email_draft_attachments(draft_id);
