use tauri::State;

use crate::db::AppState;
use project_manager_shared::models::{
    CaptureKind, CreateCaptureInput, CreateStakeholderInput, DeliverableFilters, DeliverableState,
    Stakeholder, StakeholderBriefing, StakeholderDetail, UpdateStakeholderInput,
};

#[tauri::command]
pub async fn list_stakeholders(state: State<'_, AppState>) -> Result<Vec<Stakeholder>, String> {
    project_manager_shared::repo::list_stakeholders(&state.pool).await
}

#[tauri::command]
pub async fn list_stakeholder_details(
    state: State<'_, AppState>,
) -> Result<Vec<StakeholderDetail>, String> {
    project_manager_shared::repo::list_stakeholder_details(&state.pool).await
}

#[tauri::command]
pub async fn get_stakeholder_detail(
    id: String,
    state: State<'_, AppState>,
) -> Result<StakeholderDetail, String> {
    let result = project_manager_shared::repo::get_stakeholder_detail(&state.pool, &id).await;
    let pool = state.pool.clone();
    let id_clone = id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = project_manager_shared::entity_embeddings::enqueue_high_priority(
            &pool,
            project_manager_shared::entity_embeddings::EntityKind::Stakeholder,
            &id_clone,
        )
        .await;
    });
    result
}

#[tauri::command]
pub async fn create_stakeholder(
    input: CreateStakeholderInput,
    state: State<'_, AppState>,
) -> Result<Stakeholder, String> {
    let stakeholder = project_manager_shared::repo::create_stakeholder(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(stakeholder)
}

#[tauri::command]
pub async fn update_stakeholder(
    id: String,
    input: UpdateStakeholderInput,
    state: State<'_, AppState>,
) -> Result<Stakeholder, String> {
    let stakeholder =
        project_manager_shared::repo::update_stakeholder(&state.pool, &id, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(stakeholder)
}

#[tauri::command]
pub async fn delete_stakeholder(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_stakeholder(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn generate_stakeholder_briefing(
    stakeholder_id: String,
    state: State<'_, AppState>,
) -> Result<StakeholderBriefing, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?;

    let stakeholder =
        project_manager_shared::repo::get_stakeholder(&state.pool, &stakeholder_id).await?;

    let filters = project_manager_shared::models::DeliverableFilters {
        stakeholder_id: Some(stakeholder_id.clone()),
        ..Default::default()
    };
    let deliverables =
        project_manager_shared::repo::list_deliverables(&state.pool, filters).await?;

    let recent_threads =
        project_manager_shared::gmail::stakeholder_threads(&state.pool, &stakeholder_id, 15)
            .await
            .unwrap_or_default();

    let recent_meetings =
        project_manager_shared::repo::list_meetings_for_stakeholder(&state.pool, &stakeholder_id)
            .await
            .unwrap_or_default();

    let brain_summary = project_manager_shared::brain::retrieve_brain_context(
        &state.pool,
        &state.brain_path,
        project_manager_shared::models::BrainRetrieveInput {
            query: stakeholder.name.clone(),
            focus_entity_id: Some(stakeholder_id),
            max_hops: Some(2),
            limit: Some(20),
        },
    )
    .await
    .ok()
    .map(|ctx| ctx.summary);

    match api_key {
        Some(key) => project_manager_shared::gemini::generate_stakeholder_briefing(
            &state.pool,
            &key,
            &stakeholder,
            &deliverables,
            &recent_threads,
            &recent_meetings,
            brain_summary,
        )
        .await
        .or_else(|_| {
            Ok(
                project_manager_shared::gemini::fallback_stakeholder_briefing(
                    stakeholder,
                    deliverables,
                ),
            )
        }),
        None => Ok(
            project_manager_shared::gemini::fallback_stakeholder_briefing(
                stakeholder,
                deliverables,
            ),
        ),
    }
}

#[tauri::command]
pub async fn comment_on_briefing_item(
    state: State<'_, AppState>,
    stakeholder_id: String,
    section: String,
    item_text: String,
    item_source: Option<String>,
    comment: String,
) -> Result<String, String> {
    let stakeholder =
        project_manager_shared::repo::get_stakeholder(&state.pool, &stakeholder_id).await?;

    let deliverables = project_manager_shared::repo::list_deliverables(
        &state.pool,
        DeliverableFilters {
            stakeholder_id: Some(stakeholder_id.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap_or_default();

    let del_tuples: Vec<(&str, &str, &str)> = deliverables
        .iter()
        .filter(|d| d.state != "killed")
        .map(|d| (d.id.as_str(), d.title.as_str(), d.state.as_str()))
        .collect();

    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?;

    let action = if let Some(key) = api_key {
        project_manager_shared::gemini::process_briefing_comment(
            &state.pool,
            &key,
            &stakeholder.name,
            &section,
            &item_text,
            item_source.as_deref(),
            &comment,
            &del_tuples,
        )
        .await
        .unwrap_or_else(|_| project_manager_shared::gemini::BriefingCommentAction {
            action_type: "save_memory".to_string(),
            entity_id: None,
            new_state: None,
            content: None,
            action_summary: "Note saved.".to_string(),
            memory_title: format!("Note on {} ({})", stakeholder.name, section),
            memory_body: format!("{item_text}: {comment}"),
        })
    } else {
        project_manager_shared::gemini::BriefingCommentAction {
            action_type: "save_memory".to_string(),
            entity_id: None,
            new_state: None,
            content: None,
            action_summary: "Note saved.".to_string(),
            memory_title: format!("Note on {} ({})", stakeholder.name, section),
            memory_body: format!("{item_text}: {comment}"),
        }
    };

    // Execute the action
    match action.action_type.as_str() {
        "update_deliverable_state" => {
            if let (Some(entity_id), Some(state_str)) = (&action.entity_id, &action.new_state) {
                if let Ok(new_state) = DeliverableState::from_str(state_str) {
                    let _ = project_manager_shared::repo::update_deliverable_state(
                        &state.pool,
                        entity_id,
                        new_state,
                    )
                    .await;
                }
            }
        }
        "append_stakeholder_note" => {
            if let Some(content) = &action.content {
                let now_str = chrono::Utc::now().to_rfc3339();
                let date_prefix = &now_str[..10];
                let existing = stakeholder.notes.trim();
                let new_notes = if existing.is_empty() {
                    content.clone()
                } else {
                    format!("{existing}\n\n[{date_prefix}] {content}")
                };
                let _ =
                    sqlx::query("UPDATE stakeholders SET notes = ?, updated_at = ? WHERE id = ?")
                        .bind(&new_notes)
                        .bind(&now_str)
                        .bind(&stakeholder_id)
                        .execute(&state.pool)
                        .await;
            }
        }
        "create_capture" => {
            if let Some(content) = &action.content {
                let _ = project_manager_shared::repo::create_capture(
                    &state.pool,
                    CreateCaptureInput {
                        kind: CaptureKind::Thought,
                        body: content.clone(),
                    },
                )
                .await;
            }
        }
        _ => {}
    }

    // Always save a memory
    let now = chrono::Utc::now().to_rfc3339();
    let id = ulid::Ulid::new().to_string();
    let tags_json =
        serde_json::json!([&stakeholder.name, "briefing-comment", &section]).to_string();
    let _ = sqlx::query(
        "INSERT INTO memories (id, kind, status, scope, title, body, canonical_key, source, \
         source_kind, source_id, confidence, importance, tags_json, evidence_json, created_at, updated_at) \
         VALUES (?, 'episodic', 'active', 'global', ?, ?, ?, 'manual', 'stakeholder', ?, 0.85, 0.60, ?, '[]', ?, ?)",
    )
    .bind(&id)
    .bind(&action.memory_title)
    .bind(&action.memory_body)
    .bind(&id)
    .bind(&stakeholder_id)
    .bind(&tags_json)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await;

    Ok(action.action_summary)
}
