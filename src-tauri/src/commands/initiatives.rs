use tauri::State;

use crate::{
    db::AppState,
    models::{CreateInitiativeInput, Initiative, UpdateInitiativeInput},
};

#[tauri::command]
pub async fn list_initiatives(state: State<'_, AppState>) -> Result<Vec<Initiative>, String> {
    project_manager_shared::repo::list_initiatives(&state.pool).await
}

#[tauri::command]
pub async fn get_initiative(id: String, state: State<'_, AppState>) -> Result<Initiative, String> {
    let result = project_manager_shared::repo::get_initiative(&state.pool, &id).await;
    let pool = state.pool.clone();
    let id_clone = id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = project_manager_shared::entity_embeddings::enqueue_high_priority(
            &pool,
            project_manager_shared::entity_embeddings::EntityKind::Initiative,
            &id_clone,
        )
        .await;
    });
    result
}

#[tauri::command]
pub async fn create_initiative(
    input: CreateInitiativeInput,
    state: State<'_, AppState>,
) -> Result<Initiative, String> {
    let initiative = project_manager_shared::repo::create_initiative(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(initiative)
}

#[tauri::command]
pub async fn update_initiative(
    id: String,
    input: UpdateInitiativeInput,
    state: State<'_, AppState>,
) -> Result<Initiative, String> {
    let initiative =
        project_manager_shared::repo::update_initiative(&state.pool, &id, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(initiative)
}

#[tauri::command]
pub async fn delete_initiative(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_initiative(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}
