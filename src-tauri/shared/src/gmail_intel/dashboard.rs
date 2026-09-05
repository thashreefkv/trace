//! Inbox dashboard + retry picker. Extracted from legacy.rs (13-std2).

use serde::Serialize;
use sqlx::SqlitePool;


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct InboxDashboard {
    pub since_ms: i64,
    pub classified_count: i64,
    pub action_required_count: i64,
    pub waiting_on_you_count: i64,
    pub high_priority_count: i64,
    pub failed_classifications: i64,
    pub top_intents: Vec<(String, i64)>,
}

pub async fn inbox_dashboard(pool: &SqlitePool, hours: i64) -> Result<InboxDashboard, String> {
    let since_ms = chrono::Utc::now().timestamp_millis() - hours.max(1) * 60 * 60 * 1000;
    let since_secs = since_ms / 1000;

    let classified_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gmail_threads
          WHERE last_message_at IS NOT NULL AND last_message_at >= ?
            AND ai_triaged_at IS NOT NULL",
    )
    .bind(since_secs)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let action_required_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gmail_threads
          WHERE last_message_at IS NOT NULL AND last_message_at >= ?
            AND action_required = 1",
    )
    .bind(since_secs)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let waiting_on_you_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gmail_threads
          WHERE last_message_at IS NOT NULL AND last_message_at >= ?
            AND thread_state = 'waiting_on_you'",
    )
    .bind(since_secs)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let high_priority_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gmail_threads
          WHERE last_message_at IS NOT NULL AND last_message_at >= ?
            AND effective_priority IN ('high', 'urgent')",
    )
    .bind(since_secs)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let failed_classifications: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gmail_threads
          WHERE last_analysis_error IS NOT NULL AND last_analysis_error != ''",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let top_intents: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(intent, 'unknown'), COUNT(*) FROM gmail_threads
          WHERE last_message_at IS NOT NULL AND last_message_at >= ?
            AND intent IS NOT NULL
          GROUP BY intent ORDER BY COUNT(*) DESC LIMIT 5",
    )
    .bind(since_secs)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    Ok(InboxDashboard {
        since_ms,
        classified_count,
        action_required_count,
        waiting_on_you_count,
        high_priority_count,
        failed_classifications,
        top_intents,
    })
}

// ── Retry queue ────────────────────────────────────────────────────────────

/// Find threads whose last analysis failed; the worker re-runs them with backoff.
pub async fn pick_failed_for_retry(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>(
        "SELECT thread_id FROM gmail_threads
          WHERE last_analysis_error IS NOT NULL AND last_analysis_error != ''
          ORDER BY last_message_at DESC NULLS LAST
          LIMIT ?",
    )
    .bind(limit.max(1))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("pick_failed_for_retry: {e}"))
}

// ── Calibration ────────────────────────────────────────────────────────────

