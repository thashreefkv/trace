-- Semantic embeddings for retrieval-relevant entities.
-- Used alongside BM25 + recency in `brain::score_node` to give hybrid retrieval.
-- One row per (entity_kind, entity_id). The worker re-embeds rows whose
-- `content_hash` no longer matches the current text.

CREATE TABLE IF NOT EXISTS entity_embeddings (
  entity_kind   TEXT NOT NULL,
  entity_id     TEXT NOT NULL,
  content_text  TEXT NOT NULL,
  content_hash  TEXT NOT NULL,
  model         TEXT NOT NULL,
  dim           INTEGER NOT NULL,
  vector_json   TEXT NOT NULL,
  norm          REAL NOT NULL,
  embedded_at   INTEGER NOT NULL,
  PRIMARY KEY (entity_kind, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_embeddings_kind ON entity_embeddings (entity_kind);

-- Queue of entities whose embeddings are stale (or not yet computed).
-- The opportunistic worker pulls from here in batches.
CREATE TABLE IF NOT EXISTS entity_embeddings_pending (
  entity_kind   TEXT NOT NULL,
  entity_id     TEXT NOT NULL,
  content_text  TEXT NOT NULL,
  content_hash  TEXT NOT NULL,
  enqueued_at   INTEGER NOT NULL,
  attempts      INTEGER NOT NULL DEFAULT 0,
  last_error    TEXT,
  priority      INTEGER NOT NULL DEFAULT 0,   -- higher = sooner
  PRIMARY KEY (entity_kind, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_embeddings_pending_priority
  ON entity_embeddings_pending (priority DESC, enqueued_at ASC);
