-- LLM-as-judge eval harness.
-- Fixtures are pre-labeled queries with expected outcomes; runs record how
-- the current model/retriever scored against the expectation so we can detect
-- regressions when changing prompts, retrieval, or models.

CREATE TABLE IF NOT EXISTS eval_fixtures (
  id              TEXT PRIMARY KEY,
  kind            TEXT NOT NULL,        -- 'retrieval' | 'ask' | 'classification' | 'promotion'
  name            TEXT NOT NULL,
  input_json      TEXT NOT NULL,        -- query + any extra inputs
  expectation_json TEXT NOT NULL,       -- expected_ids, expected_kind, rubric, etc.
  notes           TEXT,
  enabled         INTEGER NOT NULL DEFAULT 1,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_eval_fixtures_kind ON eval_fixtures (kind, enabled);

CREATE TABLE IF NOT EXISTS eval_runs (
  id              TEXT PRIMARY KEY,
  fixture_id      TEXT NOT NULL REFERENCES eval_fixtures(id) ON DELETE CASCADE,
  ts              INTEGER NOT NULL,
  passed          INTEGER NOT NULL,     -- 1 / 0
  score           REAL    NOT NULL,     -- 0.0–1.0 for numeric metrics
  metric          TEXT NOT NULL,        -- 'precision_at_3' | 'judge_score' | 'accuracy'
  details_json    TEXT,                 -- observed top-K, judge rationale, etc.
  latency_ms      INTEGER NOT NULL DEFAULT 0,
  baseline_score  REAL,                 -- snapshot of the baseline at run time
  delta           REAL
);

CREATE INDEX IF NOT EXISTS idx_eval_runs_fixture ON eval_runs (fixture_id, ts DESC);
CREATE INDEX IF NOT EXISTS idx_eval_runs_ts ON eval_runs (ts DESC);

CREATE TABLE IF NOT EXISTS eval_baselines (
  fixture_id      TEXT PRIMARY KEY REFERENCES eval_fixtures(id) ON DELETE CASCADE,
  score           REAL NOT NULL,
  set_at          INTEGER NOT NULL,
  run_id          TEXT REFERENCES eval_runs(id) ON DELETE SET NULL
);
