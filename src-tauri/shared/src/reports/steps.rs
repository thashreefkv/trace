//! Persistence + lifecycle helpers for the `report_steps` state machine.
//! Every named step (resolve_scope, plan_sections, draft_section_N, critique,
//! steer) lives as a row in `report_steps` and emits a [`ReportEvent`] as it
//! transitions. The frontend listens on the `report:event` channel and shows
//! status, latency, and model badges live.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::Value;
use sqlx::SqlitePool;
use tokio::sync::oneshot;

use crate::db::sql_error;
use crate::models::{ReportClarification, ReportStep};

use super::events::ReportEvent;

/// Map of `step_id -> oneshot::Sender` waiting on a user clarification answer.
/// Lives on `AppState` so commands can fire the sender when the user submits
/// a `ClarifyPrompt`.
pub type ClarificationRegistry = Arc<Mutex<HashMap<String, oneshot::Sender<ClarificationAnswer>>>>;

#[derive(Debug, Clone)]
pub struct ClarificationAnswer {
    pub selected_labels: Vec<String>,
    pub free_text: Option<String>,
}

pub fn new_clarification_registry() -> ClarificationRegistry {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Insert a new `queued` row for the step. The caller usually transitions it
/// to `running` immediately via [`start_step`].
pub async fn create_step(
    pool: &SqlitePool,
    report_run_id: &str,
    step_name: &str,
    section_index: Option<i64>,
    input: &Value,
    ticker_label: Option<&str>,
) -> Result<ReportStep, String> {
    let id = format!("step:{}", ulid::Ulid::new());
    let now = crate::repo::now_utc();
    let input_json = serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        r#"INSERT INTO report_steps
           (id, report_run_id, step_name, section_index, status, cache_hit,
            input_json, output_json, ticker_label, created_at, updated_at)
           VALUES (?, ?, ?, ?, 'queued', 0, ?, '{}', ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(report_run_id)
    .bind(step_name)
    .bind(section_index)
    .bind(&input_json)
    .bind(ticker_label)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    fetch_step(pool, &id).await
}

/// Transition a step to `running`. Returns the elapsed-time tracker for the
/// caller to consult on completion.
pub async fn start_step(
    pool: &SqlitePool,
    step_id: &str,
    ticker_label: Option<&str>,
) -> Result<Instant, String> {
    let now = crate::repo::now_utc();
    sqlx::query(
        "UPDATE report_steps SET status = 'running', started_at = ?, ticker_label = COALESCE(?, ticker_label), updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(ticker_label)
    .bind(&now)
    .bind(step_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(Instant::now())
}

pub async fn update_ticker(
    pool: &SqlitePool,
    step_id: &str,
    ticker_label: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE report_steps SET ticker_label = ?, updated_at = ? WHERE id = ?",
    )
    .bind(ticker_label)
    .bind(crate::repo::now_utc())
    .bind(step_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn complete_step(
    pool: &SqlitePool,
    step_id: &str,
    output: &Value,
    model: Option<&str>,
    cache_hit: bool,
    latency_ms: i64,
) -> Result<(), String> {
    let now = crate::repo::now_utc();
    let output_json = serde_json::to_string(output).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        r#"UPDATE report_steps
           SET status = 'done', output_json = ?, model = ?, cache_hit = ?,
               finished_at = ?, latency_ms = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&output_json)
    .bind(model)
    .bind(if cache_hit { 1_i64 } else { 0 })
    .bind(&now)
    .bind(latency_ms)
    .bind(&now)
    .bind(step_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn fail_step(
    pool: &SqlitePool,
    step_id: &str,
    error: &str,
) -> Result<(), String> {
    let now = crate::repo::now_utc();
    sqlx::query(
        r#"UPDATE report_steps SET status = 'error', error_text = ?, finished_at = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(error)
    .bind(&now)
    .bind(&now)
    .bind(step_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn cancel_step(pool: &SqlitePool, step_id: &str) -> Result<(), String> {
    let now = crate::repo::now_utc();
    sqlx::query(
        "UPDATE report_steps SET status = 'cancelled', finished_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(step_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

/// Pause the step until the user answers via [`ClarificationRegistry`].
/// The caller should `await` the returned receiver. On answer, transition
/// the step back to `running` manually before resuming work.
pub async fn await_clarification(
    pool: &SqlitePool,
    registry: &ClarificationRegistry,
    step_id: &str,
    prompt: &ReportClarification,
) -> Result<oneshot::Receiver<ClarificationAnswer>, String> {
    let now = crate::repo::now_utc();
    let clarification_json = serde_json::to_string(prompt).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        r#"UPDATE report_steps SET status = 'awaiting_clarification', clarification_json = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&clarification_json)
    .bind(&now)
    .bind(step_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    let (tx, rx) = oneshot::channel();
    {
        let mut guard = registry.lock().expect("clarification registry poisoned");
        guard.insert(step_id.to_string(), tx);
    }
    Ok(rx)
}

pub fn answer_clarification(
    registry: &ClarificationRegistry,
    step_id: &str,
    answer: ClarificationAnswer,
) -> Result<(), String> {
    let sender = {
        let mut guard = registry.lock().expect("clarification registry poisoned");
        guard.remove(step_id)
    };
    let sender = sender.ok_or_else(|| {
        format!("No pending clarification for step {step_id} — it may have expired.")
    })?;
    sender
        .send(answer)
        .map_err(|_| "Clarification listener disappeared before the answer arrived.".to_string())
}

pub async fn fetch_step(pool: &SqlitePool, step_id: &str) -> Result<ReportStep, String> {
    sqlx::query_as::<_, ReportStep>(
        r#"SELECT id, report_run_id, step_name, section_index, status, model, cache_hit,
                  started_at, finished_at, latency_ms, input_json, output_json,
                  clarification_json, ticker_label, error_text, created_at, updated_at
           FROM report_steps WHERE id = ?"#,
    )
    .bind(step_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "Report step not found.".to_string())
}

pub async fn list_steps(
    pool: &SqlitePool,
    report_run_id: &str,
) -> Result<Vec<ReportStep>, String> {
    sqlx::query_as::<_, ReportStep>(
        r#"SELECT id, report_run_id, step_name, section_index, status, model, cache_hit,
                  started_at, finished_at, latency_ms, input_json, output_json,
                  clarification_json, ticker_label, error_text, created_at, updated_at
           FROM report_steps WHERE report_run_id = ? ORDER BY created_at ASC"#,
    )
    .bind(report_run_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

/// Resolve the latest step matching a name + section_id (looking inside
/// input_json). Used by the "rerun this step" command to find what to redo.
pub async fn latest_step_for(
    pool: &SqlitePool,
    report_run_id: &str,
    step_name: &str,
    section_id: Option<&str>,
) -> Result<Option<ReportStep>, String> {
    let steps = list_steps(pool, report_run_id).await?;
    let mut matching: Vec<ReportStep> = steps
        .into_iter()
        .filter(|step| step.step_name == step_name)
        .collect();
    if let Some(section_id) = section_id {
        matching.retain(|step| {
            step.input_json.contains(&format!("\"section_id\":\"{section_id}\""))
        });
    }
    matching.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(matching.into_iter().next())
}

pub fn into_event_started(step: &ReportStep) -> ReportEvent {
    ReportEvent::StepStarted {
        step_id: step.id.clone(),
        report_run_id: step.report_run_id.clone(),
        step_name: step.step_name.clone(),
        ticker_label: step.ticker_label.clone(),
    }
}

pub fn into_event_created(step: &ReportStep) -> ReportEvent {
    let section_id = serde_json::from_str::<Value>(&step.input_json)
        .ok()
        .and_then(|value| {
            value
                .get("section_id")
                .and_then(|inner| inner.as_str())
                .map(|s| s.to_string())
        });
    ReportEvent::StepCreated {
        report_run_id: step.report_run_id.clone(),
        step_id: step.id.clone(),
        step_name: step.step_name.clone(),
        section_index: step.section_index,
        section_id,
    }
}
