-- Step state machine for the report generation pipeline. The outer
-- report_runs.status ('created'/'outline_approved'/'approved') is preserved
-- for backwards-compatibility. This table tracks the fine-grained pipeline:
-- resolve_scope, plan_sections, draft_section_<N>, critique, etc.
--
-- Every step emits report:event over Tauri so the UI shows a live timeline.
-- Steps can pause for clarification, cache results, and be re-run individually.

CREATE TABLE IF NOT EXISTS report_steps (
  id                 TEXT PRIMARY KEY,
  report_run_id      TEXT NOT NULL REFERENCES report_runs(id) ON DELETE CASCADE,
  step_name          TEXT NOT NULL,
  section_index      INTEGER,
  status             TEXT NOT NULL DEFAULT 'queued'
                     CHECK (status IN ('queued', 'running', 'awaiting_clarification',
                                       'done', 'error', 'cancelled')),
  model              TEXT,
  cache_hit          INTEGER NOT NULL DEFAULT 0,
  started_at         TEXT,
  finished_at        TEXT,
  latency_ms         INTEGER,
  input_json         TEXT NOT NULL DEFAULT '{}',
  output_json        TEXT NOT NULL DEFAULT '{}',
  clarification_json TEXT,
  ticker_label       TEXT,
  error_text         TEXT,
  created_at         TEXT NOT NULL,
  updated_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_report_steps_run
  ON report_steps (report_run_id, created_at);

CREATE INDEX IF NOT EXISTS idx_report_steps_status
  ON report_steps (status, updated_at DESC);

-- Per-report exclusions: source units the user un-ticked in the scope preview.
-- Persisted JSON array of source_unit ids; consulted on every regenerate.
-- (Added via add_column_if_missing in db.rs; ALTER lives in code, not SQL,
-- because SQLite doesn't support ADD COLUMN IF NOT EXISTS.)
--
-- Same for sections_json: the planned outline as a structured array
-- (heading + instructions + state) and section_drafts_json: keyed by section_id.
