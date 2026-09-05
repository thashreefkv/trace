-- Add is_missing flag so we can keep local_path for recovery while marking the file absent.
ALTER TABLE files ADD COLUMN is_missing INTEGER NOT NULL DEFAULT 0;
-- Mark any existing rows whose local_path is NULL and kind is local as missing.
UPDATE files SET is_missing = 1 WHERE kind = 'local' AND local_path IS NULL;
