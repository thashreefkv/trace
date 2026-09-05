CREATE TABLE IF NOT EXISTS meeting_stakeholders (
  meeting_id     TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  stakeholder_id TEXT NOT NULL REFERENCES stakeholders(id) ON DELETE CASCADE,
  PRIMARY KEY (meeting_id, stakeholder_id)
);

CREATE INDEX IF NOT EXISTS idx_meeting_stakeholders_meeting ON meeting_stakeholders (meeting_id);
CREATE INDEX IF NOT EXISTS idx_meeting_stakeholders_stakeholder ON meeting_stakeholders (stakeholder_id);
