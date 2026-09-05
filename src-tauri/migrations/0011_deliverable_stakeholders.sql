CREATE TABLE IF NOT EXISTS deliverable_stakeholders (
  deliverable_id  TEXT NOT NULL REFERENCES deliverables(id) ON DELETE CASCADE,
  stakeholder_id  TEXT NOT NULL REFERENCES stakeholders(id) ON DELETE CASCADE,
  PRIMARY KEY (deliverable_id, stakeholder_id)
);

CREATE INDEX IF NOT EXISTS idx_deliverable_stakeholders_stakeholder
  ON deliverable_stakeholders (stakeholder_id, deliverable_id);

INSERT OR IGNORE INTO deliverable_stakeholders (deliverable_id, stakeholder_id)
SELECT id, stakeholder_id
FROM deliverables
WHERE stakeholder_id IS NOT NULL AND stakeholder_id != '';
