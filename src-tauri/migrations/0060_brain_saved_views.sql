-- Phase 2 of the Brain explorer: persisted "saved views" so users can stash a
-- combination of filters + layout + camera viewport + pinned-node positions and
-- restore them with one click from the left rail.

CREATE TABLE IF NOT EXISTS brain_saved_views (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    filters_json    TEXT NOT NULL DEFAULT '{}',
    layout_json     TEXT NOT NULL DEFAULT '{}',
    viewport_json   TEXT NOT NULL DEFAULT '{}',
    pinned_json     TEXT NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_brain_saved_views_updated_at
    ON brain_saved_views (updated_at DESC);
