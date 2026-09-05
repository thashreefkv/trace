-- 0062: Brain layout cache.
-- Persists computed node positions per layout mode, keyed on a graph_version
-- hash so we can invalidate on Kuzu rebuild without manual book-keeping.
--
-- Why a separate table from `brain_saved_views`: those are user-named layouts
-- (snapshots with filters + viewport + selection). This one is the system's
-- automatic cache so the page paints in < 100ms on revisits regardless of
-- whether the user saved a view.

CREATE TABLE IF NOT EXISTS brain_layout_cache (
  mode           TEXT NOT NULL,
  graph_version  TEXT NOT NULL,
  entity_kind    TEXT NOT NULL,
  entity_id      TEXT NOT NULL,
  x              REAL NOT NULL,
  y              REAL NOT NULL,
  z              REAL,
  computed_at    INTEGER NOT NULL,
  PRIMARY KEY (mode, entity_kind, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_brain_layout_cache_version
  ON brain_layout_cache (mode, graph_version);
