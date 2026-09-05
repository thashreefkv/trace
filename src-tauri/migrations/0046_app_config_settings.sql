-- AI budget config + alert thresholds. Single-row table per the
-- gmail_sync_settings pattern (id = 1, INSERT OR IGNORE to seed).
--
-- A limit of 0 means "no limit set" — both budget alerts and the optional
-- block-when-exceeded enforcement stay off until the user opts in via
-- Settings → AI Budget.

CREATE TABLE IF NOT EXISTS app_config_settings (
    id                              INTEGER PRIMARY KEY CHECK (id = 1),
    budget_daily_usd                REAL NOT NULL DEFAULT 0,
    budget_monthly_usd              REAL NOT NULL DEFAULT 0,
    budget_alert_threshold_pct      REAL NOT NULL DEFAULT 80,
    budget_block_when_exceeded      INTEGER NOT NULL DEFAULT 0,
    created_at                      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at                      TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO app_config_settings (id) VALUES (1);
