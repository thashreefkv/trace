CREATE TABLE IF NOT EXISTS deliverable_tasks (
  id             TEXT PRIMARY KEY,
  deliverable_id TEXT NOT NULL REFERENCES deliverables(id) ON DELETE CASCADE,
  title          TEXT NOT NULL,
  status         TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'doing', 'done')),
  due_date       TEXT,
  display_order  INTEGER NOT NULL DEFAULT 0,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_deliverable_tasks_deliverable
  ON deliverable_tasks (deliverable_id, display_order);

CREATE TABLE IF NOT EXISTS deliverable_notes (
  id             TEXT PRIMARY KEY,
  deliverable_id TEXT NOT NULL REFERENCES deliverables(id) ON DELETE CASCADE,
  body           TEXT NOT NULL,
  created_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_deliverable_notes_deliverable
  ON deliverable_notes (deliverable_id, created_at DESC);
