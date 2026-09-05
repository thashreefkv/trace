CREATE TABLE IF NOT EXISTS week_plans (
  week_start     TEXT NOT NULL,
  day_index      INTEGER NOT NULL CHECK (day_index BETWEEN 0 AND 4),
  deliverable_id TEXT REFERENCES deliverables(id) ON DELETE SET NULL,
  updated_at     TEXT NOT NULL,
  PRIMARY KEY (week_start, day_index)
);

CREATE TABLE IF NOT EXISTS meeting_config (
  id                TEXT NOT NULL DEFAULT 'singleton' PRIMARY KEY,
  next_meeting_date TEXT,
  updated_at        TEXT NOT NULL
);
