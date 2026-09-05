ALTER TABLE gmail_sync_settings ADD COLUMN relevance_filter_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE gmail_sync_settings ADD COLUMN auto_analyze_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE gmail_sync_settings ADD COLUMN auto_analyze_limit INTEGER NOT NULL DEFAULT 6;
