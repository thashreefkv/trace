-- Cache of Google Calendar events for display in the week view
CREATE TABLE IF NOT EXISTS gcal_events (
    id          TEXT PRIMARY KEY NOT NULL,
    gcal_event_id TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL,
    description TEXT,
    start_date  TEXT NOT NULL,   -- YYYY-MM-DD (always set; date portion for timed events)
    end_date    TEXT,            -- YYYY-MM-DD for multi-day all-day events
    start_datetime TEXT,         -- ISO 8601 for timed events (null for all-day)
    end_datetime   TEXT,         -- ISO 8601 for timed events (null for all-day)
    is_all_day  INTEGER NOT NULL DEFAULT 1,
    html_link   TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Maps Trace entities (deliverables, tasks) → their Google Calendar event IDs
-- so we can update/delete rather than creating duplicates on re-sync
CREATE TABLE IF NOT EXISTS gcal_sync_map (
    entity_type   TEXT NOT NULL,  -- 'deliverable' | 'task'
    entity_id     TEXT NOT NULL,
    gcal_event_id TEXT NOT NULL,
    last_synced_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (entity_type, entity_id)
);
