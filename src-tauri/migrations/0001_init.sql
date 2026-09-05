CREATE TABLE IF NOT EXISTS initiatives (
  id         TEXT PRIMARY KEY,
  title      TEXT NOT NULL,
  framing    TEXT NOT NULL DEFAULT '',
  status     TEXT NOT NULL CHECK (status IN ('live','paused','shipped','parked')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_initiatives_status_updated
  ON initiatives (status, updated_at DESC);

CREATE TABLE IF NOT EXISTS stakeholders (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL UNIQUE,
  display_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_stakeholders_display_order
  ON stakeholders (display_order, name);
