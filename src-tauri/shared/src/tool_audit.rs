//! Tool call audit log.
//!
//! Every tool invocation made by the AI — both via the Ask agent
//! (`gemini::dispatch_tool`) and via the standalone MCP server — is written
//! here so the user can review what the model did, debug failures, and replay
//! sequences. Powers the "Tool calls" panel in Settings and per-turn inline
//! traces in Ask.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const MAX_ARGS_BYTES: usize = 16 * 1024;
const MAX_RESULT_BYTES: usize = 32 * 1024;
const MAX_ERROR_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ToolCallLogEntry {
    pub id: String,
    pub ts: i64,
    pub source: String,
    pub run_id: Option<String>,
    pub call_id: Option<String>,
    pub tool: String,
    pub args_json: String,
    pub result_summary: Option<String>,
    pub result_json: Option<String>,
    pub ok: bool,
    pub error: Option<String>,
    pub latency_ms: i64,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ToolCallLogFilter {
    pub source: Option<String>,
    pub tool: Option<String>,
    pub only_errors: Option<bool>,
    pub run_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ToolCallLogSnapshot {
    pub entries: Vec<ToolCallLogEntry>,
    pub total_calls_24h: i64,
    pub error_calls_24h: i64,
}

pub struct RecordInput<'a> {
    pub source: &'a str,
    pub run_id: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub tool: &'a str,
    pub args_json: &'a str,
    pub result_summary: Option<&'a str>,
    pub result_json: Option<&'a str>,
    pub ok: bool,
    pub error: Option<&'a str>,
    pub latency_ms: i64,
}

/// Insert one audit row. Failures are swallowed (logged to stderr) so they
/// never block the AI flow.
pub async fn record(pool: &SqlitePool, input: RecordInput<'_>) {
    let id = ulid::Ulid::new().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    let args = truncate_for_storage(input.args_json, MAX_ARGS_BYTES);
    let result_json = input
        .result_json
        .map(|s| truncate_for_storage(s, MAX_RESULT_BYTES));
    let error = input
        .error
        .map(|s| truncate_for_storage(s, MAX_ERROR_BYTES));

    let result = sqlx::query(
        "INSERT INTO tool_call_log
           (id, ts, source, run_id, call_id, tool, args_json,
            result_summary, result_json, ok, error, latency_ms)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(ts)
    .bind(input.source)
    .bind(input.run_id)
    .bind(input.call_id)
    .bind(input.tool)
    .bind(&args)
    .bind(input.result_summary)
    .bind(result_json.as_deref())
    .bind(if input.ok { 1_i64 } else { 0_i64 })
    .bind(error.as_deref())
    .bind(input.latency_ms)
    .execute(pool)
    .await;

    if let Err(error) = result {
        eprintln!("[tool_audit] failed to record: {error}");
    }
}

pub async fn list(
    pool: &SqlitePool,
    filter: ToolCallLogFilter,
) -> Result<ToolCallLogSnapshot, String> {
    let limit = filter.limit.unwrap_or(100).clamp(1, 500);

    let mut sql = String::from(
        "SELECT id, ts, source, run_id, call_id, tool, args_json,
                result_summary, result_json, ok, error, latency_ms
           FROM tool_call_log WHERE 1=1",
    );
    let mut binds: Vec<String> = Vec::new();

    if let Some(source) = &filter.source {
        sql.push_str(" AND source = ?");
        binds.push(source.clone());
    }
    if let Some(tool) = &filter.tool {
        sql.push_str(" AND tool = ?");
        binds.push(tool.clone());
    }
    if matches!(filter.only_errors, Some(true)) {
        sql.push_str(" AND ok = 0");
    }
    if let Some(run_id) = &filter.run_id {
        sql.push_str(" AND run_id = ?");
        binds.push(run_id.clone());
    }
    sql.push_str(" ORDER BY ts DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, ToolCallLogRow>(&sql);
    for value in &binds {
        query = query.bind(value);
    }
    query = query.bind(limit);

    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("list tool calls: {e}"))?;

    let entries: Vec<ToolCallLogEntry> = rows.into_iter().map(Into::into).collect();

    let cutoff = chrono::Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;
    let (total_calls_24h, error_calls_24h): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), SUM(CASE WHEN ok = 0 THEN 1 ELSE 0 END)
           FROM tool_call_log WHERE ts >= ?",
    )
    .bind(cutoff)
    .fetch_one(pool)
    .await
    .map(|(total, errors): (i64, Option<i64>)| (total, errors.unwrap_or(0)))
    .unwrap_or((0, 0));

    Ok(ToolCallLogSnapshot {
        entries,
        total_calls_24h,
        error_calls_24h,
    })
}

#[derive(sqlx::FromRow)]
struct ToolCallLogRow {
    id: String,
    ts: i64,
    source: String,
    run_id: Option<String>,
    call_id: Option<String>,
    tool: String,
    args_json: String,
    result_summary: Option<String>,
    result_json: Option<String>,
    ok: i64,
    error: Option<String>,
    latency_ms: i64,
}

impl From<ToolCallLogRow> for ToolCallLogEntry {
    fn from(row: ToolCallLogRow) -> Self {
        ToolCallLogEntry {
            id: row.id,
            ts: row.ts,
            source: row.source,
            run_id: row.run_id,
            call_id: row.call_id,
            tool: row.tool,
            args_json: row.args_json,
            result_summary: row.result_summary,
            result_json: row.result_json,
            ok: row.ok != 0,
            error: row.error,
            latency_ms: row.latency_ms,
        }
    }
}

fn truncate_for_storage(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = String::with_capacity(end + 32);
    out.push_str(&value[..end]);
    out.push_str("…[truncated]");
    out
}
