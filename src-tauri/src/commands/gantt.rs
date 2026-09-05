use tauri::State;

use crate::db::AppState;
use project_manager_shared::models::{
    CreateSectionInput, InitiativeGantt, InitiativeSection, UpdateGanttDatesInput,
    UpdateSectionInput,
};

#[tauri::command]
pub async fn get_initiative_gantt(
    initiative_id: String,
    state: State<'_, AppState>,
) -> Result<InitiativeGantt, String> {
    project_manager_shared::repo::get_initiative_gantt(&state.pool, &initiative_id).await
}

#[tauri::command]
pub async fn create_initiative_section(
    input: CreateSectionInput,
    state: State<'_, AppState>,
) -> Result<InitiativeSection, String> {
    let section =
        project_manager_shared::repo::create_initiative_section(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(section)
}

#[tauri::command]
pub async fn update_initiative_section(
    id: String,
    input: UpdateSectionInput,
    state: State<'_, AppState>,
) -> Result<InitiativeSection, String> {
    let section =
        project_manager_shared::repo::update_initiative_section(&state.pool, &id, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(section)
}

#[tauri::command]
pub async fn delete_initiative_section(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::repo::delete_initiative_section(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn update_deliverable_gantt_dates(
    id: String,
    input: UpdateGanttDatesInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::repo::update_deliverable_gantt_dates(&state.pool, &id, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn set_deliverable_section(
    deliverable_id: String,
    section_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::repo::set_deliverable_section(
        &state.pool,
        &deliverable_id,
        section_id.as_deref(),
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}
