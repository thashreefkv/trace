-- Audit log for every tool call made by the AI (Ask agent + MCP server).
-- Powers the "Tool calls" debug panel in Settings and inline Ask traces.

CREATE TABLE IF NOT EXISTS tool_call_log (
  id              TEXT PRIMARY KEY,
  ts              INTEGER NOT NULL,
  source          TEXT NOT NULL,       -- 'ask' | 'mcp' | other
  run_id          TEXT,                -- present for ask turns
  call_id         TEXT,                -- per-call id within a run
  tool            TEXT NOT NULL,
  args_json       TEXT NOT NULL,
  result_summary  TEXT,                -- short, UI-friendly summary
  result_json     TEXT,                -- full result (capped server-side)
  ok              INTEGER NOT NULL,    -- 1 / 0
  error           TEXT,
  latency_ms      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_tool_call_log_ts ON tool_call_log (ts DESC);
CREATE INDEX IF NOT EXISTS idx_tool_call_log_tool ON tool_call_log (tool, ts DESC);
CREATE INDEX IF NOT EXISTS idx_tool_call_log_run ON tool_call_log (run_id, ts);
CREATE INDEX IF NOT EXISTS idx_tool_call_log_ok ON tool_call_log (ok, ts DESC);
