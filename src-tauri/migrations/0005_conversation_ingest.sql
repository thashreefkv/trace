ALTER TABLE captures
  ADD COLUMN promoted_conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_captures_promoted_conversation
  ON captures (promoted_conversation_id);

CREATE INDEX IF NOT EXISTS idx_conversations_ingested_at
  ON conversations (ingested_at DESC);
