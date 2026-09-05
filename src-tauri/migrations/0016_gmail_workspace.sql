CREATE TABLE IF NOT EXISTS gmail_sync_settings (
  id                         INTEGER PRIMARY KEY CHECK (id = 1),
  sync_enabled               INTEGER NOT NULL DEFAULT 1,
  sync_interval_hours        INTEGER NOT NULL DEFAULT 4,
  notification_poll_minutes  INTEGER NOT NULL DEFAULT 5,
  max_threads_per_sync       INTEGER NOT NULL DEFAULT 150,
  include_sent               INTEGER NOT NULL DEFAULT 1,
  include_drafts             INTEGER NOT NULL DEFAULT 1,
  notify_new_mail            INTEGER NOT NULL DEFAULT 1,
  backfill_enabled           INTEGER NOT NULL DEFAULT 1,
  backfill_page_token        TEXT,
  backfill_query             TEXT,
  last_backfill_at           TEXT,
  backfill_completed_at      TEXT,
  account_email              TEXT,
  last_sync_started_at       TEXT,
  last_sync_completed_at     TEXT,
  last_history_id            TEXT,
  last_error                 TEXT
);

INSERT OR IGNORE INTO gmail_sync_settings (
  id,
  sync_enabled,
  sync_interval_hours,
  notification_poll_minutes,
  max_threads_per_sync,
  include_sent,
  include_drafts,
  notify_new_mail
) VALUES (1, 1, 4, 5, 150, 1, 1, 1);

CREATE TABLE IF NOT EXISTS gmail_labels (
  gmail_label_id TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  type           TEXT NOT NULL DEFAULT '',
  color          TEXT,
  updated_at     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS gmail_threads (
  thread_id       TEXT PRIMARY KEY,
  subject         TEXT NOT NULL DEFAULT '',
  snippet         TEXT NOT NULL DEFAULT '',
  participants    TEXT NOT NULL DEFAULT '[]',
  first_message_at INTEGER,
  last_message_at  INTEGER,
  message_count   INTEGER NOT NULL DEFAULT 0,
  has_unread      INTEGER NOT NULL DEFAULT 0,
  is_sent_only    INTEGER NOT NULL DEFAULT 0,
  last_from_name  TEXT NOT NULL DEFAULT '',
  last_from_email TEXT NOT NULL DEFAULT '',
  summary         TEXT,
  sentiment       TEXT,
  urgency         TEXT,
  ai_generated_at TEXT,
  last_sync_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gmail_threads_last_message
  ON gmail_threads (last_message_at DESC);

CREATE TABLE IF NOT EXISTS gmail_messages (
  message_id        TEXT PRIMARY KEY,
  thread_id         TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  history_id        TEXT,
  subject           TEXT NOT NULL DEFAULT '',
  snippet           TEXT NOT NULL DEFAULT '',
  from_name         TEXT NOT NULL DEFAULT '',
  from_email        TEXT NOT NULL DEFAULT '',
  to_json           TEXT NOT NULL DEFAULT '[]',
  cc_json           TEXT NOT NULL DEFAULT '[]',
  bcc_json          TEXT NOT NULL DEFAULT '[]',
  date_ts           INTEGER,
  internal_date_ts  INTEGER,
  plain_body        TEXT NOT NULL DEFAULT '',
  html_body         TEXT NOT NULL DEFAULT '',
  raw_headers_json  TEXT NOT NULL DEFAULT '{}',
  label_ids_json    TEXT NOT NULL DEFAULT '[]',
  is_sent           INTEGER NOT NULL DEFAULT 0,
  is_draft          INTEGER NOT NULL DEFAULT 0,
  is_unread         INTEGER NOT NULL DEFAULT 0,
  size_estimate     INTEGER,
  artifact_urls_json TEXT NOT NULL DEFAULT '[]',
  synced_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gmail_messages_thread
  ON gmail_messages (thread_id, internal_date_ts ASC);

CREATE INDEX IF NOT EXISTS idx_gmail_messages_from
  ON gmail_messages (from_email, internal_date_ts DESC);

CREATE TABLE IF NOT EXISTS gmail_attachments (
  id                TEXT PRIMARY KEY,
  message_id        TEXT NOT NULL REFERENCES gmail_messages(message_id) ON DELETE CASCADE,
  thread_id         TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  attachment_id     TEXT,
  filename          TEXT NOT NULL DEFAULT '',
  mime_type         TEXT NOT NULL DEFAULT '',
  size              INTEGER,
  shared_by_email   TEXT NOT NULL DEFAULT '',
  shared_with_json  TEXT NOT NULL DEFAULT '[]',
  created_at        TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_gmail_attachments_unique
  ON gmail_attachments (message_id, COALESCE(attachment_id, ''), filename);

CREATE INDEX IF NOT EXISTS idx_gmail_attachments_thread
  ON gmail_attachments (thread_id);

CREATE TABLE IF NOT EXISTS gmail_thread_labels (
  thread_id       TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  gmail_label_id  TEXT NOT NULL REFERENCES gmail_labels(gmail_label_id) ON DELETE CASCADE,
  PRIMARY KEY (thread_id, gmail_label_id)
);

CREATE INDEX IF NOT EXISTS idx_gmail_thread_labels_label
  ON gmail_thread_labels (gmail_label_id, thread_id);

CREATE TABLE IF NOT EXISTS gmail_participants (
  email          TEXT PRIMARY KEY,
  name           TEXT NOT NULL DEFAULT '',
  first_seen_at  TEXT NOT NULL,
  last_seen_at   TEXT NOT NULL,
  sent_count     INTEGER NOT NULL DEFAULT 0,
  received_count INTEGER NOT NULL DEFAULT 0,
  thread_count   INTEGER NOT NULL DEFAULT 0,
  last_thread_id TEXT
);

CREATE TABLE IF NOT EXISTS gmail_thread_participants (
  thread_id     TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  email         TEXT NOT NULL,
  name          TEXT NOT NULL DEFAULT '',
  role          TEXT NOT NULL,
  message_count INTEGER NOT NULL DEFAULT 0,
  first_seen_at TEXT NOT NULL,
  last_seen_at  TEXT NOT NULL,
  PRIMARY KEY (thread_id, email, role)
);

CREATE INDEX IF NOT EXISTS idx_gmail_thread_participants_email
  ON gmail_thread_participants (email, last_seen_at DESC);

CREATE TABLE IF NOT EXISTS gmail_drafts (
  draft_id      TEXT PRIMARY KEY,
  message_id    TEXT NOT NULL,
  thread_id     TEXT,
  subject       TEXT NOT NULL DEFAULT '',
  to_json       TEXT NOT NULL DEFAULT '[]',
  cc_json       TEXT NOT NULL DEFAULT '[]',
  bcc_json      TEXT NOT NULL DEFAULT '[]',
  body_preview  TEXT NOT NULL DEFAULT '',
  updated_at    TEXT,
  synced_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gmail_drafts_thread
  ON gmail_drafts (thread_id);

CREATE TABLE IF NOT EXISTS gmail_links (
  id          TEXT PRIMARY KEY,
  thread_id   TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  message_id  TEXT REFERENCES gmail_messages(message_id) ON DELETE CASCADE,
  url         TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'url',
  title       TEXT,
  created_at  TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_gmail_links_unique
  ON gmail_links (thread_id, url);

CREATE INDEX IF NOT EXISTS idx_gmail_links_thread
  ON gmail_links (thread_id);

CREATE TABLE IF NOT EXISTS gmail_thread_deliverables (
  thread_id      TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  deliverable_id TEXT NOT NULL REFERENCES deliverables(id) ON DELETE CASCADE,
  linked_at      TEXT NOT NULL,
  source         TEXT NOT NULL DEFAULT 'manual',
  PRIMARY KEY (thread_id, deliverable_id)
);

CREATE INDEX IF NOT EXISTS idx_gmail_thread_deliverables_deliverable
  ON gmail_thread_deliverables (deliverable_id, linked_at DESC);

CREATE TABLE IF NOT EXISTS gmail_thread_initiatives (
  thread_id     TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  initiative_id TEXT NOT NULL REFERENCES initiatives(id) ON DELETE CASCADE,
  linked_at     TEXT NOT NULL,
  source        TEXT NOT NULL DEFAULT 'manual',
  PRIMARY KEY (thread_id, initiative_id)
);

CREATE INDEX IF NOT EXISTS idx_gmail_thread_initiatives_initiative
  ON gmail_thread_initiatives (initiative_id, linked_at DESC);

CREATE TABLE IF NOT EXISTS gmail_thread_captures (
  thread_id  TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  capture_id TEXT NOT NULL REFERENCES captures(id) ON DELETE CASCADE,
  linked_at  TEXT NOT NULL,
  PRIMARY KEY (thread_id, capture_id)
);

CREATE TABLE IF NOT EXISTS gmail_followups (
  id                        TEXT PRIMARY KEY,
  thread_id                 TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  message_id                TEXT REFERENCES gmail_messages(message_id) ON DELETE SET NULL,
  sent_at                   TEXT NOT NULL,
  expected_reply_after_days INTEGER NOT NULL DEFAULT 3,
  due_at                    TEXT NOT NULL,
  status                    TEXT NOT NULL DEFAULT 'open',
  resolved_at               TEXT,
  created_at                TEXT NOT NULL,
  updated_at                TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gmail_followups_status_due
  ON gmail_followups (status, due_at ASC);

CREATE TABLE IF NOT EXISTS gmail_ai_suggestions (
  id          TEXT PRIMARY KEY,
  thread_id   TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  kind        TEXT NOT NULL,
  title       TEXT NOT NULL DEFAULT '',
  body        TEXT NOT NULL DEFAULT '',
  payload     TEXT NOT NULL DEFAULT '{}',
  status      TEXT NOT NULL DEFAULT 'pending',
  created_at  TEXT NOT NULL,
  applied_at  TEXT
);

CREATE INDEX IF NOT EXISTS idx_gmail_ai_suggestions_thread
  ON gmail_ai_suggestions (thread_id, created_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS gmail_thread_search USING fts5(
  thread_id UNINDEXED,
  subject,
  participants,
  body
);
