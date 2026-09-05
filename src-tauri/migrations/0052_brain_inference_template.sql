-- Section 6.2 — RL feedback surface.
--
-- Tags each `brain_inferences` row with the RL template that generated it
-- (`meeting_action_exact`, `meeting_action_fuzzy`, `email_thread_mention`,
-- `blocker_email_match`, `blocker_fuzzy`) so the inference review queue can
-- (a) filter by template and (b) JOIN against `inference_thresholds` to
-- show the current threshold for each row.
--
-- The column itself is added separately via `ensure_table_column` in db.rs so
-- the migration stays idempotent on re-run. This file contains:
--   1. Backfill of `template` for existing rows, using `(source_kind,
--      relation_kind, target_kind, confidence)` as the disambiguator. The
--      cutoffs match the original seeded thresholds.
--   2. Two indexes — one for the queue's template filter and one for the
--      supersession self-JOIN. Both are gated on the column existing, so
--      we run them *after* the ensure_table_column pass.

-- Backfill: meeting action → deliverable.
--   exact = confidence >= 0.86 (the seeded `meeting_action_exact` threshold),
--   fuzzy = otherwise.
UPDATE brain_inferences
SET template = CASE
    WHEN confidence >= 0.86 THEN 'meeting_action_exact'
    ELSE 'meeting_action_fuzzy'
END
WHERE template IS NULL
  AND source_kind = 'meeting_action'
  AND relation_kind = 'GENERATED'
  AND target_kind  = 'deliverable';

-- Backfill: email thread → deliverable.
UPDATE brain_inferences
SET template = 'email_thread_mention'
WHERE template IS NULL
  AND source_kind = 'email_thread'
  AND relation_kind = 'RELATED_TO'
  AND target_kind  = 'deliverable';

-- Backfill: blocker → stakeholder.
--   email_match = confidence >= 0.88 (the seeded `blocker_email_match` threshold),
--   fuzzy       = otherwise.
UPDATE brain_inferences
SET template = CASE
    WHEN confidence >= 0.88 THEN 'blocker_email_match'
    ELSE 'blocker_fuzzy'
END
WHERE template IS NULL
  AND source_kind = 'blocker'
  AND relation_kind = 'WAITING_ON'
  AND target_kind  = 'stakeholder';

-- Indexes — both partial / composite. Cheap to maintain.
CREATE INDEX IF NOT EXISTS idx_brain_inferences_template
  ON brain_inferences (template, status, updated_at DESC);
