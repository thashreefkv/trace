use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{
        Capture, CaptureFilters, CaptureKind, CaptureStatus, CreateCaptureInput,
        CreateDeliverableInput, CreateInitiativeInput, Deliverable, Initiative,
    },
};

use super::{
    create_deliverable_task, ensure_references_exist, get_deliverable, get_initiative,
    has_ascii_whitespace, insert_deliverable_in_tx, now_utc, replace_initiative_links,
    replace_stakeholder_links, shipped_at_for_state, validate_deliverable_input,
    validate_initiative_input,
};

pub async fn create_capture(
    pool: &SqlitePool,
    input: CreateCaptureInput,
) -> Result<Capture, String> {
    let body = validate_capture_input(input.kind, input.body)?;
    let id = Ulid::new().to_string();
    let now = now_utc();

    sqlx::query(
        r#"
        INSERT INTO captures (id, kind, body, status, created_at, updated_at)
        VALUES (?, ?, ?, 'inbox', ?, ?)
        "#,
    )
    .bind(&id)
    .bind(input.kind.as_str())
    .bind(body)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_capture(pool, &id).await
}

pub async fn list_captures(
    pool: &SqlitePool,
    filters: CaptureFilters,
) -> Result<Vec<Capture>, String> {
    let status_filter = filters.status.map(|value| value.as_str().to_string());
    let kind_filter = filters.kind.map(|value| value.as_str().to_string());

    sqlx::query_as::<_, Capture>(
        r#"
        SELECT
          c.id,
          c.kind,
          c.body,
          c.status,
          c.promoted_deliverable_id,
          d.title AS promoted_deliverable_title,
          c.promoted_initiative_id,
          i.title AS promoted_initiative_title,
          c.promoted_conversation_id,
          cv.title AS promoted_conversation_title,
          c.promoted_task_id,
          dt.title AS promoted_task_title,
          c.created_at,
          c.updated_at,
          c.promoted_at
        FROM captures c
        LEFT JOIN deliverables d ON d.id = c.promoted_deliverable_id
        LEFT JOIN initiatives i ON i.id = c.promoted_initiative_id
        LEFT JOIN conversations cv ON cv.id = c.promoted_conversation_id
        LEFT JOIN deliverable_tasks dt ON dt.id = c.promoted_task_id
        WHERE (? IS NULL OR c.status = ?)
          AND (? IS NULL OR c.kind = ?)
        ORDER BY c.created_at DESC
        "#,
    )
    .bind(status_filter.clone())
    .bind(status_filter)
    .bind(kind_filter.clone())
    .bind(kind_filter)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn get_capture(pool: &SqlitePool, id: &str) -> Result<Capture, String> {
    sqlx::query_as::<_, Capture>(
        r#"
        SELECT
          c.id,
          c.kind,
          c.body,
          c.status,
          c.promoted_deliverable_id,
          d.title AS promoted_deliverable_title,
          c.promoted_initiative_id,
          i.title AS promoted_initiative_title,
          c.promoted_conversation_id,
          cv.title AS promoted_conversation_title,
          c.promoted_task_id,
          dt.title AS promoted_task_title,
          c.created_at,
          c.updated_at,
          c.promoted_at
        FROM captures c
        LEFT JOIN deliverables d ON d.id = c.promoted_deliverable_id
        LEFT JOIN initiatives i ON i.id = c.promoted_initiative_id
        LEFT JOIN conversations cv ON cv.id = c.promoted_conversation_id
        LEFT JOIN deliverable_tasks dt ON dt.id = c.promoted_task_id
        WHERE c.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "capture not found".to_string())
}

pub async fn dismiss_capture(pool: &SqlitePool, id: &str) -> Result<Capture, String> {
    let now = now_utc();
    let result = sqlx::query(
        r#"
        UPDATE captures
        SET status = 'dismissed', updated_at = ?
        WHERE id = ? AND status IN ('inbox', 'suggested')
        "#,
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("capture not found or already promoted".to_string());
    }

    get_capture(pool, id).await
}

pub async fn suggest_capture(pool: &SqlitePool, id: &str) -> Result<Capture, String> {
    let now = now_utc();
    let result = sqlx::query(
        r#"
        UPDATE captures
        SET status = 'suggested', updated_at = ?
        WHERE id = ? AND status = 'inbox'
        "#,
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("capture not found or not in inbox".to_string());
    }

    get_capture(pool, id).await
}

pub async fn restore_capture_to_inbox(pool: &SqlitePool, id: &str) -> Result<Capture, String> {
    let now = now_utc();
    let result = sqlx::query(
        r#"
        UPDATE captures
        SET status = 'inbox', updated_at = ?
        WHERE id = ? AND status = 'suggested'
        "#,
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("capture not found or not in suggestions".to_string());
    }

    get_capture(pool, id).await
}

pub async fn promote_capture_to_deliverable(
    pool: &SqlitePool,
    capture_id: &str,
    input: CreateDeliverableInput,
) -> Result<Deliverable, String> {
    let capture = get_capture(pool, capture_id).await?;
    if capture.status != CaptureStatus::Inbox.as_str()
        && capture.status != CaptureStatus::Suggested.as_str()
    {
        return Err("only inbox or suggested captures can be promoted".to_string());
    }

    let input = validate_deliverable_input(
        input.title,
        input.deliverable_type,
        input.state,
        input.claim,
        input.artifact_url,
        input.conversation_id,
        input.stakeholder_id,
        input.stakeholder_ids,
        input.initiative_ids,
    )?;

    ensure_references_exist(
        pool,
        &input.initiative_ids,
        &input.stakeholder_ids,
        input.conversation_id.as_deref(),
    )
    .await?;

    let deliverable_id = Ulid::new().to_string();
    let now = now_utc();
    let shipped_at = shipped_at_for_state(input.state, None, &now);
    let mut tx = pool.begin().await.map_err(sql_error)?;

    insert_deliverable_in_tx(
        &mut tx,
        &deliverable_id,
        &input,
        &now,
        shipped_at.as_deref(),
    )
    .await?;
    replace_initiative_links(&mut tx, &deliverable_id, &input.initiative_ids).await?;
    replace_stakeholder_links(&mut tx, &deliverable_id, &input.stakeholder_ids).await?;

    let result = sqlx::query(
        r#"
        UPDATE captures
        SET status = 'promoted',
            promoted_deliverable_id = ?,
            promoted_at = ?,
            updated_at = ?
        WHERE id = ? AND status = 'inbox'
        "#,
    )
    .bind(&deliverable_id)
    .bind(&now)
    .bind(&now)
    .bind(capture_id)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("capture not found or not in inbox".to_string());
    }

    tx.commit().await.map_err(sql_error)?;
    get_deliverable(pool, &deliverable_id).await
}

pub async fn promote_capture_to_initiative(
    pool: &SqlitePool,
    capture_id: &str,
    input: CreateInitiativeInput,
) -> Result<Initiative, String> {
    let capture = get_capture(pool, capture_id).await?;
    if capture.status != CaptureStatus::Inbox.as_str()
        && capture.status != CaptureStatus::Suggested.as_str()
    {
        return Err("only inbox or suggested captures can be promoted".to_string());
    }
    if capture.kind != CaptureKind::Thought.as_str() {
        return Err("only thought captures can be promoted to initiatives".to_string());
    }

    let input = validate_initiative_input(
        input.title,
        input.framing,
        input.status,
        input.icon,
        input.icon_color,
    )?;
    let initiative_id = Ulid::new().to_string();
    let now = now_utc();
    let mut tx = pool.begin().await.map_err(sql_error)?;

    sqlx::query(
        r#"
        INSERT INTO initiatives (id, title, framing, status, icon, icon_color, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&initiative_id)
    .bind(&input.title)
    .bind(&input.framing)
    .bind(input.status.as_str())
    .bind(&input.icon)
    .bind(&input.icon_color)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    let result = sqlx::query(
        r#"
        UPDATE captures
        SET status = 'promoted',
            promoted_initiative_id = ?,
            promoted_at = ?,
            updated_at = ?
        WHERE id = ? AND status = 'inbox'
        "#,
    )
    .bind(&initiative_id)
    .bind(&now)
    .bind(&now)
    .bind(capture_id)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("capture not found or not in inbox".to_string());
    }

    tx.commit().await.map_err(sql_error)?;
    get_initiative(pool, &initiative_id).await
}

pub async fn promote_capture_to_task(
    pool: &SqlitePool,
    capture_id: &str,
    deliverable_id: &str,
    title: String,
    notes: Option<String>,
    due_date: Option<String>,
) -> Result<crate::models::DeliverableTask, String> {
    let capture = get_capture(pool, capture_id).await?;
    if capture.status != CaptureStatus::Inbox.as_str()
        && capture.status != CaptureStatus::Suggested.as_str()
    {
        return Err("only inbox or suggested captures can be promoted".to_string());
    }

    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("task title cannot be empty".to_string());
    }

    let task = create_deliverable_task(
        pool,
        crate::models::CreateTaskInput {
            deliverable_id: deliverable_id.to_string(),
            title,
            due_date,
            notes,
            url: None,
        },
    )
    .await?;

    let now = now_utc();
    let result = sqlx::query(
        r#"
        UPDATE captures
        SET status = 'promoted',
            promoted_task_id = ?,
            promoted_at = ?,
            updated_at = ?
        WHERE id = ? AND (status = 'inbox' OR status = 'suggested')
        "#,
    )
    .bind(&task.id)
    .bind(&now)
    .bind(&now)
    .bind(capture_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("capture not found or not promotable".to_string());
    }

    Ok(task)
}

pub fn validate_capture_input(kind: CaptureKind, body: String) -> Result<String, String> {
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err("capture body is required".to_string());
    }

    match kind {
        CaptureKind::Thought => Ok(body),
        CaptureKind::ClaudeLink => normalize_claude_link(&body),
        CaptureKind::ArtifactLink => {
            if is_valid_http_url(&body) {
                Ok(body)
            } else {
                Err("artifact links must be valid http or https URLs".to_string())
            }
        }
    }
}

pub fn normalize_claude_link(value: &str) -> Result<String, String> {
    let value = value.trim();
    if has_ascii_whitespace(value) {
        return Err("Claude links must not contain whitespace".to_string());
    }

    let Some(rest) = value.strip_prefix("https://") else {
        return Err("Claude links must use https://claude.ai/chat/{id}".to_string());
    };

    let path = rest
        .strip_prefix("claude.ai/chat/")
        .or_else(|| rest.strip_prefix("www.claude.ai/chat/"));

    match path {
        Some(id)
            if !id.is_empty() && !id.contains('/') && !id.contains('?') && !id.contains('#') =>
        {
            Ok(format!("https://claude.ai/chat/{id}"))
        }
        _ => Err("Claude links must use https://claude.ai/chat/{id}".to_string()),
    }
}

pub fn is_valid_http_url(value: &str) -> bool {
    if has_ascii_whitespace(value) {
        return false;
    }

    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"));
    let Some(rest) = rest else {
        return false;
    };

    !rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .is_empty()
}
