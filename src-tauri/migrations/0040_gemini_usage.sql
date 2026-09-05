-- Gemini API usage + cost tracking.
-- Every call writes one row so /settings can show per-feature breakdowns.

CREATE TABLE IF NOT EXISTS gemini_usage_log (
  id                TEXT PRIMARY KEY,
  ts                INTEGER NOT NULL,
  feature           TEXT NOT NULL,        -- 'ask' | 'brain_extract' | 'email_classify' | ...
  model             TEXT NOT NULL,
  prompt_tokens     INTEGER NOT NULL DEFAULT 0,
  completion_tokens INTEGER NOT NULL DEFAULT 0,
  cached_tokens     INTEGER NOT NULL DEFAULT 0,   -- subset of prompt_tokens served from cache
  total_tokens      INTEGER NOT NULL DEFAULT 0,
  est_cost_usd      REAL    NOT NULL DEFAULT 0,
  latency_ms        INTEGER NOT NULL DEFAULT 0,
  error             TEXT
);

CREATE INDEX IF NOT EXISTS idx_gemini_usage_ts ON gemini_usage_log (ts DESC);
CREATE INDEX IF NOT EXISTS idx_gemini_usage_feature ON gemini_usage_log (feature, ts DESC);
CREATE INDEX IF NOT EXISTS idx_gemini_usage_model ON gemini_usage_log (model, ts DESC);
