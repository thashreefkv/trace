-- Gantt: add start_date to deliverables for timeline bars
ALTER TABLE deliverables ADD COLUMN start_date TEXT;

-- Swim-lane sections within an initiative
CREATE TABLE IF NOT EXISTS initiative_sections (
  id           TEXT PRIMARY KEY,
  initiative_id TEXT NOT NULL REFERENCES initiatives(id) ON DELETE CASCADE,
  title        TEXT NOT NULL,
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_initiative_sections_initiative
  ON initiative_sections (initiative_id, position);

-- Link deliverables to sections (nullable — ungrouped deliverables get null)
ALTER TABLE deliverables ADD COLUMN section_id TEXT REFERENCES initiative_sections(id) ON DELETE SET NULL;
