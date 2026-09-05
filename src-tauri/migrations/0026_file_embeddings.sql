CREATE TABLE IF NOT EXISTS file_embeddings (
  file_id      TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  chunk_index  INTEGER NOT NULL,
  chunk_text   TEXT NOT NULL,
  model        TEXT NOT NULL,
  dim          INTEGER NOT NULL,
  vector_json  TEXT NOT NULL,
  norm         REAL NOT NULL,
  fingerprint  TEXT NOT NULL,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL,
  PRIMARY KEY (file_id, chunk_index)
);
CREATE INDEX IF NOT EXISTS idx_file_embeddings_file ON file_embeddings(file_id);
