-- Folder-entity links: a Trace folder can be linked to any entity

CREATE TABLE IF NOT EXISTS folder_entity_links (
  folder_id    TEXT NOT NULL REFERENCES trace_folders(id) ON DELETE CASCADE,
  entity_kind  TEXT NOT NULL CHECK (entity_kind IN
    ('initiative','deliverable','deliverable_task','stakeholder','capture','meeting','conversation')),
  entity_id    TEXT NOT NULL,
  linked_at    TEXT NOT NULL,
  PRIMARY KEY (folder_id, entity_kind, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_folder_entity_links_target
  ON folder_entity_links(entity_kind, entity_id);
