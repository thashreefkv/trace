CREATE TABLE IF NOT EXISTS memory_settings (
  id                       INTEGER PRIMARY KEY CHECK (id = 1),
  enabled                  INTEGER NOT NULL DEFAULT 1,
  auto_extract_enabled     INTEGER NOT NULL DEFAULT 1,
  work_related_only        INTEGER NOT NULL DEFAULT 1,
  preserve_continuity      INTEGER NOT NULL DEFAULT 1,
  require_confirmation     INTEGER NOT NULL DEFAULT 0,
  retrieval_limit          INTEGER NOT NULL DEFAULT 12,
  updated_at               TEXT NOT NULL
);

INSERT OR IGNORE INTO memory_settings (
  id,
  enabled,
  auto_extract_enabled,
  work_related_only,
  preserve_continuity,
  require_confirmation,
  retrieval_limit,
  updated_at
) VALUES (
  1,
  1,
  1,
  1,
  1,
  0,
  12,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
);

CREATE TABLE IF NOT EXISTS memories (
  id                     TEXT PRIMARY KEY,
  kind                   TEXT NOT NULL CHECK (kind IN ('episodic', 'semantic', 'procedural')),
  status                 TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'superseded', 'archived', 'deleted')),
  scope                  TEXT NOT NULL DEFAULT 'global' CHECK (scope IN ('global', 'project', 'session')),
  title                  TEXT NOT NULL,
  body                   TEXT NOT NULL,
  canonical_key          TEXT NOT NULL,
  source                 TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'generated', 'consolidated', 'system')),
  source_kind            TEXT,
  source_id              TEXT,
  confidence             REAL NOT NULL DEFAULT 0.75,
  importance             REAL NOT NULL DEFAULT 0.50,
  retrieval_count        INTEGER NOT NULL DEFAULT 0,
  success_count          INTEGER NOT NULL DEFAULT 0,
  contradiction_count    INTEGER NOT NULL DEFAULT 0,
  tags_json              TEXT NOT NULL DEFAULT '[]',
  evidence_json          TEXT NOT NULL DEFAULT '[]',
  supersedes_id          TEXT REFERENCES memories(id) ON DELETE SET NULL,
  version                INTEGER NOT NULL DEFAULT 1,
  expires_at             TEXT,
  archived_at            TEXT,
  deleted_at             TEXT,
  last_retrieved_at      TEXT,
  created_at             TEXT NOT NULL,
  updated_at             TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_memories_active_key
  ON memories (canonical_key)
  WHERE status = 'active' AND deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_memories_kind_status
  ON memories (kind, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_memories_source
  ON memories (source_kind, source_id);

CREATE INDEX IF NOT EXISTS idx_memories_lifecycle
  ON memories (status, expires_at, last_retrieved_at);

CREATE TABLE IF NOT EXISTS memory_events (
  id             TEXT PRIMARY KEY,
  memory_id      TEXT REFERENCES memories(id) ON DELETE SET NULL,
  action         TEXT NOT NULL,
  detail_json    TEXT NOT NULL DEFAULT '{}',
  created_at     TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_events_memory
  ON memory_events (memory_id, created_at DESC);

CREATE TABLE IF NOT EXISTS memory_consolidation_runs (
  id                   TEXT PRIMARY KEY,
  source               TEXT NOT NULL,
  status               TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
  created_count        INTEGER NOT NULL DEFAULT 0,
  updated_count        INTEGER NOT NULL DEFAULT 0,
  archived_count       INTEGER NOT NULL DEFAULT 0,
  error_message        TEXT,
  started_at           TEXT NOT NULL,
  completed_at         TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_search USING fts5(
  memory_id UNINDEXED,
  title,
  body,
  tags,
  content=''
);

CREATE TRIGGER IF NOT EXISTS memories_ai
AFTER INSERT ON memories
BEGIN
  INSERT INTO memory_search(rowid, memory_id, title, body, tags)
  VALUES (new.rowid, new.id, new.title, new.body, new.tags_json);
END;

CREATE TRIGGER IF NOT EXISTS memories_ad
AFTER DELETE ON memories
BEGIN
  INSERT INTO memory_search(memory_search, rowid, memory_id, title, body, tags)
  VALUES ('delete', old.rowid, old.id, old.title, old.body, old.tags_json);
END;

CREATE TRIGGER IF NOT EXISTS memories_au
AFTER UPDATE OF title, body, tags_json, status, deleted_at ON memories
BEGIN
  INSERT INTO memory_search(memory_search, rowid, memory_id, title, body, tags)
  VALUES ('delete', old.rowid, old.id, old.title, old.body, old.tags_json);
  INSERT INTO memory_search(rowid, memory_id, title, body, tags)
  SELECT new.rowid, new.id, new.title, new.body, new.tags_json
  WHERE new.status != 'deleted' AND new.deleted_at IS NULL;
END;

INSERT OR IGNORE INTO memories (
  id,
  kind,
  status,
  scope,
  title,
  body,
  canonical_key,
  source,
  confidence,
  importance,
  tags_json,
  evidence_json,
  created_at,
  updated_at
) VALUES
  (
    'mem_default_global_work_memory',
    'procedural',
    'active',
    'global',
    'Global work memory is enabled',
    'Remember work-related context globally across the app: what the user does, how they work, and why they are doing the work.',
    'procedural:global-work-memory-enabled',
    'system',
    1.0,
    1.0,
    '["policy","global","work"]',
    '["User requested global first-class work memory by default."]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  ),
  (
    'mem_default_work_related_storage',
    'procedural',
    'active',
    'global',
    'Work-related memory does not require confirmation',
    'Store work-related facts, preferences, decisions, project context, and workflow patterns without asking for confirmation first.',
    'procedural:work-related-no-confirmation',
    'system',
    1.0,
    0.95,
    '["policy","capture"]',
    '["User said work-related information can be stored without extra confirmation."]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  ),
  (
    'mem_default_preserve_continuity',
    'procedural',
    'active',
    'global',
    'Preserve work continuity',
    'Prefer preserving useful work continuity over aggressive decay. Archive low-value or obsolete entries, but keep project history, decisions, preferences, and recurring workflow patterns available.',
    'procedural:preserve-work-continuity',
    'system',
    1.0,
    1.0,
    '["policy","lifecycle","continuity"]',
    '["User chose preserve work continuity."]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  ),
  (
    'mem_default_manual_and_generated',
    'procedural',
    'active',
    'global',
    'Use both manual and generated memory',
    'Memory should support both user-edited entries and generated/consolidated entries. Manual edits are higher authority than generated summaries.',
    'procedural:manual-and-generated-memory',
    'system',
    1.0,
    0.95,
    '["policy","manual","generated"]',
    '["User chose both manual and generated memory."]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );

