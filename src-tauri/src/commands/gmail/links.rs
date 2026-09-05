use tauri::State;

use crate::db::AppState;

#[tauri::command]
pub async fn gmail_get_local_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailThreadDetail, String> {
    let result = project_manager_shared::gmail::get_local_thread(&state.pool, &thread_id).await;
    let pool = state.pool.clone();
    let id_clone = thread_id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = project_manager_shared::entity_embeddings::enqueue_high_priority(
            &pool,
            project_manager_shared::entity_embeddings::EntityKind::GmailThread,
            &id_clone,
        )
        .await;
    });
    result
}

#[tauri::command]
pub async fn gmail_list_labels(
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailLabelRecord>, String> {
    project_manager_shared::gmail::list_gmail_labels(&state.pool).await
}

#[tauri::command]
pub async fn gmail_category_counts(
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailCategoryCount>, String> {
    project_manager_shared::gmail::category_counts(&state.pool).await
}

#[tauri::command]
pub async fn gmail_list_drafts(
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailDraftRecord>, String> {
    project_manager_shared::gmail::list_drafts(&state.pool).await
}

#[tauri::command]
pub async fn gmail_link_thread_to_deliverable(
    thread_id: String,
    deliverable_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::link_thread_to_deliverable(
        &state.pool,
        &thread_id,
        &deliverable_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gmail_unlink_thread_from_deliverable(
    thread_id: String,
    deliverable_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::unlink_thread_from_deliverable(
        &state.pool,
        &thread_id,
        &deliverable_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gmail_link_thread_to_initiative(
    thread_id: String,
    initiative_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::link_thread_to_initiative(
        &state.pool,
        &thread_id,
        &initiative_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gmail_suggest_threads_for_deliverable(
    deliverable_id: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailLocalThread>, String> {
    project_manager_shared::gmail::suggest_threads_for_deliverable(
        &state.pool,
        &deliverable_id,
        limit.unwrap_or(8),
    )
    .await
}

#[tauri::command]
pub async fn gmail_stakeholder_threads(
    stakeholder_id: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailLocalThread>, String> {
    project_manager_shared::gmail::stakeholder_threads(
        &state.pool,
        &stakeholder_id,
        limit.unwrap_or(8),
    )
    .await
}

#[tauri::command]
pub async fn gmail_exclude_thread_from_stakeholder(
    thread_id: String,
    stakeholder_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::exclude_thread_from_stakeholder(
        &state.pool,
        &thread_id,
        &stakeholder_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gmail_stakeholder_health(
    stakeholder_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailStakeholderHealth, String> {
    project_manager_shared::gmail::stakeholder_health(&state.pool, &stakeholder_id).await
}

#[tauri::command]
pub async fn gmail_stakeholder_suggestions(
    min_threads: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailStakeholderSuggestion>, String> {
    project_manager_shared::gmail::stakeholder_suggestions(&state.pool, min_threads.unwrap_or(3))
        .await
}

#[tauri::command]
pub async fn gmail_relationship_graph(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailRelationshipEdge>, String> {
    project_manager_shared::gmail::relationship_graph(&state.pool, limit.unwrap_or(30)).await
}

#[tauri::command]
pub async fn gmail_create_capture_from_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::Capture, String> {
    let capture =
        project_manager_shared::gmail::create_capture_from_thread(&state.pool, &thread_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(capture)
}
