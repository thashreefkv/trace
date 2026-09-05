use tauri::State;

use crate::{
    db::AppState,
    models::{GeneratedWeekPlan, MeetingConfig, SetDayInput, WeekView},
};

#[tauri::command]
pub async fn get_week_view(
    week_start: String,
    state: State<'_, AppState>,
) -> Result<WeekView, String> {
    project_manager_shared::repo::get_week_view(&state.pool, &week_start, &state.app_support_dir)
        .await
}

#[tauri::command]
pub async fn set_day_deliverable(
    input: SetDayInput,
    state: State<'_, AppState>,
) -> Result<WeekView, String> {
    project_manager_shared::repo::set_day_deliverable(
        &state.pool,
        &input.week_start,
        input.day_index,
        input.deliverable_id.as_deref(),
        &state.app_support_dir,
    )
    .await
    .map(|view| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        view
    })
}

#[tauri::command]
pub async fn get_meeting_config(state: State<'_, AppState>) -> Result<MeetingConfig, String> {
    project_manager_shared::repo::get_meeting_config(&state.pool).await
}

#[tauri::command]
pub async fn set_meeting_date(
    date: Option<String>,
    state: State<'_, AppState>,
) -> Result<MeetingConfig, String> {
    let config =
        project_manager_shared::repo::set_meeting_date(&state.pool, date.as_deref()).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(config)
}

#[tauri::command]
pub async fn advance_meeting_date(state: State<'_, AppState>) -> Result<MeetingConfig, String> {
    let config = project_manager_shared::repo::advance_meeting_date(&state.pool).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(config)
}

#[tauri::command]
pub async fn generate_week_plan(
    week_start: String,
    state: State<'_, AppState>,
) -> Result<GeneratedWeekPlan, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| "Gemini API key not set. Configure it in Settings.".to_string())?;

    let filters = project_manager_shared::models::DeliverableFilters {
        state: Some(project_manager_shared::models::DeliverableState::Drafting),
        ..Default::default()
    };
    let mut deliverables =
        project_manager_shared::repo::list_deliverables(&state.pool, filters).await?;

    let filters_in_review = project_manager_shared::models::DeliverableFilters {
        state: Some(project_manager_shared::models::DeliverableState::InReview),
        ..Default::default()
    };
    let in_review =
        project_manager_shared::repo::list_deliverables(&state.pool, filters_in_review).await?;
    deliverables.extend(in_review);

    let meeting_config = project_manager_shared::repo::get_meeting_config(&state.pool).await?;

    project_manager_shared::gemini::generate_week_plan(
        &state.pool,
        &api_key,
        &week_start,
        meeting_config.next_meeting_date.as_deref(),
        &deliverables,
    )
    .await
}
