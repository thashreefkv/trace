use tauri::{AppHandle, State};

use crate::{
    db::AppState,
    models::{
        ApplyFlaggedToBacklogInput, ApplyGeneratedTasksInput, CreateDeliverableInput,
        CreateLabelInput, CreateNoteInput, CreateTaskInput, Deliverable, DeliverableFilters,
        DeliverableNote, DeliverableState, DeliverableTask, GeneratedTasks, Label,
        ReorderDirectionInput, UpdateDeliverableInput, UpdateDeliverableMetadataInput,
        UpdateTaskInput,
    },
};

#[tauri::command]
pub async fn list_deliverables(
    filters: Option<DeliverableFilters>,
    state: State<'_, AppState>,
) -> Result<Vec<Deliverable>, String> {
    project_manager_shared::repo::list_deliverables(&state.pool, filters.unwrap_or_default()).await
}

#[tauri::command]
pub async fn list_deliverables_for_initiative(
    initiative_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Deliverable>, String> {
    project_manager_shared::repo::list_deliverables_for_initiative(&state.pool, &initiative_id)
        .await
}

#[tauri::command]
pub async fn get_deliverable(
    id: String,
    state: State<'_, AppState>,
) -> Result<Deliverable, String> {
    let result = project_manager_shared::repo::get_deliverable(&state.pool, &id).await;
    bump_embedding_priority(
        state.pool.clone(),
        project_manager_shared::entity_embeddings::EntityKind::Deliverable,
        id,
    );
    result
}

fn bump_embedding_priority(
    pool: sqlx::SqlitePool,
    kind: project_manager_shared::entity_embeddings::EntityKind,
    entity_id: String,
) {
    tauri::async_runtime::spawn(async move {
        let _ = project_manager_shared::entity_embeddings::enqueue_high_priority(
            &pool, kind, &entity_id,
        )
        .await;
    });
}

#[tauri::command]
pub async fn create_deliverable(
    input: CreateDeliverableInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Deliverable, String> {
    let deliverable = project_manager_shared::repo::create_deliverable(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(deliverable)
}

#[tauri::command]
pub async fn update_deliverable(
    id: String,
    input: UpdateDeliverableInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Deliverable, String> {
    let deliverable =
        project_manager_shared::repo::update_deliverable(&state.pool, &id, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(deliverable)
}

#[tauri::command]
pub async fn update_deliverable_state(
    id: String,
    state_value: DeliverableState,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Deliverable, String> {
    let deliverable =
        project_manager_shared::repo::update_deliverable_state(&state.pool, &id, state_value)
            .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(deliverable)
}

#[tauri::command]
pub async fn delete_deliverable(
    id: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    project_manager_shared::repo::delete_deliverable(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn search_deliverables(
    query: String,
    filters: Option<DeliverableFilters>,
    state: State<'_, AppState>,
) -> Result<Vec<Deliverable>, String> {
    project_manager_shared::repo::search_deliverables(
        &state.pool,
        &query,
        filters.unwrap_or_default(),
        50,
    )
    .await
}

// ── Tasks ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_deliverable_tasks(
    deliverable_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<DeliverableTask>, String> {
    project_manager_shared::repo::list_deliverable_tasks(&state.pool, &deliverable_id).await
}

#[tauri::command]
pub async fn create_deliverable_task(
    input: CreateTaskInput,
    state: State<'_, AppState>,
) -> Result<DeliverableTask, String> {
    let task = project_manager_shared::repo::create_deliverable_task(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(task)
}

#[tauri::command]
pub async fn update_deliverable_task(
    id: String,
    input: UpdateTaskInput,
    state: State<'_, AppState>,
) -> Result<DeliverableTask, String> {
    let task =
        project_manager_shared::repo::update_deliverable_task(&state.pool, &id, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(task)
}

#[tauri::command]
pub async fn delete_deliverable_task(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_deliverable_task(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn reorder_deliverable_task(
    input: ReorderDirectionInput,
    state: State<'_, AppState>,
) -> Result<Vec<DeliverableTask>, String> {
    project_manager_shared::repo::reorder_deliverable_task(&state.pool, &input.id, &input.direction)
        .await
}

// ── Notes ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_deliverable_notes(
    deliverable_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<DeliverableNote>, String> {
    project_manager_shared::repo::list_deliverable_notes(&state.pool, &deliverable_id).await
}

#[tauri::command]
pub async fn create_deliverable_note(
    input: CreateNoteInput,
    state: State<'_, AppState>,
) -> Result<DeliverableNote, String> {
    let note = project_manager_shared::repo::create_deliverable_note(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(note)
}

#[tauri::command]
pub async fn delete_deliverable_note(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_deliverable_note(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

// ── Metadata + Focus ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn update_deliverable_metadata(
    id: String,
    input: UpdateDeliverableMetadataInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Deliverable, String> {
    let deliverable =
        project_manager_shared::repo::update_deliverable_metadata(&state.pool, &id, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(deliverable)
}

#[tauri::command]
pub async fn set_deliverable_focus(
    id: String,
    focused: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Deliverable, String> {
    let deliverable =
        project_manager_shared::repo::set_deliverable_focus(&state.pool, &id, focused).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(deliverable)
}

// ── AI task generation ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_deliverable_tasks(
    deliverable_id: String,
    state: State<'_, AppState>,
) -> Result<GeneratedTasks, String> {
    let deliverable =
        project_manager_shared::repo::get_deliverable(&state.pool, &deliverable_id).await?;

    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err("Gemini API key not configured. Add it in Settings.".to_string());
    };

    let existing_tasks =
        project_manager_shared::repo::list_deliverable_tasks(&state.pool, &deliverable_id)
            .await
            .unwrap_or_default();

    let notes = project_manager_shared::repo::list_deliverable_notes(&state.pool, &deliverable_id)
        .await
        .unwrap_or_default();

    let brain_context = project_manager_shared::brain::retrieve_brain_context(
        &state.pool,
        &state.brain_path,
        project_manager_shared::models::BrainRetrieveInput {
            query: format!("{} {}", deliverable.title, deliverable.claim),
            focus_entity_id: Some(deliverable_id.clone()),
            max_hops: Some(2),
            limit: Some(20),
        },
    )
    .await
    .ok()
    .map(|ctx| ctx.summary);

    // Email threads linked directly to this deliverable
    let linked_threads = project_manager_shared::gmail::list_local_threads(
        &state.pool,
        project_manager_shared::gmail::GmailThreadFilter {
            deliverable_id: Some(deliverable_id.clone()),
            limit: Some(8),
            ..Default::default()
        },
    )
    .await
    .unwrap_or_default();

    let email_summaries: Vec<String> = linked_threads
        .iter()
        .filter_map(|t| {
            let title = t.ai_title.as_deref().unwrap_or(t.subject.as_str());
            let summary = t.summary.as_deref().unwrap_or("");
            if summary.is_empty() {
                Some(title.to_string())
            } else {
                Some(format!("{title}: {summary}"))
            }
        })
        .collect();

    // Meeting summaries for all stakeholders on this deliverable (deduplicated)
    let mut seen_meeting_ids = std::collections::HashSet::new();
    let mut all_meetings = Vec::new();
    for s in &deliverable.stakeholders {
        let ms = project_manager_shared::repo::list_meetings_for_stakeholder(&state.pool, &s.id)
            .await
            .unwrap_or_default();
        for m in ms {
            if seen_meeting_ids.insert(m.id.clone()) {
                all_meetings.push(m);
            }
        }
    }

    let meeting_notes: Vec<String> = all_meetings
        .iter()
        .filter_map(|m| {
            let parts: Vec<&str> = [m.summary.as_deref(), m.key_decisions.as_deref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.trim().is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(format!(
                    "{} ({}): {}",
                    m.title,
                    &m.date[..10.min(m.date.len())],
                    parts.join(" | ")
                ))
            }
        })
        .take(5)
        .collect();

    // Calendar events for all stakeholders (deduplicated)
    let mut seen_event_ids = std::collections::HashSet::new();
    let mut cal_lines: Vec<String> = Vec::new();
    for s in &deliverable.stakeholders {
        if let Ok(stakeholder) =
            project_manager_shared::repo::get_stakeholder(&state.pool, &s.id).await
        {
            if !stakeholder.email.is_empty() {
                let events =
                    project_manager_shared::google_calendar::get_stakeholder_calendar_events(
                        &state.pool,
                        &stakeholder.email,
                    )
                    .await
                    .unwrap_or_default();
                for ev in events {
                    if seen_event_ids.insert(ev.gcal_event_id.clone()) {
                        let date = ev
                            .start_datetime
                            .as_deref()
                            .and_then(|dt| dt.get(..10))
                            .unwrap_or(&ev.start_date);
                        let desc = ev
                            .description
                            .as_deref()
                            .filter(|d| !d.trim().is_empty())
                            .map(|d| {
                                let truncated = &d[..d.len().min(150)];
                                format!(": {truncated}")
                            })
                            .unwrap_or_default();
                        cal_lines.push(format!("{} ({}){}", ev.title, date, desc));
                    }
                }
            }
        }
    }
    // Cap at 8 events to keep prompt tight
    cal_lines.truncate(8);

    let stakeholder_names: Vec<&str> = deliverable
        .stakeholders
        .iter()
        .map(|s| s.name.as_str())
        .collect();

    project_manager_shared::gemini::generate_tasks(
        &state.pool,
        &api_key,
        &deliverable.title,
        &deliverable.claim,
        &deliverable.deliverable_type,
        &deliverable.state,
        deliverable.deadline.as_deref(),
        &deliverable
            .initiatives
            .iter()
            .map(|i| i.title.as_str())
            .collect::<Vec<_>>(),
        &stakeholder_names,
        &existing_tasks
            .iter()
            .map(|t| (t.title.as_str(), t.status.as_str()))
            .collect::<Vec<_>>(),
        &notes.iter().map(|n| n.body.as_str()).collect::<Vec<_>>(),
        &email_summaries
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        &meeting_notes.iter().map(String::as_str).collect::<Vec<_>>(),
        &cal_lines.iter().map(String::as_str).collect::<Vec<_>>(),
        brain_context.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn apply_generated_deliverable_tasks(
    input: ApplyGeneratedTasksInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Vec<DeliverableTask>, String> {
    let tasks =
        project_manager_shared::repo::apply_generated_deliverable_tasks(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(tasks)
}

// ── Labels ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_labels(state: State<'_, AppState>) -> Result<Vec<Label>, String> {
    project_manager_shared::repo::list_labels(&state.pool).await
}

#[tauri::command]
pub async fn create_label(
    input: CreateLabelInput,
    state: State<'_, AppState>,
) -> Result<Label, String> {
    let label = project_manager_shared::repo::create_label(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(label)
}

#[tauri::command]
pub async fn delete_label(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_label(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn assign_label_to_deliverable(
    deliverable_id: String,
    label_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::repo::assign_label(&state.pool, &deliverable_id, &label_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn remove_label_from_deliverable(
    deliverable_id: String,
    label_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::repo::remove_label(&state.pool, &deliverable_id, &label_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

// ── Within-column reorder ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn reorder_deliverable(
    id: String,
    display_order: i64,
    state: State<'_, AppState>,
) -> Result<Deliverable, String> {
    let deliverable =
        project_manager_shared::repo::reorder_deliverable(&state.pool, &id, display_order).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(deliverable)
}

#[tauri::command]
pub async fn reorder_deliverable_within_state(
    input: ReorderDirectionInput,
    state: State<'_, AppState>,
) -> Result<Vec<Deliverable>, String> {
    project_manager_shared::repo::reorder_deliverable_within_state(
        &state.pool,
        &input.id,
        &input.direction,
    )
    .await
    .map(|deliverables| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        deliverables
    })
}

// ── State move with friction note ─────────────────────────────────────────────

#[tauri::command]
pub async fn update_deliverable_state_friction(
    id: String,
    state_value: String,
    friction_note: Option<String>,
    app_state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Deliverable, String> {
    let deliverable = project_manager_shared::repo::update_deliverable_state_friction(
        &app_state.pool,
        &id,
        &state_value,
        friction_note,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(app_state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(deliverable)
}

// ── Apply flagged meeting item to backlog ──────────────────────────────────────

#[tauri::command]
pub async fn apply_flagged_to_backlog(
    input: ApplyFlaggedToBacklogInput,
    state: State<'_, AppState>,
) -> Result<Deliverable, String> {
    let deliverable =
        project_manager_shared::repo::apply_flagged_to_backlog(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(deliverable)
}
