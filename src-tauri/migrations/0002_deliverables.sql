CREATE TABLE IF NOT EXISTS deliverables (
  id             TEXT PRIMARY KEY,
  title          TEXT NOT NULL,
  type           TEXT NOT NULL CHECK (type IN (
    'deck',
    'design_doc',
    'prototype',
    'analysis',
    'framework',
    'pitch',
    'research',
    'code',
    'email',
    'meeting_prep',
    'other'
  )),
  state          TEXT NOT NULL CHECK (state IN ('drafting','in_review','shipped','killed')),
  claim          TEXT NOT NULL DEFAULT '',
  artifact_url   TEXT,
  stakeholder_id TEXT REFERENCES stakeholders(id) ON DELETE SET NULL,
  created_at     TEXT NOT NULL,
  shipped_at     TEXT,
  updated_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_deliverables_state_updated
  ON deliverables (state, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_deliverables_stakeholder
  ON deliverables (stakeholder_id);

CREATE TABLE IF NOT EXISTS deliverable_initiatives (
  deliverable_id TEXT NOT NULL REFERENCES deliverables(id) ON DELETE CASCADE,
  initiative_id  TEXT NOT NULL REFERENCES initiatives(id) ON DELETE CASCADE,
  PRIMARY KEY (deliverable_id, initiative_id)
);

CREATE INDEX IF NOT EXISTS idx_deliverable_initiatives_initiative
  ON deliverable_initiatives (initiative_id, deliverable_id);

CREATE VIRTUAL TABLE IF NOT EXISTS deliverable_search USING fts5(
  deliverable_id UNINDEXED,
  title,
  claim,
  content=''
);

CREATE TRIGGER IF NOT EXISTS deliverables_ai
AFTER INSERT ON deliverables
BEGIN
  INSERT INTO deliverable_search(rowid, deliverable_id, title, claim)
  VALUES (new.rowid, new.id, new.title, new.claim);
END;

CREATE TRIGGER IF NOT EXISTS deliverables_ad
AFTER DELETE ON deliverables
BEGIN
  INSERT INTO deliverable_search(deliverable_search, rowid, deliverable_id, title, claim)
  VALUES ('delete', old.rowid, old.id, old.title, old.claim);
END;

CREATE TRIGGER IF NOT EXISTS deliverables_au
AFTER UPDATE OF title, claim ON deliverables
BEGIN
  INSERT INTO deliverable_search(deliverable_search, rowid, deliverable_id, title, claim)
  VALUES ('delete', old.rowid, old.id, old.title, old.claim);
  INSERT INTO deliverable_search(rowid, deliverable_id, title, claim)
  VALUES (new.rowid, new.id, new.title, new.claim);
END;
