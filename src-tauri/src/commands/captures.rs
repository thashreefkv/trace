use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};

use crate::{
    commands::memory::auto_extract_from_text,
    db::AppState,
    models::{
        Capture, CaptureFilters, CreateCaptureInput, CreateDeliverableInput, CreateInitiativeInput,
        Deliverable, DeliverableTask, Initiative,
    },
};
use project_manager_shared::capture_promotion::{
    self, ApplyOutcome, PromotionAccuracySummary, PromotionAlternative, PromotionSuggestion,
};
use project_manager_shared::models::{
    BrainLearningEventInput, CaptureKind, DeliverableState, DeliverableType, InitiativeStatus,
};

const CAPTURE_MEMORY_MIN_BODY: usize = 80;
const SOURCE_CAPTURE_PROMOTE: &str = "capture_promote";

#[derive(Debug, Clone, Serialize)]
pub struct AppliedPromotion {
    pub capture: Capture,
    pub kind: String,
    pub applied_entity_kind: String,
    pub applied_entity_id: String,
}

#[tauri::command]
pub async fn create_capture(
    input: CreateCaptureInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Capture, String> {
    let capture = project_manager_shared::repo::create_capture(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    if capture.body.trim().chars().count() >= CAPTURE_MEMORY_MIN_BODY {
        let pool = state.pool.clone();
        let app_support_dir = state.app_support_dir.clone();
        let app = app.clone();
        let body = capture.body.clone();
        let kind = capture.kind.clone();
        let capture_id = capture.id.clone();
        tauri::async_runtime::spawn(async move {
            let source = format!("Capture (kind: {kind}). Body:\n{body}");
            auto_extract_from_text(
                &pool,
                &app_support_dir,
                &app,
                "capture",
                Some(&capture_id),
                &source,
            )
            .await;
        });
    }
    spawn_promotion_suggester(state.pool.clone(), app, capture.id.clone());
    Ok(capture)
}

fn spawn_promotion_suggester(pool: sqlx::SqlitePool, app: AppHandle, capture_id: String) {
    tauri::async_runtime::spawn(async move {
        // Exit silently when no Gemini key is configured. The suggester is
        // best-effort — the user's other flows aren't blocked on it.
        let Some(api_key) = project_manager_shared::runtime::gemini_api_key() else {
            return;
        };
        crate::bg_events::emit_started(&app, SOURCE_CAPTURE_PROMOTE);
        match capture_promotion::suggest_capture_promotion(&pool, &api_key, &capture_id).await {
            Ok(suggestion) => {
                let summary = format!(
                    "Suggested {} ({}%)",
                    suggestion.kind,
                    (suggestion.confidence * 100.0).round() as i64
                );
                crate::bg_events::emit_finished(&app, SOURCE_CAPTURE_PROMOTE, Some(summary));
                let _ = app.emit(
                    "capture:promotion_ready",
                    serde_json::json!({
                        "capture_id": capture_id,
                        "suggestion_id": suggestion.id,
                        "status": suggestion.status,
                    }),
                );
            }
            Err(error) => {
                crate::bg_events::emit_error(&app, SOURCE_CAPTURE_PROMOTE, &error);
                let _ = app.emit(
                    "capture:promotion_ready",
                    serde_json::json!({
                        "capture_id": capture_id,
                        "status": "errored",
                        "error": error,
                    }),
                );
            }
        }
    });
}

#[tauri::command]
pub async fn list_captures(
    filters: Option<CaptureFilters>,
    state: State<'_, AppState>,
) -> Result<Vec<Capture>, String> {
    project_manager_shared::repo::list_captures(&state.pool, filters.unwrap_or_default()).await
}

#[tauri::command]
pub async fn get_capture(id: String, state: State<'_, AppState>) -> Result<Capture, String> {
    let result = project_manager_shared::repo::get_capture(&state.pool, &id).await;
    let pool = state.pool.clone();
    let id_clone = id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = project_manager_shared::entity_embeddings::enqueue_high_priority(
            &pool,
            project_manager_shared::entity_embeddings::EntityKind::Capture,
            &id_clone,
        )
        .await;
    });
    result
}

#[tauri::command]
pub async fn dismiss_capture(id: String, state: State<'_, AppState>) -> Result<Capture, String> {
    let capture = project_manager_shared::repo::dismiss_capture(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(capture)
}

#[tauri::command]
pub async fn suggest_capture(id: String, state: State<'_, AppState>) -> Result<Capture, String> {
    let capture = project_manager_shared::repo::suggest_capture(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(capture)
}

#[tauri::command]
pub async fn restore_capture_to_inbox(
    id: String,
    state: State<'_, AppState>,
) -> Result<Capture, String> {
    let capture = project_manager_shared::repo::restore_capture_to_inbox(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(capture)
}

#[tauri::command]
pub async fn promote_capture_to_deliverable(
    capture_id: String,
    input: CreateDeliverableInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Deliverable, String> {
    let deliverable = project_manager_shared::repo::promote_capture_to_deliverable(
        &state.pool,
        &capture_id,
        input,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(deliverable)
}

#[tauri::command]
pub async fn promote_capture_to_initiative(
    capture_id: String,
    input: CreateInitiativeInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Initiative, String> {
    let initiative = project_manager_shared::repo::promote_capture_to_initiative(
        &state.pool,
        &capture_id,
        input,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(initiative)
}

#[tauri::command]
pub async fn promote_capture_to_task(
    capture_id: String,
    deliverable_id: String,
    title: String,
    notes: Option<String>,
    due_date: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<DeliverableTask, String> {
    let task = project_manager_shared::repo::promote_capture_to_task(
        &state.pool,
        &capture_id,
        &deliverable_id,
        title,
        notes,
        due_date,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(task)
}

// -------- Section 4: capture promotion AI --------

#[tauri::command]
pub async fn suggest_capture_promotion(
    capture_id: String,
    state: State<'_, AppState>,
) -> Result<PromotionSuggestion, String> {
    let api_key = project_manager_shared::runtime::gemini_api_key()
        .ok_or_else(|| "Gemini API key not configured".to_string())?;
    capture_promotion::suggest_capture_promotion(&state.pool, &api_key, &capture_id).await
}

#[tauri::command]
pub async fn get_capture_promotion_suggestion(
    capture_id: String,
    state: State<'_, AppState>,
) -> Result<Option<PromotionSuggestion>, String> {
    capture_promotion::get_current_suggestion(&state.pool, &capture_id).await
}

#[tauri::command]
pub async fn get_capture_promotion_accuracy(
    state: State<'_, AppState>,
) -> Result<PromotionAccuracySummary, String> {
    capture_promotion::promotion_accuracy_summary(&state.pool).await
}

#[tauri::command]
pub async fn apply_capture_promotion_suggestion(
    capture_id: String,
    suggestion_id: String,
    override_kind: Option<String>,
    override_target_id: Option<String>,
    override_alternative_index: Option<usize>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AppliedPromotion, String> {
    let suggestion = capture_promotion::get_suggestion_by_id(&state.pool, &suggestion_id).await?;
    if suggestion.capture_id != capture_id {
        return Err("suggestion does not match capture".to_string());
    }
    if suggestion.status != "pending" {
        return Err(format!(
            "suggestion already resolved (status: {})",
            suggestion.status
        ));
    }

    let capture = project_manager_shared::repo::get_capture(&state.pool, &capture_id).await?;

    // Resolve effective kind + target after overrides.
    let (effective_kind, effective_target, alt_index): (
        String,
        Option<(String, Option<String>)>,
        Option<usize>,
    ) = if let Some(idx) = override_alternative_index {
        let alt: &PromotionAlternative = suggestion
            .alternatives
            .get(idx)
            .ok_or_else(|| "alternative index out of range".to_string())?;
        (
            alt.kind.clone(),
            Some((alt.kind.clone(), alt.target_id.clone())),
            Some(idx),
        )
    } else if let Some(kind) = override_kind.clone() {
        (kind.clone(), Some((kind, override_target_id.clone())), None)
    } else {
        (suggestion.kind.clone(), None, None)
    };

    let effective_target_id = match &effective_target {
        Some((_, id)) => id.clone(),
        None => suggestion.target_id.clone(),
    };

    let (applied_entity_kind, applied_entity_id) = match effective_kind.as_str() {
        "task" => {
            let target_id = effective_target_id
                .clone()
                .ok_or_else(|| "task requires a deliverable target_id".to_string())?;
            let task = project_manager_shared::repo::promote_capture_to_task(
                &state.pool,
                &capture_id,
                &target_id,
                first_line(&capture.body),
                Some(capture.body.clone()),
                None,
            )
            .await?;
            ("task".to_string(), task.id)
        }
        "deliverable" => {
            let input = build_deliverable_input(&capture, effective_target_id.clone());
            let deliverable = project_manager_shared::repo::promote_capture_to_deliverable(
                &state.pool,
                &capture_id,
                input,
            )
            .await?;
            ("deliverable".to_string(), deliverable.id)
        }
        "initiative" => {
            if capture.kind != CaptureKind::Thought.as_str() {
                return Err("only thought captures can be promoted to initiatives".to_string());
            }
            let input = build_initiative_input(&capture);
            let initiative = project_manager_shared::repo::promote_capture_to_initiative(
                &state.pool,
                &capture_id,
                input,
            )
            .await?;
            ("initiative".to_string(), initiative.id)
        }
        other => return Err(format!("invalid promotion kind: {other}")),
    };

    let outcome = match (override_kind.as_deref(), alt_index, &effective_target) {
        (None, None, None) => ApplyOutcome::Accepted,
        (_, Some(idx), _) => ApplyOutcome::AcceptedAlternative { index: idx },
        _ => ApplyOutcome::Overridden {
            used_kind: effective_kind.clone(),
            used_target_id: effective_target_id.clone(),
        },
    };

    capture_promotion::record_apply_outcome(
        &state.pool,
        &suggestion_id,
        outcome,
        &applied_entity_kind,
        &applied_entity_id,
    )
    .await?;

    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;

    let capture = project_manager_shared::repo::get_capture(&state.pool, &capture_id).await?;
    Ok(AppliedPromotion {
        capture,
        kind: effective_kind,
        applied_entity_kind,
        applied_entity_id,
    })
}

#[tauri::command]
pub async fn undo_capture_promotion(
    suggestion_id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Capture, String> {
    let suggestion = capture_promotion::get_suggestion_by_id(&state.pool, &suggestion_id).await?;
    let applied_kind = suggestion
        .applied_entity_kind
        .clone()
        .ok_or_else(|| "this suggestion has not been applied".to_string())?;
    let applied_id = suggestion
        .applied_entity_id
        .clone()
        .ok_or_else(|| "this suggestion has not been applied".to_string())?;

    if matches!(suggestion.status.as_str(), "undone" | "stale" | "errored") {
        return Err(format!(
            "suggestion is not undoable (status: {})",
            suggestion.status
        ));
    }

    ensure_entity_untouched(&state.pool, &applied_kind, &applied_id).await?;

    match applied_kind.as_str() {
        "task" => {
            project_manager_shared::repo::delete_deliverable_task(&state.pool, &applied_id).await?;
        }
        "deliverable" => {
            project_manager_shared::repo::delete_deliverable(&state.pool, &applied_id).await?;
        }
        "initiative" => {
            project_manager_shared::repo::delete_initiative(&state.pool, &applied_id).await?;
        }
        other => return Err(format!("unknown applied entity kind: {other}")),
    }

    sqlx::query(
        r#"
        UPDATE captures
           SET status = 'inbox',
               promoted_deliverable_id = NULL,
               promoted_initiative_id = NULL,
               promoted_task_id = NULL,
               promoted_at = NULL,
               updated_at = ?
         WHERE id = ?
        "#,
    )
    .bind(project_manager_shared::repo::now_utc())
    .bind(&suggestion.capture_id)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("reset capture: {e}"))?;

    capture_promotion::record_undo(&state.pool, &suggestion_id).await?;

    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;

    project_manager_shared::repo::get_capture(&state.pool, &suggestion.capture_id).await
}

#[tauri::command]
pub async fn record_capture_promotion_event(
    suggestion_id: String,
    event_type: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Lightweight passthrough so the UI can record `shown` events explicitly
    // (e.g. when a stale suggestion gets re-displayed) without invoking the
    // bigger record_brain_learning_event surface from JS.
    let suggestion = capture_promotion::get_suggestion_by_id(&state.pool, &suggestion_id).await?;
    let _ = project_manager_shared::brain::record_brain_learning_event(
        &state.pool,
        BrainLearningEventInput {
            template: Some(capture_promotion::TEMPLATE.to_string()),
            item_id: suggestion_id.clone(),
            item_kind: Some(capture_promotion::ITEM_KIND.to_string()),
            event_type,
            reward: Some(0.0),
            context: Some(serde_json::json!({
                "confidence": suggestion.confidence,
                "kind": suggestion.kind,
                "target_id": suggestion.target_id,
            })),
        },
    )
    .await;
    Ok(())
}

// -------- helpers --------

async fn ensure_entity_untouched(
    pool: &sqlx::SqlitePool,
    applied_kind: &str,
    applied_id: &str,
) -> Result<(), String> {
    let (table, label) = match applied_kind {
        "task" => ("deliverable_tasks", "task"),
        "deliverable" => ("deliverables", "deliverable"),
        "initiative" => ("initiatives", "initiative"),
        other => return Err(format!("unknown applied entity kind: {other}")),
    };
    let row: Option<(String, String)> = sqlx::query_as(&format!(
        "SELECT created_at, updated_at FROM {table} WHERE id = ?"
    ))
    .bind(applied_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("read {label} for undo check: {e}"))?;
    let (created, updated) = row.ok_or_else(|| format!("{label} no longer exists"))?;
    if created != updated {
        return Err(format!(
            "the {label} has been edited since it was created — undo refused"
        ));
    }
    Ok(())
}

fn first_line(body: &str) -> String {
    body.lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .unwrap_or("Untitled")
        .chars()
        .take(120)
        .collect()
}

fn build_deliverable_input(
    capture: &project_manager_shared::models::Capture,
    initiative_id: Option<String>,
) -> CreateDeliverableInput {
    CreateDeliverableInput {
        title: first_line(&capture.body),
        deliverable_type: DeliverableType::Other,
        state: DeliverableState::Drafting,
        claim: capture.body.clone(),
        artifact_url: None,
        conversation_id: None,
        stakeholder_id: None,
        stakeholder_ids: Vec::new(),
        initiative_ids: initiative_id.into_iter().collect(),
    }
}

fn build_initiative_input(
    capture: &project_manager_shared::models::Capture,
) -> CreateInitiativeInput {
    CreateInitiativeInput {
        title: first_line(&capture.body),
        framing: capture.body.clone(),
        status: InitiativeStatus::Live,
        icon: "target".to_string(),
        icon_color: "#6366f1".to_string(),
    }
}

// ─── Apple Notes sync ───────────────────────────────────────────────────────

/// Core sync logic: called by both the background poller and the manual
/// command. Returns the number of new captures created.
pub async fn run_apple_notes_sync(pool: &SqlitePool) -> Result<u32, String> {
    // Blocking osascript calls are offloaded to the thread-pool so we don't
    // stall the async runtime.
    let notes = tokio::task::spawn_blocking(|| {
        crate::apple_notes::ensure_folders()?;
        crate::apple_notes::list_inbox_notes()
    })
    .await
    .map_err(|e| format!("spawn_blocking panicked: {e}"))??;

    let mut created = 0u32;

    for note in notes {
        // Dedup: skip notes we have already imported.
        let already: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM apple_notes_imported WHERE note_id = ?)",
        )
        .bind(&note.id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

        if already {
            continue;
        }

        // Build the capture body — prepend the title if it adds information.
        let body = if note.title.is_empty() || note.body.starts_with(&note.title) {
            note.body.trim().to_string()
        } else {
            format!("{}\n\n{}", note.title, note.body.trim())
        };

        if body.is_empty() {
            continue;
        }

        let capture = project_manager_shared::repo::create_capture(
            pool,
            project_manager_shared::models::CreateCaptureInput {
                kind: CaptureKind::Thought,
                body,
            },
        )
        .await?;

        // Record the mapping so we never import this note again.
        sqlx::query(
            "INSERT OR IGNORE INTO apple_notes_imported (note_id, capture_id) VALUES (?, ?)",
        )
        .bind(&note.id)
        .bind(&capture.id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

        // Move the note to "Trace — Captured" (best-effort).
        let note_id = note.id.clone();
        if let Err(e) =
            tokio::task::spawn_blocking(move || crate::apple_notes::move_to_captured(&note_id))
                .await
                .map_err(|e| format!("spawn_blocking panicked: {e}"))?
        {
            eprintln!("[apple_notes] move_to_captured failed for {}: {e}", note.id);
        }

        created += 1;
    }

    Ok(created)
}

/// Spawn the background poller that checks the "Trace" Notes folder every
/// 60 seconds and imports new notes as Captures.
pub fn spawn_apple_notes_sync(pool: SqlitePool) {
    tauri::async_runtime::spawn(async move {
        // Brief startup delay so migrations are guaranteed to have run.
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        loop {
            match run_apple_notes_sync(&pool).await {
                Ok(n) if n > 0 => eprintln!("[apple_notes] imported {n} note(s)"),
                Err(e) => eprintln!("[apple_notes] sync error: {e}"),
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

/// Tauri command: ensure both Notes folders exist and return their names.
/// Call this once from the Settings UI to bootstrap the integration.
#[tauri::command]
pub async fn apple_notes_setup() -> Result<(String, String), String> {
    tokio::task::spawn_blocking(|| crate::apple_notes::ensure_folders())
        .await
        .map_err(|e| format!("spawn_blocking panicked: {e}"))??;
    Ok((
        crate::apple_notes::INBOX_FOLDER.to_string(),
        crate::apple_notes::CAPTURED_FOLDER.to_string(),
    ))
}

/// Tauri command: run a sync immediately and return the number of new captures.
#[tauri::command]
pub async fn apple_notes_sync_now(state: State<'_, AppState>) -> Result<u32, String> {
    run_apple_notes_sync(&state.pool).await
}
