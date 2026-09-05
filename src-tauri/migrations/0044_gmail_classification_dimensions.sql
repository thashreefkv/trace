-- Multi-dimensional email classification.
-- The single ai_category + ai_priority pair is too coarse. We add explicit
-- dimensions so the user (and the brain) can reason about emails the way a
-- human triages: what's the sender asking for, do I need to do anything,
-- and who's holding the next ball.

ALTER TABLE gmail_threads ADD COLUMN intent TEXT;
-- 'asking' | 'informing' | 'requesting_decision' | 'scheduling' | 'acknowledging' | 'venting' | 'other'

ALTER TABLE gmail_threads ADD COLUMN action_required INTEGER NOT NULL DEFAULT 0;

ALTER TABLE gmail_threads ADD COLUMN predicted_action TEXT;
-- 'reply' | 'schedule' | 'file' | 'ignore' | 'other'

ALTER TABLE gmail_threads ADD COLUMN thread_state TEXT;
-- 'waiting_on_you' | 'waiting_on_them' | 'resolved' | 'dormant'

ALTER TABLE gmail_threads ADD COLUMN dimensions_confidence_json TEXT NOT NULL DEFAULT '{}';
-- per-dimension confidence: {"category":0.9,"priority":0.7,"intent":0.85,...}

ALTER TABLE gmail_threads ADD COLUMN bundle_id TEXT;
-- groups reply chains + same-subject threads within 24h for collapsing in the inbox

CREATE INDEX IF NOT EXISTS idx_gmail_threads_intent ON gmail_threads (intent);
CREATE INDEX IF NOT EXISTS idx_gmail_threads_thread_state ON gmail_threads (thread_state);
CREATE INDEX IF NOT EXISTS idx_gmail_threads_bundle ON gmail_threads (bundle_id);
CREATE INDEX IF NOT EXISTS idx_gmail_threads_action_required
  ON gmail_threads (action_required, last_message_at DESC);
