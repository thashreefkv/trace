-- Section 2 — retrieval + inference RL.
--
-- `inference_thresholds`: learned per-template confidence thresholds.
-- Seeded with the values previously hardcoded in
-- `refresh_brain_inferences` (0.86 / 0.72 / 0.64 / 0.88 / 0.74).
-- A periodic background recompute shifts each threshold toward the
-- candidate value that hits the target precision.
--
-- The `brain_inferences.superseded_by` / `supersede_reason` columns are
-- added separately via `ensure_table_column` in db.rs so the migration
-- stays idempotent on re-run.

CREATE TABLE IF NOT EXISTS inference_thresholds (
    template        TEXT PRIMARY KEY,
    threshold       REAL NOT NULL,
    sample_count    INTEGER NOT NULL DEFAULT 0,
    last_recomputed TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO inference_thresholds (template, threshold)
VALUES
    ('meeting_action_exact', 0.86),
    ('meeting_action_fuzzy', 0.72),
    ('email_thread_mention', 0.64),
    ('blocker_email_match',  0.88),
    ('blocker_fuzzy',        0.74);
