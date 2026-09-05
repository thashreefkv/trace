-- Files (local + Google Drive) feature
-- Phase 1: local files only; Drive columns reserved for Phase 2.

CREATE TABLE IF NOT EXISTS trace_folders (
  id          TEXT PRIMARY KEY,
  parent_id   TEXT REFERENCES trace_folders(id) ON DELETE CASCADE,
  name        TEXT NOT NULL,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  UNIQUE(parent_id, name)
);
CREATE INDEX IF NOT EXISTS idx_trace_folders_parent ON trace_folders(parent_id);

CREATE TABLE IF NOT EXISTS google_drive_accounts (
  id          TEXT PRIMARY KEY,
  email       TEXT NOT NULL UNIQUE,
  created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS google_drive_settings (
  account_id           TEXT PRIMARY KEY REFERENCES google_drive_accounts(id) ON DELETE CASCADE,
  last_page_token      TEXT,
  last_sync_at         TEXT,
  poll_interval_secs   INTEGER NOT NULL DEFAULT 180,
  max_items_per_run    INTEGER NOT NULL DEFAULT 200,
  initial_import_done  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS files (
  id                    TEXT PRIMARY KEY,
  kind                  TEXT NOT NULL CHECK (kind IN ('local','drive')),
  trace_folder_id       TEXT REFERENCES trace_folders(id) ON DELETE SET NULL,
  name                  TEXT NOT NULL,
  mime_type             TEXT,
  size_bytes            INTEGER,
  description           TEXT,
  local_path            TEXT UNIQUE,
  local_inode           TEXT,
  content_hash          TEXT,
  drive_account_id      TEXT REFERENCES google_drive_accounts(id) ON DELETE CASCADE,
  drive_file_id         TEXT,
  drive_parent_id       TEXT,
  drive_mime            TEXT,
  drive_web_view_link   TEXT,
  drive_modified_time   TEXT,
  drive_trashed         INTEGER NOT NULL DEFAULT 0,
  drive_md5             TEXT,
  created_at            TEXT NOT NULL,
  updated_at            TEXT NOT NULL,
  UNIQUE(drive_account_id, drive_file_id)
);
CREATE INDEX IF NOT EXISTS idx_files_folder       ON files(trace_folder_id);
CREATE INDEX IF NOT EXISTS idx_files_kind         ON files(kind);
CREATE INDEX IF NOT EXISTS idx_files_drive_parent ON files(drive_parent_id);

CREATE TABLE IF NOT EXISTS file_links (
  file_id      TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
  entity_kind  TEXT NOT NULL CHECK (entity_kind IN
    ('initiative','deliverable','deliverable_task','stakeholder','capture','meeting','conversation')),
  entity_id    TEXT NOT NULL,
  linked_at    TEXT NOT NULL,
  source       TEXT NOT NULL DEFAULT 'manual',
  PRIMARY KEY (file_id, entity_kind, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_file_links_target ON file_links(entity_kind, entity_id);

CREATE VIRTUAL TABLE IF NOT EXISTS file_search USING fts5(
  id UNINDEXED,
  name,
  description,
  folder_path,
  link_kinds,
  tokenize='porter'
);

CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
  INSERT INTO file_search(id, name, description, folder_path, link_kinds)
  VALUES (new.id, new.name, COALESCE(new.description,''), '', '');
END;

CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
  DELETE FROM file_search WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS files_au AFTER UPDATE ON files BEGIN
  UPDATE file_search
    SET name = new.name,
        description = COALESCE(new.description,'')
    WHERE id = new.id;
END;
