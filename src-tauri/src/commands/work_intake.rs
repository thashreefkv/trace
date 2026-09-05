use tauri::{AppHandle, State};

use crate::db::AppState;
use project_manager_shared::models::{
    ApproveWorkIntakeInput, WorkIntakeApplyResult, WorkIntakeFilters, WorkIntakeSuggestion,
};

#[tauri::command]
pub async fn list_work_intake_suggestions(
    filters: Option<WorkIntakeFilters>,
    state: State<'_, AppState>,
) -> Result<Vec<WorkIntakeSuggestion>, String> {
    project_manager_shared::repo::list_work_intake_suggestions(
        &state.pool,
        filters.unwrap_or_default(),
    )
    .await
}

#[tauri::command]
pub async fn generate_workspace_work_intake(
    state: State<'_, AppState>,
) -> Result<Vec<WorkIntakeSuggestion>, String> {
    let suggestions =
        project_manager_shared::repo::generate_workspace_work_intake(&state.pool).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(suggestions)
}

#[tauri::command]
pub async fn approve_work_intake_suggestion(
    input: ApproveWorkIntakeInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<WorkIntakeApplyResult, String> {
    let result =
        project_manager_shared::repo::approve_work_intake_suggestion(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    Ok(result)
}

#[tauri::command]
pub async fn dismiss_work_intake_suggestion(
    id: String,
    state: State<'_, AppState>,
) -> Result<WorkIntakeSuggestion, String> {
    let suggestion =
        project_manager_shared::repo::dismiss_work_intake_suggestion(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(suggestion)
}
