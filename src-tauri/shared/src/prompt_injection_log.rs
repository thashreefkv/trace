//! Section 7 — Prompt injection audit log writer + reader.
//!
//! Records every sanitize-flag, truncate, and destructive-tool confirmation
//! outcome so the user has an audit trail when the model refused to act on
//! adversarial content. Mirrors the shape and conventions of [tool_audit].

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const MAX_EXCERPT_BYTES: usize = 4 * 1024;
const MAX_REASON_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PromptInjectionEntry {
    pub id: String,
    pub ts: i64,
    pub source: String,
    pub origin_kind: Option<String>,
    pub origin_id: Option<String>,
    pub run_id: Option<String>,
    pub call_id: Option<String>,
    pub tool: Option<String>,
    pub action_taken: String,
    pub reason: String,
    pub flags_json: String,
    pub content_excerpt: String,
    pub original_bytes: i64,
    pub sanitized_bytes: i64,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PromptInjectionFilter {
    pub source: Option<String>,
    pub action: Option<String>,
    pub only_with_flags: Option<bool>,
    pub run_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PromptInjectionSnapshot {
    pub entries: Vec<PromptInjectionEntry>,
    pub total_24h: i64,
    pub refusals_24h: i64,
    pub flagged_24h: i64,
}

pub struct RecordInput<'a> {
    pub source: &'a str,
    pub origin_kind: Option<&'a str>,
    pub origin_id: Option<&'a str>,
    pub run_id: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub tool: Option<&'a str>,
    pub action_taken: &'a str,
    pub reason: &'a str,
    pub flags_json: &'a str,
    pub content_excerpt: &'a str,
    pub original_bytes: i64,
    pub sanitized_bytes: i64,
}

/// Insert one audit row. Failures are swallowed (logged to stderr) so they
/// never block the AI flow.
pub async fn record(pool: &SqlitePool, input: RecordInput<'_>) {
    let id = ulid::Ulid::new().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    let excerpt = truncate_for_storage(input.content_excerpt, MAX_EXCERPT_BYTES);
    let reason = truncate_for_storage(input.reason, MAX_REASON_BYTES);

    let result = sqlx::query(
        "INSERT INTO prompt_injection_log
           (id, ts, source, origin_kind, origin_id, run_id, call_id, tool,
            action_taken, reason, flags_json, content_excerpt,
            original_bytes, sanitized_bytes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(ts)
    .bind(input.source)
    .bind(input.origin_kind)
    .bind(input.origin_id)
    .bind(input.run_id)
    .bind(input.call_id)
    .bind(input.tool)
    .bind(input.action_taken)
    .bind(&reason)
    .bind(input.flags_json)
    .bind(&excerpt)
    .bind(input.original_bytes)
    .bind(input.sanitized_bytes)
    .execute(pool)
    .await;

    if let Err(error) = result {
        eprintln!("[prompt_injection_log] failed to record: {error}");
    }
}

pub async fn list(
    pool: &SqlitePool,
    filter: PromptInjectionFilter,
) -> Result<PromptInjectionSnapshot, String> {
    let limit = filter.limit.unwrap_or(100).clamp(1, 500);

    let mut sql = String::from(
        "SELECT id, ts, source, origin_kind, origin_id, run_id, call_id, tool,
                action_taken, reason, flags_json, content_excerpt,
                original_bytes, sanitized_bytes
           FROM prompt_injection_log WHERE 1=1",
    );
    let mut binds: Vec<String> = Vec::new();

    if let Some(source) = &filter.source {
        sql.push_str(" AND source = ?");
        binds.push(source.clone());
    }
    if let Some(action) = &filter.action {
        sql.push_str(" AND action_taken = ?");
        binds.push(action.clone());
    }
    if matches!(filter.only_with_flags, Some(true)) {
        sql.push_str(" AND flags_json != '[]'");
    }
    if let Some(run_id) = &filter.run_id {
        sql.push_str(" AND run_id = ?");
        binds.push(run_id.clone());
    }
    sql.push_str(" ORDER BY ts DESC LIMIT ?");

    let mut query = sqlx::query_as::<_, PromptInjectionRow>(&sql);
    for value in &binds {
        query = query.bind(value);
    }
    query = query.bind(limit);

    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("list prompt injection log: {e}"))?;

    let entries: Vec<PromptInjectionEntry> = rows.into_iter().map(Into::into).collect();

    let cutoff = chrono::Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;
    let (total_24h, refusals_24h, flagged_24h): (i64, i64, i64) = sqlx::query_as(
        "SELECT
            COUNT(*),
            SUM(CASE WHEN action_taken IN ('refused','rejected') THEN 1 ELSE 0 END),
            SUM(CASE WHEN flags_json != '[]' THEN 1 ELSE 0 END)
          FROM prompt_injection_log WHERE ts >= ?",
    )
    .bind(cutoff)
    .fetch_one(pool)
    .await
    .map(|(total, refusals, flagged): (i64, Option<i64>, Option<i64>)| {
        (total, refusals.unwrap_or(0), flagged.unwrap_or(0))
    })
    .unwrap_or((0, 0, 0));

    Ok(PromptInjectionSnapshot {
        entries,
        total_24h,
        refusals_24h,
        flagged_24h,
    })
}

#[derive(sqlx::FromRow)]
struct PromptInjectionRow {
    id: String,
    ts: i64,
    source: String,
    origin_kind: Option<String>,
    origin_id: Option<String>,
    run_id: Option<String>,
    call_id: Option<String>,
    tool: Option<String>,
    action_taken: String,
    reason: String,
    flags_json: String,
    content_excerpt: String,
    original_bytes: i64,
    sanitized_bytes: i64,
}

impl From<PromptInjectionRow> for PromptInjectionEntry {
    fn from(row: PromptInjectionRow) -> Self {
        PromptInjectionEntry {
            id: row.id,
            ts: row.ts,
            source: row.source,
            origin_kind: row.origin_kind,
            origin_id: row.origin_id,
            run_id: row.run_id,
            call_id: row.call_id,
            tool: row.tool,
            action_taken: row.action_taken,
            reason: row.reason,
            flags_json: row.flags_json,
            content_excerpt: row.content_excerpt,
            original_bytes: row.original_bytes,
            sanitized_bytes: row.sanitized_bytes,
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
