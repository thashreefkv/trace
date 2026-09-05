use sqlx::SqlitePool;

use crate::db::sql_error;
use crate::models::ReasoningRun;

pub async fn list_reasoning_runs(
    pool: &SqlitePool,
    limit: Option<i64>,
) -> Result<Vec<ReasoningRun>, String> {
    sqlx::query_as::<_, ReasoningRun>(
        r#"SELECT id, query_text, depth, query_mode, scope_json, result_markdown,
           citations_json, generated_assertions_json, action_proposals_json, contradictions_json,
           unsupported_json, model, cache_hit, latency_ms, status, created_at, updated_at
           FROM reasoning_runs ORDER BY created_at DESC LIMIT ?"#,
    )
    .bind(limit.unwrap_or(50).clamp(1, 200))
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}
