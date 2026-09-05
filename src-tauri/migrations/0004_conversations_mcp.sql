CREATE TABLE IF NOT EXISTS conversations (
  id          TEXT PRIMARY KEY,
  chat_url    TEXT NOT NULL UNIQUE,
  title       TEXT,
  summary     TEXT,
  occurred_at TEXT,
  ingested_at TEXT NOT NULL
);

ALTER TABLE deliverables
  ADD COLUMN conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_conversations_chat_url
  ON conversations (chat_url);

CREATE INDEX IF NOT EXISTS idx_deliverables_conversation
  ON deliverables (conversation_id);
