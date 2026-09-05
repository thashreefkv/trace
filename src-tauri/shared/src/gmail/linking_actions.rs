//! Manual link mutations and thread->entity creation: link/unlink
//! deliverables and initiatives, suggest threads for a deliverable, and
//! create captures/tasks from a thread. Extracted from legacy.rs (13-G15).

use sqlx::SqlitePool;

use super::models::*;
use super::{get_local_thread, list_local_threads, now_utc, refresh_thread_intelligence};

pub async fn link_thread_to_deliverable(
    pool: &SqlitePool,
    thread_id: &str,
    deliverable_id: &str,
) -> Result<(), String> {
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT INTO gmail_thread_deliverables (thread_id, deliverable_id, linked_at, source)
        VALUES (?, ?, ?, 'manual')
        ON CONFLICT(thread_id, deliverable_id) DO UPDATE SET
          linked_at = excluded.linked_at,
          source = 'manual',
          confidence = NULL,
          rationale = ''
        "#,
    )
    .bind(thread_id)
    .bind(deliverable_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    refresh_thread_intelligence(pool, thread_id).await?;
    Ok(())
}

pub async fn unlink_thread_from_deliverable(
    pool: &SqlitePool,
    thread_id: &str,
    deliverable_id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM gmail_thread_deliverables WHERE thread_id = ? AND deliverable_id = ?")
        .bind(thread_id)
        .bind(deliverable_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    refresh_thread_intelligence(pool, thread_id).await?;
    Ok(())
}

pub async fn link_thread_to_initiative(
    pool: &SqlitePool,
    thread_id: &str,
    initiative_id: &str,
) -> Result<(), String> {
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT INTO gmail_thread_initiatives (thread_id, initiative_id, linked_at, source)
        VALUES (?, ?, ?, 'manual')
        ON CONFLICT(thread_id, initiative_id) DO UPDATE SET
          linked_at = excluded.linked_at,
          source = 'manual',
          confidence = NULL,
          rationale = ''
        "#,
    )
    .bind(thread_id)
    .bind(initiative_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    refresh_thread_intelligence(pool, thread_id).await?;
    Ok(())
}

pub async fn suggest_threads_for_deliverable(
    pool: &SqlitePool,
    deliverable_id: &str,
    limit: i64,
) -> Result<Vec<GmailLocalThread>, String> {
    let deliverable = crate::repo::get_deliverable(pool, deliverable_id).await?;
    let query = if deliverable.claim.trim().is_empty() {
        deliverable.title
    } else {
        format!("{} {}", deliverable.title, deliverable.claim)
    };
    list_local_threads(
        pool,
        GmailThreadFilter {
            query: Some(query),
            limit: Some(limit),
            ..GmailThreadFilter::default()
        },
    )
    .await
}

pub async fn create_capture_from_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<crate::models::Capture, String> {
    let detail = get_local_thread(pool, thread_id).await?;
    let body = format!(
        "Email thread: {}\n\n{}\n\ntrace://gmail/thread/{}",
        detail.thread.subject, detail.thread.snippet, detail.thread.thread_id
    );
    let capture = crate::repo::create_capture(
        pool,
        crate::models::CreateCaptureInput {
            kind: crate::models::CaptureKind::Thought,
            body,
        },
    )
    .await?;
    sqlx::query(
        "INSERT OR IGNORE INTO gmail_thread_captures (thread_id, capture_id, linked_at) VALUES (?, ?, ?)",
    )
    .bind(thread_id)
    .bind(&capture.id)
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(capture)
}

pub async fn create_task_from_thread(
    pool: &SqlitePool,
    thread_id: &str,
    deliverable_id: &str,
    title: &str,
    due_date: Option<String>,
) -> Result<crate::models::DeliverableTask, String> {
    let detail = get_local_thread(pool, thread_id).await?;
    let task_title = if title.trim().is_empty() {
        format!("Follow up on email: {}", detail.thread.subject)
    } else {
        title.trim().to_string()
    };
    let task = crate::repo::create_deliverable_task(
        pool,
        crate::models::CreateTaskInput {
            deliverable_id: deliverable_id.to_string(),
            title: task_title,
            due_date,
            notes: None,
            url: None,
        },
    )
    .await?;
    let _ = link_thread_to_deliverable(pool, thread_id, deliverable_id).await;
    Ok(task)
}

