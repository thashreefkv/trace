-- Trace-native reasoning GraphRAG and strategic report persistence.

CREATE TABLE IF NOT EXISTS reasoning_source_units (
  id                TEXT PRIMARY KEY,
  source_kind       TEXT NOT NULL,
  source_id         TEXT NOT NULL,
  chunk_index       INTEGER NOT NULL DEFAULT 0,
  title             TEXT NOT NULL DEFAULT '',
  body              TEXT NOT NULL DEFAULT '',
  citation_label    TEXT NOT NULL DEFAULT '',
  source_route      TEXT,
  source_updated_at TEXT,
  content_hash      TEXT NOT NULL,
  access_state      TEXT NOT NULL DEFAULT 'included'
                    CHECK (access_state IN ('included','excluded','deleted')),
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL,
  UNIQUE (source_kind, source_id, chunk_index)
);
CREATE INDEX IF NOT EXISTS idx_reasoning_source_units_kind
  ON reasoning_source_units (source_kind, source_updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_reasoning_source_units_hash
  ON reasoning_source_units (content_hash);

CREATE TABLE IF NOT EXISTS claim_versions (
  id                  TEXT PRIMARY KEY,
  claim_key           TEXT NOT NULL,
  statement           TEXT NOT NULL,
  source_kind         TEXT,
  source_id           TEXT,
  temporal_scope      TEXT,
  confidence          REAL NOT NULL DEFAULT 0.5,
  contradiction_state TEXT NOT NULL DEFAULT 'none'
                      CHECK (contradiction_state IN ('none','possible','confirmed','resolved')),
  evidence_json       TEXT NOT NULL DEFAULT '[]',
  status              TEXT NOT NULL DEFAULT 'pending'
                      CHECK (status IN ('pending','approved','rejected')),
  generated_by        TEXT NOT NULL DEFAULT 'gemini',
  reasoning_run_id    TEXT,
  created_at          TEXT NOT NULL,
  updated_at          TEXT NOT NULL,
  reviewed_at         TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_claim_versions_key_status
  ON claim_versions (claim_key, status);
CREATE INDEX IF NOT EXISTS idx_claim_versions_review
  ON claim_versions (status, updated_at DESC);

CREATE TABLE IF NOT EXISTS graph_communities (
  id              TEXT PRIMARY KEY,
  community_kind  TEXT NOT NULL,
  scope_key       TEXT NOT NULL,
  title           TEXT NOT NULL,
  level           INTEGER NOT NULL DEFAULT 0,
  version_hash    TEXT NOT NULL DEFAULT '',
  status          TEXT NOT NULL DEFAULT 'active'
                  CHECK (status IN ('active','stale','archived')),
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  UNIQUE (community_kind, scope_key)
);
CREATE TABLE IF NOT EXISTS graph_community_members (
  community_id TEXT NOT NULL REFERENCES graph_communities(id) ON DELETE CASCADE,
  entity_kind  TEXT NOT NULL,
  entity_id    TEXT NOT NULL,
  source       TEXT NOT NULL DEFAULT 'canonical',
  added_at     TEXT NOT NULL,
  PRIMARY KEY (community_id, entity_kind, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_graph_community_members_entity
  ON graph_community_members (entity_kind, entity_id);

CREATE TABLE IF NOT EXISTS community_reports (
  id               TEXT PRIMARY KEY,
  community_id     TEXT NOT NULL REFERENCES graph_communities(id) ON DELETE CASCADE,
  title            TEXT NOT NULL,
  summary_markdown TEXT NOT NULL,
  evidence_json    TEXT NOT NULL DEFAULT '[]',
  version_hash     TEXT NOT NULL DEFAULT '',
  status           TEXT NOT NULL DEFAULT 'pending'
                   CHECK (status IN ('pending','approved','rejected','stale')),
  model            TEXT NOT NULL,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL,
  reviewed_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_community_reports_status
  ON community_reports (status, updated_at DESC);

CREATE TABLE IF NOT EXISTS reasoning_runs (
  id                     TEXT PRIMARY KEY,
  query_text             TEXT NOT NULL,
  depth                  TEXT NOT NULL DEFAULT 'deep',
  query_mode             TEXT NOT NULL,
  scope_json             TEXT NOT NULL DEFAULT '{}',
  result_markdown        TEXT NOT NULL DEFAULT '',
  citations_json         TEXT NOT NULL DEFAULT '[]',
  generated_assertions_json TEXT NOT NULL DEFAULT '[]',
  action_proposals_json  TEXT NOT NULL DEFAULT '[]',
  contradictions_json    TEXT NOT NULL DEFAULT '[]',
  unsupported_json       TEXT NOT NULL DEFAULT '[]',
  model                  TEXT NOT NULL,
  cache_hit              INTEGER NOT NULL DEFAULT 0,
  latency_ms             INTEGER NOT NULL DEFAULT 0,
  status                 TEXT NOT NULL DEFAULT 'completed',
  created_at             TEXT NOT NULL,
  updated_at             TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reasoning_runs_created
  ON reasoning_runs (created_at DESC);
CREATE TABLE IF NOT EXISTS reasoning_cache (
  cache_key          TEXT PRIMARY KEY,
  model              TEXT NOT NULL,
  prompt_role        TEXT NOT NULL,
  query_mode         TEXT NOT NULL,
  source_fingerprint TEXT NOT NULL,
  synthesis_json     TEXT NOT NULL,
  created_at         TEXT NOT NULL,
  last_used_at       TEXT NOT NULL,
  hit_count          INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS reasoning_run_sources (
  reasoning_run_id TEXT NOT NULL REFERENCES reasoning_runs(id) ON DELETE CASCADE,
  source_unit_id   TEXT NOT NULL REFERENCES reasoning_source_units(id) ON DELETE CASCADE,
  relevance_score REAL NOT NULL DEFAULT 0,
  citation_label  TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (reasoning_run_id, source_unit_id)
);

CREATE TABLE IF NOT EXISTS agent_review_items (
  id                   TEXT PRIMARY KEY,
  source_kind          TEXT NOT NULL,
  source_id            TEXT NOT NULL,
  proposal_type        TEXT NOT NULL,
  proposed_change_json TEXT NOT NULL DEFAULT '{}',
  rationale            TEXT NOT NULL DEFAULT '',
  status               TEXT NOT NULL DEFAULT 'pending'
                       CHECK (status IN ('pending','approved','rejected','applied')),
  created_at           TEXT NOT NULL,
  resolved_at          TEXT
);
CREATE INDEX IF NOT EXISTS idx_agent_review_items_status
  ON agent_review_items (status, created_at DESC);

CREATE TABLE IF NOT EXISTS entity_events (
  id          TEXT PRIMARY KEY,
  entity_kind TEXT NOT NULL,
  entity_id   TEXT NOT NULL,
  event_kind  TEXT NOT NULL,
  actor       TEXT NOT NULL DEFAULT 'system',
  detail_json TEXT NOT NULL DEFAULT '{}',
  created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_entity_events_entity
  ON entity_events (entity_kind, entity_id, created_at DESC);

CREATE TABLE IF NOT EXISTS report_templates (
  report_type       TEXT PRIMARY KEY,
  title             TEXT NOT NULL,
  description       TEXT NOT NULL,
  sections_json     TEXT NOT NULL DEFAULT '[]',
  default_query_mode TEXT NOT NULL,
  updated_at        TEXT NOT NULL
);
INSERT OR IGNORE INTO report_templates
  (report_type, title, description, sections_json, default_query_mode, updated_at)
VALUES
  ('quarterly', 'Quarterly / Q3 Report', 'Strategic period synthesis with themes, movement, risk, and recommendations.',
   '["Executive summary","Outcomes and movement","Themes and evidence","Risks and decisions","Next-quarter recommendations"]', 'global', datetime('now')),
  ('initiative', 'Initiative Report', 'Source-grounded account of an initiative and its strategic implications.',
   '["Current framing","Delivered outcomes","In-flight work","Evidence and signals","Risks","Recommended next move"]', 'drift', datetime('now')),
  ('decision_memo', 'Decision Memo', 'Recommendation with alternatives, evidence, risk, and next action.',
   '["Decision","Recommendation","Why now","Evidence","Alternatives","Risks","Open questions","Next action"]', 'drift', datetime('now'));

CREATE TABLE IF NOT EXISTS report_runs (
  id               TEXT PRIMARY KEY,
  report_type      TEXT NOT NULL CHECK (report_type IN ('quarterly','initiative','decision_memo')),
  title            TEXT NOT NULL,
  status           TEXT NOT NULL DEFAULT 'created'
                   CHECK (status IN ('created','outlined','outline_approved','drafted','approved','discarded')),
  scope_json       TEXT NOT NULL DEFAULT '{}',
  outline_markdown TEXT NOT NULL DEFAULT '',
  draft_markdown   TEXT NOT NULL DEFAULT '',
  citations_json   TEXT NOT NULL DEFAULT '[]',
  unsupported_json TEXT NOT NULL DEFAULT '[]',
  reasoning_run_id TEXT REFERENCES reasoning_runs(id) ON DELETE SET NULL,
  deliverable_id   TEXT REFERENCES deliverables(id) ON DELETE SET NULL,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL,
  approved_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_report_runs_updated ON report_runs (updated_at DESC);

CREATE TABLE IF NOT EXISTS report_sources (
  report_run_id  TEXT NOT NULL REFERENCES report_runs(id) ON DELETE CASCADE,
  source_unit_id TEXT NOT NULL REFERENCES reasoning_source_units(id) ON DELETE CASCADE,
  citation_label TEXT NOT NULL DEFAULT '',
  used_in_sections_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (report_run_id, source_unit_id)
);
CREATE TABLE IF NOT EXISTS report_exports (
  id             TEXT PRIMARY KEY,
  report_run_id  TEXT NOT NULL REFERENCES report_runs(id) ON DELETE CASCADE,
  format         TEXT NOT NULL CHECK (format IN ('markdown','docx','pdf','google_docs')),
  export_path    TEXT,
  artifact_url   TEXT,
  created_at     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_report_exports_report
  ON report_exports (report_run_id, created_at DESC);
