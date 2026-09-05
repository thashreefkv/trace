-- GMeet transcript folder per Drive account
ALTER TABLE google_drive_settings ADD COLUMN gmeet_folder_id TEXT;
ALTER TABLE google_drive_settings ADD COLUMN gmeet_folder_name TEXT;
