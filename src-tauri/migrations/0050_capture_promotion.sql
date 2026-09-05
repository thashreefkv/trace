-- AI-suggested promotions for captures (Section 4 of the rebuild).
-- A new row lands every time `suggest_capture_promotion` runs (either auto on
-- create or via explicit refresh). At most one row per capture has
-- `status='pending'` at any time; the suggester marks older pendings as
-- `stale` in the same tx before inserting the new one. History is preserved
-- so the RL feedback loop can reason about accept/override patterns over time.

CREATE TABLE IF NOT EXISTS capture_promotion_suggestions (
  id                  TEXT PRIMARY KEY,
  capture_id          TEXT NOT NULL,
  kind                TEXT NOT NULL,                       -- 'task' | 'deliverable' | 'initiative'
  target_id           TEXT,                                -- deliverable_id / initiative_id when relevant
  target_kind         TEXT,                                -- 'deliverable' | 'initiative' | NULL
  confidence          REAL NOT NULL DEFAULT 0.0,
  rationale           TEXT NOT NULL DEFAULT '',
  alternatives_json   TEXT NOT NULL DEFAULT '[]',
  status              TEXT NOT NULL DEFAULT 'pending',     -- pending | accepted | accepted_alternative | overridden | stale | errored | undone
  error_reason        TEXT,
  applied_entity_kind TEXT,                                -- snapshot of what `apply_*` created
  applied_entity_id   TEXT,
  model               TEXT NOT NULL DEFAULT '',
  latency_ms          INTEGER NOT NULL DEFAULT 0,
  created_at          INTEGER NOT NULL,
  resolved_at         INTEGER
);

CREATE INDEX IF NOT EXISTS idx_capture_promotion_capture
  ON capture_promotion_suggestions (capture_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_capture_promotion_recent
  ON capture_promotion_suggestions (status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_capture_promotion_resolved
  ON capture_promotion_suggestions (resolved_at DESC) WHERE resolved_at IS NOT NULL;
