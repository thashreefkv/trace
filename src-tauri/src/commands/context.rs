use tauri::State;

use crate::{
    db::AppState,
    models::{WorkGraph, WorkGraphFilters},
};

#[tauri::command]
pub async fn get_work_context_graph(
    filters: Option<WorkGraphFilters>,
    state: State<'_, AppState>,
) -> Result<WorkGraph, String> {
    project_manager_shared::brain::get_work_context_graph(
        &state.pool,
        &state.brain_path,
        filters.unwrap_or_default(),
    )
    .await
}
