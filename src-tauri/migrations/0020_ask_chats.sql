-- Server-side persistence for the Ask agent: chats, branched turns, attachments,
-- and an FTS5 index for cross-chat search.

CREATE TABLE IF NOT EXISTS ask_chats (
  id           TEXT PRIMARY KEY,
  title        TEXT NOT NULL,
  mode         TEXT NOT NULL,
  summary      TEXT NOT NULL DEFAULT '',
  archived_at  TEXT,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ask_chats_updated
  ON ask_chats (updated_at DESC);

CREATE TABLE IF NOT EXISTS ask_turns (
  id              TEXT PRIMARY KEY,
  chat_id         TEXT NOT NULL REFERENCES ask_chats(id) ON DELETE CASCADE,
  parent_id       TEXT REFERENCES ask_turns(id) ON DELETE CASCADE,
  fork_of         TEXT,
  mode            TEXT,
  question        TEXT NOT NULL,
  answer          TEXT NOT NULL DEFAULT '',
  reasoning       TEXT NOT NULL DEFAULT '',
  status          TEXT NOT NULL,
  error           TEXT,
  refs_json       TEXT NOT NULL DEFAULT '[]',
  questions_json  TEXT NOT NULL DEFAULT '[]',
  steps_json      TEXT NOT NULL DEFAULT '[]',
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ask_turns_chat
  ON ask_turns (chat_id, created_at);

CREATE INDEX IF NOT EXISTS idx_ask_turns_parent
  ON ask_turns (parent_id);

CREATE TABLE IF NOT EXISTS ask_turn_attachments (
  id          TEXT PRIMARY KEY,
  turn_id     TEXT NOT NULL REFERENCES ask_turns(id) ON DELETE CASCADE,
  mime_type   TEXT NOT NULL,
  filename    TEXT,
  data_b64    TEXT NOT NULL,
  size_bytes  INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ask_turn_attachments_turn
  ON ask_turn_attachments (turn_id);

CREATE VIRTUAL TABLE IF NOT EXISTS ask_chat_search USING fts5(
  chat_id UNINDEXED,
  turn_id UNINDEXED,
  question,
  answer,
  content=''
);

CREATE TRIGGER IF NOT EXISTS ask_turns_search_ai
AFTER INSERT ON ask_turns
BEGIN
  INSERT INTO ask_chat_search(rowid, chat_id, turn_id, question, answer)
  VALUES (new.rowid, new.chat_id, new.id, new.question, new.answer);
END;

CREATE TRIGGER IF NOT EXISTS ask_turns_search_au
AFTER UPDATE OF question, answer ON ask_turns
BEGIN
  INSERT INTO ask_chat_search(ask_chat_search, rowid, chat_id, turn_id, question, answer)
  VALUES ('delete', old.rowid, old.chat_id, old.id, old.question, old.answer);
  INSERT INTO ask_chat_search(rowid, chat_id, turn_id, question, answer)
  VALUES (new.rowid, new.chat_id, new.id, new.question, new.answer);
END;

CREATE TRIGGER IF NOT EXISTS ask_turns_search_ad
AFTER DELETE ON ask_turns
BEGIN
  INSERT INTO ask_chat_search(ask_chat_search, rowid, chat_id, turn_id, question, answer)
  VALUES ('delete', old.rowid, old.chat_id, old.id, old.question, old.answer);
END;
