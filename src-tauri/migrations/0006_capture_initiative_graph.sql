ALTER TABLE captures
  ADD COLUMN promoted_initiative_id TEXT REFERENCES initiatives(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_captures_promoted_initiative
  ON captures (promoted_initiative_id);
