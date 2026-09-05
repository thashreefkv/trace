//! Weekly digest rollup (overdue / awaiting / recently shipped) and the
//! load-threads-by-ids helper it uses. Extracted from legacy.rs (13-G17).

use std::collections::BTreeSet;

use sqlx::SqlitePool;

use super::models::*;
use super::{get_local_thread, now_utc};

pub async fn weekly_digest(pool: &SqlitePool) -> Result<GmailWeeklyDigest, String> {
    let overdue_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT f.thread_id
        FROM gmail_followups f
        INNER JOIN gmail_threads t ON t.thread_id = f.thread_id
        WHERE f.status = 'open'
          AND f.due_at <= ?
        ORDER BY
          CASE t.effective_priority
            WHEN 'urgent' THEN 0
            WHEN 'high' THEN 1
            WHEN 'medium' THEN 2
            ELSE 3
          END,
          f.due_at ASC
        LIMIT 12
        "#,
    )
    .bind(now_utc())
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    let waiting_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT f.thread_id
        FROM gmail_followups f
        INNER JOIN gmail_threads t ON t.thread_id = f.thread_id
        WHERE f.status = 'open'
        ORDER BY
          CASE t.effective_priority
            WHEN 'urgent' THEN 0
            WHEN 'high' THEN 1
            WHEN 'medium' THEN 2
            ELSE 3
          END,
          f.due_at ASC
        LIMIT 12
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    let urgent_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT thread_id
        FROM gmail_threads
        WHERE effective_priority IN ('urgent', 'high')
        ORDER BY
          CASE effective_priority WHEN 'urgent' THEN 0 ELSE 1 END,
          COALESCE(last_message_at, first_message_at, 0) DESC
        LIMIT 8
        "#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let draft_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gmail_drafts")
        .fetch_one(pool)
        .await
        .map_err(crate::db::sql_error)?;

    let waiting_for_response = load_threads_by_ids(pool, waiting_ids).await?;
    let overdue_followups = load_threads_by_ids(pool, overdue_ids).await?;
    let urgent = load_threads_by_ids(pool, urgent_ids).await.unwrap_or_default();
    let summary = format!(
        "{} thread(s) waiting for response, {} overdue follow-up(s), {} draft(s) unsent.",
        waiting_for_response.len(),
        overdue_followups.len(),
        draft_count
    );

    Ok(GmailWeeklyDigest {
        summary,
        waiting_for_response,
        overdue_followups,
        urgent_threads: urgent,
        draft_count,
    })
}

async fn load_threads_by_ids(
    pool: &SqlitePool,
    thread_ids: Vec<String>,
) -> Result<Vec<GmailLocalThread>, String> {
    let mut threads = Vec::new();
    let mut seen = BTreeSet::new();
    for thread_id in thread_ids {
        if !seen.insert(thread_id.clone()) {
            continue;
        }
        if let Ok(detail) = get_local_thread(pool, &thread_id).await {
            threads.push(detail.thread);
        }
    }
    Ok(threads)
}




