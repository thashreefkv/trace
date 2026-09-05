CREATE TABLE IF NOT EXISTS gmail_thread_stakeholder_excludes (
  thread_id      TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
  stakeholder_id TEXT NOT NULL REFERENCES stakeholders(id) ON DELETE CASCADE,
  excluded_at    TEXT NOT NULL,
  PRIMARY KEY (thread_id, stakeholder_id)
);
