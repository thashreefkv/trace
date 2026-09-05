CREATE TABLE IF NOT EXISTS brain_inferences (
  id             TEXT PRIMARY KEY,
  source_kind    TEXT NOT NULL,
  source_id      TEXT NOT NULL,
  relation_kind  TEXT NOT NULL,
  target_kind    TEXT NOT NULL,
  target_id      TEXT NOT NULL,
  confidence     REAL NOT NULL,
  rationale      TEXT NOT NULL DEFAULT '',
  evidence_json  TEXT NOT NULL DEFAULT '{}',
  status         TEXT NOT NULL DEFAULT 'pending',
  generated_by   TEXT NOT NULL DEFAULT 'system',
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL,
  reviewed_at    TEXT,
  CHECK (status IN ('pending', 'accepted', 'rejected'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_brain_inferences_identity
  ON brain_inferences (
    source_kind,
    source_id,
    relation_kind,
    target_kind,
    target_id,
    generated_by
  );

CREATE INDEX IF NOT EXISTS idx_brain_inferences_status
  ON brain_inferences (status, confidence DESC, updated_at DESC);

CREATE TABLE IF NOT EXISTS brain_answer_feedback (
  id             TEXT PRIMARY KEY,
  question       TEXT NOT NULL DEFAULT '',
  template       TEXT,
  feedback       TEXT NOT NULL,
  corrected_json TEXT NOT NULL DEFAULT '{}',
  created_at     TEXT NOT NULL,
  CHECK (feedback IN ('useful', 'wrong', 'ignored'))
);

CREATE INDEX IF NOT EXISTS idx_brain_answer_feedback_template
  ON brain_answer_feedback (template, created_at DESC);
