-- User-correctable email classification.
--
-- The LLM populates `gmail_threads.ai_category` / `ai_priority`. These two
-- tables let the user override individual threads or define deterministic
-- sender-based rules. Effective category resolution:
--   per-thread override (gmail_user_classifications)
--     ↳ sender rule match (gmail_sender_rules)
--       ↳ LLM-set ai_category / ai_priority
--
-- All overrides feed `brain_rl_events` so the RL system learns from corrections.

CREATE TABLE IF NOT EXISTS gmail_user_classifications (
  thread_id     TEXT PRIMARY KEY,
  category      TEXT,                  -- override; NULL = no override on category
  priority      TEXT,                  -- override; NULL = no override on priority
  note          TEXT,
  source        TEXT NOT NULL DEFAULT 'manual', -- 'manual' | 'sender_rule'
  rule_id       TEXT,                  -- if applied from a sender rule
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gmail_user_classifications_category
  ON gmail_user_classifications (category);

CREATE TABLE IF NOT EXISTS gmail_sender_rules (
  id            TEXT PRIMARY KEY,
  pattern       TEXT NOT NULL,         -- email or pattern: 'foo@bar.com' or '*@bar.com' or 'newsletter@*'
  pattern_kind  TEXT NOT NULL DEFAULT 'glob',   -- 'exact' | 'glob' | 'domain'
  category      TEXT,                  -- desired category (NULL = no change)
  priority      TEXT,                  -- desired priority (NULL = no change)
  note          TEXT,
  enabled       INTEGER NOT NULL DEFAULT 1,
  applied_count INTEGER NOT NULL DEFAULT 0,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gmail_sender_rules_enabled
  ON gmail_sender_rules (enabled, pattern);
