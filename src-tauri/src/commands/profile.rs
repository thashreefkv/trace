use crate::db::AppState;
use project_manager_shared::{
    models::{UpdateUserProfileInput, UserProfile},
    repo,
};
use tauri::State;

#[tauri::command]
pub async fn get_user_profile(state: State<'_, AppState>) -> Result<UserProfile, String> {
    repo::get_user_profile(&state.pool).await
}

#[tauri::command]
pub async fn update_user_profile(
    state: State<'_, AppState>,
    input: UpdateUserProfileInput,
) -> Result<UserProfile, String> {
    repo::update_user_profile(&state.pool, input).await
}
