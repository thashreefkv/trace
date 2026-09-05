ALTER TABLE captures ADD COLUMN promoted_task_id TEXT REFERENCES deliverable_tasks(id);
ALTER TABLE captures ADD COLUMN promoted_task_title TEXT;
