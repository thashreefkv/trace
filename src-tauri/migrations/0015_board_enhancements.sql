-- Labels
CREATE TABLE IF NOT EXISTS labels (
  id    TEXT PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE,
  color TEXT NOT NULL DEFAULT 'zinc'
);

-- Deliverable ↔ label many-to-many
CREATE TABLE IF NOT EXISTS deliverable_labels (
  deliverable_id TEXT NOT NULL REFERENCES deliverables(id) ON DELETE CASCADE,
  label_id       TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
  PRIMARY KEY (deliverable_id, label_id)
);

CREATE INDEX IF NOT EXISTS idx_deliverable_labels_deliverable
  ON deliverable_labels (deliverable_id);

-- State-change history (friction log)
CREATE TABLE IF NOT EXISTS deliverable_state_history (
  id             TEXT PRIMARY KEY,
  deliverable_id TEXT NOT NULL REFERENCES deliverables(id) ON DELETE CASCADE,
  from_state     TEXT NOT NULL,
  to_state       TEXT NOT NULL,
  friction_note  TEXT,
  moved_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_state_history_deliverable
  ON deliverable_state_history (deliverable_id, moved_at DESC);
