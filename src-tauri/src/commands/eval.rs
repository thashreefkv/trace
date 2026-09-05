use tauri::State;

use crate::db::AppState;
use project_manager_shared::eval::{
    self, CreateFixtureInput, EvalFixture, EvalRun, EvalSummary, ImportFixturesInput,
    ImportFixturesResult,
};

#[tauri::command]
pub async fn list_eval_fixtures(state: State<'_, AppState>) -> Result<Vec<EvalFixture>, String> {
    eval::list_fixtures(&state.pool).await
}

#[tauri::command]
pub async fn create_eval_fixture(
    input: CreateFixtureInput,
    state: State<'_, AppState>,
) -> Result<EvalFixture, String> {
    eval::create_fixture(&state.pool, input).await
}

#[tauri::command]
pub async fn delete_eval_fixture(id: String, state: State<'_, AppState>) -> Result<(), String> {
    eval::delete_fixture(&state.pool, &id).await
}

#[tauri::command]
pub async fn run_eval_fixture(
    fixture_id: String,
    state: State<'_, AppState>,
) -> Result<EvalRun, String> {
    let fixtures = eval::list_fixtures(&state.pool).await?;
    let fixture = fixtures
        .into_iter()
        .find(|f| f.id == fixture_id)
        .ok_or_else(|| format!("fixture {fixture_id} not found"))?;
    eval::run_fixture(&state.pool, &state.brain_path, &fixture).await
}

#[tauri::command]
pub async fn run_all_evals(state: State<'_, AppState>) -> Result<Vec<EvalRun>, String> {
    eval::run_all(&state.pool, &state.brain_path).await
}

#[tauri::command]
pub async fn list_eval_runs(
    fixture_id: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<EvalRun>, String> {
    let limit = limit.unwrap_or(50);
    match fixture_id {
        Some(id) => eval::list_runs_for_fixture(&state.pool, &id, limit).await,
        None => eval::latest_runs(&state.pool, limit).await,
    }
}

#[tauri::command]
pub async fn set_eval_baseline(
    fixture_id: String,
    run_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    eval::set_baseline(&state.pool, &fixture_id, &run_id).await
}

#[tauri::command]
pub async fn get_eval_summary(state: State<'_, AppState>) -> Result<EvalSummary, String> {
    eval::summary(&state.pool).await
}

#[tauri::command]
pub async fn import_eval_fixtures(
    input: ImportFixturesInput,
    state: State<'_, AppState>,
) -> Result<ImportFixturesResult, String> {
    eval::import_fixtures(&state.pool, input).await
}
