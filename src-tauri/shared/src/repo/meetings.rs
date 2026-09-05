use sqlx::{Sqlite, SqlitePool, Transaction};
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{
        ApplyMeetingActionInput, CreateMeetingInput, DeliverableState, GeminiMeetingOutput,
        Meeting, MeetingAction, MeetingConfig, MeetingRow, MeetingWithActions, StakeholderRef,
    },
};

use super::{
    create_deliverable_task, get_deliverable, now_utc, update_deliverable_metadata,
    update_deliverable_state,
};

pub async fn get_meeting_config(pool: &SqlitePool) -> Result<MeetingConfig, String> {
    let config = sqlx::query_as::<_, MeetingConfig>(
        "SELECT next_meeting_date FROM meeting_config WHERE id = 'singleton'",
    )
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .unwrap_or(MeetingConfig {
        next_meeting_date: None,
    });

    Ok(config)
}

pub async fn set_meeting_date(
    pool: &SqlitePool,
    date: Option<&str>,
) -> Result<MeetingConfig, String> {
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT INTO meeting_config (id, next_meeting_date, updated_at)
        VALUES ('singleton', ?, ?)
        ON CONFLICT(id) DO UPDATE SET next_meeting_date = excluded.next_meeting_date, updated_at = excluded.updated_at
        "#,
    )
    .bind(date)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_meeting_config(pool).await
}

pub async fn advance_meeting_date(pool: &SqlitePool) -> Result<MeetingConfig, String> {
    let config = get_meeting_config(pool).await?;
    let next = if let Some(ref date_str) = config.next_meeting_date {
        use chrono::NaiveDate;
        if let Ok(d) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            Some(
                (d + chrono::Duration::days(7))
                    .format("%Y-%m-%d")
                    .to_string(),
            )
        } else {
            None
        }
    } else {
        None
    };

    set_meeting_date(pool, next.as_deref()).await
}

// ── Meetings ──────────────────────────────────────────────────────────────────

pub async fn list_meetings(pool: &SqlitePool) -> Result<Vec<Meeting>, String> {
    let rows = sqlx::query_as::<_, MeetingRow>(
        r#"
        SELECT id, title, date, duration_secs, transcript, summary, key_decisions,
               status, error_message, created_at, updated_at
        FROM meetings
        ORDER BY date DESC, created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut meetings = Vec::with_capacity(rows.len());
    for row in rows {
        let stakeholders = fetch_stakeholders_for_meeting(pool, &row.id).await?;
        meetings.push(row.with_stakeholders(stakeholders));
    }
    Ok(meetings)
}

pub async fn get_meeting(pool: &SqlitePool, id: &str) -> Result<MeetingWithActions, String> {
    let row = sqlx::query_as::<_, MeetingRow>(
        r#"
        SELECT id, title, date, duration_secs, transcript, summary, key_decisions,
               status, error_message, created_at, updated_at
        FROM meetings WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| format!("meeting {id} not found"))?;

    let stakeholders = fetch_stakeholders_for_meeting(pool, id).await?;
    let meeting = row.with_stakeholders(stakeholders);

    let actions = list_meeting_actions(pool, id).await?;
    Ok(MeetingWithActions { meeting, actions })
}

pub async fn list_meeting_actions(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<MeetingAction>, String> {
    sqlx::query_as::<_, MeetingAction>(
        r#"
        SELECT id, meeting_id, kind, target_id, target_title, body, payload, applied, created_at
        FROM meeting_actions
        WHERE meeting_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn create_meeting(
    pool: &SqlitePool,
    input: CreateMeetingInput,
) -> Result<Meeting, String> {
    let id = Ulid::new().to_string();
    let now = now_utc();
    let date = if input.date.trim().is_empty() {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    } else {
        input.date
    };

    let mut tx = pool.begin().await.map_err(sql_error)?;

    sqlx::query(
        r#"
        INSERT INTO meetings (id, title, date, status, created_at, updated_at)
        VALUES (?, ?, ?, 'draft', ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.title)
    .bind(&date)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    replace_meeting_stakeholders(&mut tx, &id, &input.stakeholder_ids).await?;

    tx.commit().await.map_err(sql_error)?;

    get_meeting_by_id(pool, &id).await
}

async fn get_meeting_by_id(pool: &SqlitePool, id: &str) -> Result<Meeting, String> {
    let row = sqlx::query_as::<_, MeetingRow>(
        r#"
        SELECT id, title, date, duration_secs, transcript, summary, key_decisions,
               status, error_message, created_at, updated_at
        FROM meetings WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| format!("meeting {id} not found"))?;

    let stakeholders = fetch_stakeholders_for_meeting(pool, id).await?;
    Ok(row.with_stakeholders(stakeholders))
}

pub async fn fetch_stakeholders_for_meeting(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<StakeholderRef>, String> {
    sqlx::query_as::<_, StakeholderRef>(
        r#"
        SELECT s.id, s.name, s.role
        FROM stakeholders s
        JOIN meeting_stakeholders ms ON ms.stakeholder_id = s.id
        WHERE ms.meeting_id = ?
        ORDER BY s.name ASC
        "#,
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn replace_meeting_stakeholders(
    tx: &mut Transaction<'_, Sqlite>,
    meeting_id: &str,
    stakeholder_ids: &[String],
) -> Result<(), String> {
    sqlx::query("DELETE FROM meeting_stakeholders WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(&mut **tx)
        .await
        .map_err(sql_error)?;

    for sid in stakeholder_ids {
        sqlx::query("INSERT INTO meeting_stakeholders (meeting_id, stakeholder_id) VALUES (?, ?)")
            .bind(meeting_id)
            .bind(sid)
            .execute(&mut **tx)
            .await
            .map_err(sql_error)?;
    }
    Ok(())
}

pub async fn update_meeting_title(
    pool: &SqlitePool,
    id: &str,
    title: &str,
) -> Result<Meeting, String> {
    let now = now_utc();
    sqlx::query("UPDATE meetings SET title = ?, updated_at = ? WHERE id = ?")
        .bind(title)
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    get_meeting_by_id(pool, id).await
}

pub async fn save_meeting_processed(
    pool: &SqlitePool,
    id: &str,
    output: &GeminiMeetingOutput,
    duration_secs: i64,
) -> Result<MeetingWithActions, String> {
    let now = now_utc();
    let key_decisions_json = serde_json::to_string(&output.key_decisions)
        .map_err(|e| format!("failed to serialize key decisions: {e}"))?;

    sqlx::query(
        r#"
        UPDATE meetings
        SET title = ?, transcript = ?, summary = ?, key_decisions = ?,
            duration_secs = ?, status = 'done', error_message = NULL, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&output.title)
    .bind(&output.transcript)
    .bind(&output.summary)
    .bind(&key_decisions_json)
    .bind(duration_secs)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    // Insert action suggestions
    for suggestion in &output.action_suggestions {
        let action_id = Ulid::new().to_string();
        let target_title = if suggestion.suggested_target.trim().is_empty() {
            None
        } else {
            Some(suggestion.suggested_target.trim())
        };
        sqlx::query(
            r#"
            INSERT INTO meeting_actions (id, meeting_id, kind, target_title, body, payload, applied, created_at)
            VALUES (?, ?, ?, ?, ?, NULL, 0, ?)
            "#,
        )
        .bind(&action_id)
        .bind(id)
        .bind(&suggestion.kind)
        .bind(target_title)
        .bind(suggestion.body.trim())
        .bind(&now)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    get_meeting(pool, id).await
}

pub async fn save_meeting_error(
    pool: &SqlitePool,
    id: &str,
    error: &str,
) -> Result<Meeting, String> {
    let now = now_utc();
    sqlx::query(
        "UPDATE meetings SET status = 'error', error_message = ?, updated_at = ? WHERE id = ?",
    )
    .bind(error)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    get_meeting_by_id(pool, id).await
}

/// Save the result of an agentic minutes upload to an existing meeting record.
/// Updates title/date (if extracted), saves summary, and persists proposed
/// actions as unapplied MeetingAction rows. Approval applies the writes later.
pub async fn save_minutes_summary(
    pool: &SqlitePool,
    meeting_id: &str,
    result: &crate::models::MinutesProcessingResult,
) -> Result<MeetingWithActions, String> {
    let now = now_utc();

    // Overwrite title only if the agent extracted a real one
    if let Some(title) = result
        .meeting_title
        .as_deref()
        .filter(|t| !t.trim().is_empty())
    {
        sqlx::query("UPDATE meetings SET title = ?, updated_at = ? WHERE id = ?")
            .bind(title)
            .bind(&now)
            .bind(meeting_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }

    // Overwrite date only if the agent found one
    if let Some(date) = result
        .meeting_date
        .as_deref()
        .filter(|d| !d.trim().is_empty())
    {
        sqlx::query("UPDATE meetings SET date = ?, updated_at = ? WHERE id = ?")
            .bind(date)
            .bind(&now)
            .bind(meeting_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }

    sqlx::query(
        "UPDATE meetings SET summary = ?, status = 'done', error_message = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(&result.summary)
    .bind(&now)
    .bind(meeting_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    // Persist every agent action as a pending proposal so the UI can review it.
    for action in &result.actions {
        let action_id = Ulid::new().to_string();
        let kind = normalize_minutes_action_kind(action);
        let payload = serde_json::to_string(action)
            .map_err(|e| format!("failed to serialize minutes action payload: {e}"))?;
        sqlx::query(
            r#"INSERT INTO meeting_actions (id, meeting_id, kind, target_id, target_title, body, payload, applied, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)"#,
        )
        .bind(&action_id)
        .bind(meeting_id)
        .bind(&kind)
        .bind(action.target_id.as_deref())
        .bind(action.target.as_deref())
        .bind(&action.detail)
        .bind(&payload)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    // Persist flagged deliverables as pending capture proposals too.
    for item in &result.flagged {
        let action_id = Ulid::new().to_string();
        let body = format!(
            "{} — {} (suggested type: {})",
            item.claim, item.why, item.suggested_type
        );
        let payload = serde_json::to_string(item)
            .map_err(|e| format!("failed to serialize flagged deliverable payload: {e}"))?;
        sqlx::query(
            r#"INSERT INTO meeting_actions (id, meeting_id, kind, target_title, body, payload, applied, created_at)
               VALUES (?, ?, 'flagged', ?, ?, ?, 0, ?)"#,
        )
        .bind(&action_id)
        .bind(meeting_id)
        .bind(&item.title)
        .bind(&body)
        .bind(&payload)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    get_meeting(pool, meeting_id).await
}

fn normalize_minutes_action_kind(action: &crate::models::MinutesAction) -> String {
    match action.kind.as_str() {
        "note_added" => match action.target_kind.as_deref() {
            Some("initiative") => "initiative_note".to_string(),
            _ => "deliverable_note".to_string(),
        },
        other => other.to_string(),
    }
}

pub async fn delete_meeting(pool: &SqlitePool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM meetings WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn apply_meeting_action(
    pool: &SqlitePool,
    input: ApplyMeetingActionInput,
) -> Result<MeetingAction, String> {
    // Load the action
    let action = sqlx::query_as::<_, MeetingAction>(
        "SELECT id, meeting_id, kind, target_id, target_title, body, payload, applied, created_at FROM meeting_actions WHERE id = ?",
    )
    .bind(&input.action_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| format!("meeting action {} not found", input.action_id))?;

    if action.applied {
        return Ok(action);
    }

    let now = now_utc();
    let payload = action_payload(&action)?;
    let resolved_target = input.target_id.clone().or(action.target_id.clone());

    match action.kind.as_str() {
        "deliverable_note" => {
            let deliverable_id = required_target("deliverable_note", resolved_target.as_deref())?;
            let note_id = Ulid::new().to_string();
            sqlx::query(
                "INSERT INTO deliverable_notes (id, deliverable_id, body, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(&note_id)
            .bind(deliverable_id)
            .bind(&action.body)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
        "initiative_note" => {
            let initiative_id = required_target("initiative_note", resolved_target.as_deref())?;
            let note_id = Ulid::new().to_string();
            sqlx::query(
                "INSERT INTO initiative_notes (id, initiative_id, body, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(&note_id)
            .bind(initiative_id)
            .bind(&action.body)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
        "capture" | "capture_created" => {
            let capture_id = Ulid::new().to_string();
            sqlx::query(
                "INSERT INTO captures (id, kind, body, status, created_at, updated_at) VALUES (?, 'thought', ?, 'inbox', ?, ?)",
            )
            .bind(&capture_id)
            .bind(&action.body)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
        "task_created" => {
            let deliverable_id = required_target("task_created", resolved_target.as_deref())?;
            let title = payload_string(&payload, "title")
                .or_else(|| Some(action.body.trim().to_string()))
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "task_created requires a title".to_string())?;
            let due_date = payload_string(&payload, "due_date").filter(|value| !value.is_empty());
            create_deliverable_task(
                pool,
                crate::models::CreateTaskInput {
                    deliverable_id: deliverable_id.to_string(),
                    title,
                    due_date,
                    notes: None,
                    url: None,
                },
            )
            .await?;
        }
        "state_updated" => {
            let deliverable_id = required_target("state_updated", resolved_target.as_deref())?;
            let state = payload_string(&payload, "state")
                .ok_or_else(|| "state_updated requires payload.state".to_string())?;
            update_deliverable_state(pool, deliverable_id, parse_deliverable_state(&state)?)
                .await?;
        }
        "deadline_set" => {
            let deliverable_id = required_target("deadline_set", resolved_target.as_deref())?;
            let deadline = payload_string(&payload, "deadline")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "deadline_set requires payload.deadline".to_string())?;
            let current = get_deliverable(pool, deliverable_id).await?;
            update_deliverable_metadata(
                pool,
                deliverable_id,
                crate::models::UpdateDeliverableMetadataInput {
                    deadline: Some(deadline),
                    effort: current.effort,
                    impact: current.impact,
                    blocker_reason: current.blocker_reason,
                    priority: current.priority,
                },
            )
            .await?;
        }
        "blocker_set" => {
            let deliverable_id = required_target("blocker_set", resolved_target.as_deref())?;
            let current = get_deliverable(pool, deliverable_id).await?;
            let blocker_value = payload_string(&payload, "blocker_reason")
                .ok_or_else(|| "blocker_set requires payload.blocker_reason".to_string())?;
            let blocker_reason = if blocker_value.trim().is_empty() {
                None
            } else {
                Some(blocker_value.trim().to_string())
            };
            update_deliverable_metadata(
                pool,
                deliverable_id,
                crate::models::UpdateDeliverableMetadataInput {
                    deadline: current.deadline,
                    effort: current.effort,
                    impact: current.impact,
                    blocker_reason,
                    priority: current.priority,
                },
            )
            .await?;
        }
        "flagged" => {
            let capture_id = Ulid::new().to_string();
            let title = action
                .target_title
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("Candidate deliverable");
            let body = format!("[CANDIDATE] {title}\n{}", action.body);
            sqlx::query(
                "INSERT INTO captures (id, kind, body, status, created_at, updated_at) VALUES (?, 'thought', ?, 'inbox', ?, ?)",
            )
            .bind(&capture_id)
            .bind(body)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
        other => return Err(format!("unknown action kind: {other}")),
    }

    // Update the resolved target_id and mark applied
    let resolved_target_title = resolve_meeting_action_target_title(
        pool,
        &action.kind,
        resolved_target.as_deref(),
        action.target_title.as_deref(),
    )
    .await?;
    sqlx::query(
        "UPDATE meeting_actions SET applied = 1, target_id = COALESCE(?, target_id), target_title = COALESCE(?, target_title) WHERE id = ?",
    )
    .bind(resolved_target.as_deref())
    .bind(resolved_target_title.as_deref())
    .bind(&input.action_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query_as::<_, MeetingAction>(
        "SELECT id, meeting_id, kind, target_id, target_title, body, payload, applied, created_at FROM meeting_actions WHERE id = ?",
    )
    .bind(&input.action_id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}

pub fn action_payload(action: &MeetingAction) -> Result<serde_json::Value, String> {
    action
        .payload
        .as_deref()
        .filter(|payload| !payload.trim().is_empty())
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()
        .map_err(|e| format!("invalid meeting action payload: {e}"))
        .map(|payload| payload.unwrap_or_else(|| serde_json::json!({})))
}

fn payload_string(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
}

fn required_target<'a>(kind: &str, target_id: Option<&'a str>) -> Result<&'a str, String> {
    target_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{kind} requires a target"))
}

pub fn parse_deliverable_state(state: &str) -> Result<DeliverableState, String> {
    match state {
        "backlog" => Ok(DeliverableState::Backlog),
        "todo" => Ok(DeliverableState::Todo),
        "drafting" => Ok(DeliverableState::Drafting),
        "in_review" => Ok(DeliverableState::InReview),
        "shipped" => Ok(DeliverableState::Shipped),
        "killed" => Ok(DeliverableState::Killed),
        other => Err(format!("unknown deliverable state: {other}")),
    }
}

async fn resolve_meeting_action_target_title(
    pool: &SqlitePool,
    kind: &str,
    target_id: Option<&str>,
    fallback: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(target_id) = target_id.filter(|value| !value.trim().is_empty()) else {
        return Ok(fallback.map(str::to_string));
    };

    let title = match kind {
        "initiative_note" => {
            sqlx::query_scalar::<_, String>("SELECT title FROM initiatives WHERE id = ?")
                .bind(target_id)
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?
        }
        "deliverable_note" | "task_created" | "state_updated" | "deadline_set" | "blocker_set" => {
            sqlx::query_scalar::<_, String>("SELECT title FROM deliverables WHERE id = ?")
                .bind(target_id)
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?
        }
        _ => fallback.map(str::to_string),
    };

    Ok(title.or_else(|| fallback.map(str::to_string)))
}

pub async fn dismiss_meeting_action(pool: &SqlitePool, action_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM meeting_actions WHERE id = ?")
        .bind(action_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

