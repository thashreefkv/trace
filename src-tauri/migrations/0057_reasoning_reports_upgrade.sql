-- Repair support for databases that created reasoning_runs before all
-- telemetry and proposal fields were added during initial development.
-- The missing columns are added conditionally in db.rs because SQLite does
-- not support ALTER TABLE ... ADD COLUMN IF NOT EXISTS.

CREATE TABLE IF NOT EXISTS reasoning_runs (
  id                        TEXT PRIMARY KEY,
  query_text                TEXT NOT NULL,
  depth                     TEXT NOT NULL DEFAULT 'deep',
  query_mode                TEXT NOT NULL,
  scope_json                TEXT NOT NULL DEFAULT '{}',
  result_markdown           TEXT NOT NULL DEFAULT '',
  citations_json            TEXT NOT NULL DEFAULT '[]',
  generated_assertions_json TEXT NOT NULL DEFAULT '[]',
  action_proposals_json     TEXT NOT NULL DEFAULT '[]',
  contradictions_json       TEXT NOT NULL DEFAULT '[]',
  unsupported_json          TEXT NOT NULL DEFAULT '[]',
  model                     TEXT NOT NULL,
  cache_hit                 INTEGER NOT NULL DEFAULT 0,
  latency_ms                INTEGER NOT NULL DEFAULT 0,
  status                    TEXT NOT NULL DEFAULT 'completed',
  created_at                TEXT NOT NULL,
  updated_at                TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reasoning_runs_created
  ON reasoning_runs (created_at DESC);

CREATE TABLE IF NOT EXISTS reasoning_cache (
  cache_key          TEXT PRIMARY KEY,
  model              TEXT NOT NULL,
  prompt_role        TEXT NOT NULL,
  query_mode         TEXT NOT NULL,
  source_fingerprint TEXT NOT NULL,
  synthesis_json     TEXT NOT NULL,
  created_at         TEXT NOT NULL,
  last_used_at       TEXT NOT NULL,
  hit_count          INTEGER NOT NULL DEFAULT 0
);
