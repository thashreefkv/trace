-- Snapshot history of every Gemini analysis run for a thread.
-- A new row is inserted each time `analyze_thread_with_gemini` completes
-- (manual click or auto-analyze on new mail). This lets the UI show a
-- timeline + diff "what changed since last analysis."

CREATE TABLE IF NOT EXISTS gmail_thread_analysis_history (
  id              TEXT PRIMARY KEY,
  thread_id       TEXT NOT NULL,
  analyzed_at     TEXT NOT NULL,
  -- Trigger source: 'manual' (user clicked) or 'auto_new_mail' (sync detected
  -- a new inbound message) or 'auto_initial' (first sync after thread known).
  trigger         TEXT NOT NULL DEFAULT 'manual',
  -- The full GmailAiResult JSON for diffing across snapshots.
  result_json     TEXT NOT NULL,
  -- Convenience columns for ordering / filtering without parsing JSON.
  category        TEXT,
  priority        TEXT,
  summary         TEXT,
  message_count_at_analysis INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_analysis_history_thread
  ON gmail_thread_analysis_history(thread_id, analyzed_at DESC);
