-- Vector embeddings + retrieval signals for the memory system.
-- The 'sensitivity' and 'pinned' columns on memories are added
-- idempotently via apply_memory_retrieval_migration in shared/src/db.rs
-- so this file stays safe to run multiple times.

CREATE TABLE IF NOT EXISTS memory_embeddings (
  memory_id   TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
  model       TEXT NOT NULL,
  dim         INTEGER NOT NULL,
  vector_json TEXT NOT NULL,
  norm        REAL NOT NULL,
  fingerprint TEXT NOT NULL,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_embeddings_model
  ON memory_embeddings (model);

CREATE INDEX IF NOT EXISTS idx_memory_embeddings_fingerprint
  ON memory_embeddings (fingerprint);

CREATE TABLE IF NOT EXISTS memory_retrievals (
  id              TEXT PRIMARY KEY,
  query           TEXT NOT NULL,
  memory_ids_json TEXT NOT NULL DEFAULT '[]',
  scores_json     TEXT NOT NULL DEFAULT '{}',
  context_kind    TEXT NOT NULL DEFAULT 'manual',
  source_kind     TEXT,
  source_id       TEXT,
  feedback        TEXT,
  created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_retrievals_created
  ON memory_retrievals (created_at DESC);
