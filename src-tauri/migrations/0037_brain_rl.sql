CREATE TABLE IF NOT EXISTS brain_rl_events (
  id TEXT PRIMARY KEY,
  template TEXT NOT NULL DEFAULT '',
  item_id TEXT NOT NULL,
  item_kind TEXT NOT NULL DEFAULT '',
  event_type TEXT NOT NULL,
  reward REAL NOT NULL,
  context_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_brain_rl_events_template_item
  ON brain_rl_events (template, item_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_brain_rl_events_type
  ON brain_rl_events (event_type, created_at DESC);

CREATE TABLE IF NOT EXISTS brain_rl_policies (
  template TEXT PRIMARY KEY,
  feature_names_json TEXT NOT NULL,
  a_matrix_json TEXT NOT NULL,
  b_vector_json TEXT NOT NULL,
  observations INTEGER NOT NULL DEFAULT 0,
  alpha REAL NOT NULL DEFAULT 0.75,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS brain_rl_item_scores (
  template TEXT NOT NULL,
  item_id TEXT NOT NULL,
  item_kind TEXT NOT NULL DEFAULT '',
  score REAL NOT NULL,
  exploitation REAL NOT NULL,
  exploration REAL NOT NULL,
  features_json TEXT NOT NULL DEFAULT '{}',
  updated_at TEXT NOT NULL,
  PRIMARY KEY (template, item_id)
);

CREATE INDEX IF NOT EXISTS idx_brain_rl_item_scores_template
  ON brain_rl_item_scores (template, score DESC, updated_at DESC);
