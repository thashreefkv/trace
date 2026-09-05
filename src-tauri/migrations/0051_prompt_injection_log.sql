-- Section 7 — Prompt injection defense audit log.
-- One row per sanitize/flag/truncate event on untrusted content arriving at a
-- Gemini prompt, and one row per destructive tool-call confirmation outcome.
-- Drives the "Prompt injection log" panel in Settings and gives the user a
-- full audit trail when the model refuses to act on attacker-controlled
-- instructions buried in an email/web page/capture.

CREATE TABLE IF NOT EXISTS prompt_injection_log (
  id                TEXT PRIMARY KEY,
  ts                INTEGER NOT NULL,
  source            TEXT NOT NULL,            -- 'email' | 'web' | 'capture' | 'memory' | 'tool_confirm' | 'tool_reject'
  origin_kind       TEXT,                     -- e.g. 'gmail_thread', 'fetch_url', 'capture'
  origin_id         TEXT,                     -- thread id, capture id, url, etc.
  run_id            TEXT,                     -- when triggered from an Ask turn
  call_id           TEXT,                     -- when triggered from a specific tool call
  tool              TEXT,                     -- when triggered from tool dispatch
  action_taken     TEXT NOT NULL,             -- 'sanitized' | 'flagged' | 'truncated' | 'refused' | 'confirmed' | 'rejected'
  reason            TEXT NOT NULL DEFAULT '',
  flags_json        TEXT NOT NULL DEFAULT '[]',
  content_excerpt   TEXT NOT NULL DEFAULT '', -- first 1024 chars of the relevant chunk
  original_bytes    INTEGER NOT NULL DEFAULT 0,
  sanitized_bytes   INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_prompt_injection_ts
  ON prompt_injection_log (ts DESC);

CREATE INDEX IF NOT EXISTS idx_prompt_injection_source_ts
  ON prompt_injection_log (source, ts DESC);

CREATE INDEX IF NOT EXISTS idx_prompt_injection_run
  ON prompt_injection_log (run_id, ts DESC) WHERE run_id IS NOT NULL;
