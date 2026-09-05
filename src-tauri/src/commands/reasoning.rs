use tauri::State;

use crate::db::AppState;
use crate::models::{
    AgentReviewItem, ClaimReviewItem, CommunityReport, ReasoningIndexSummary, ReasoningQueryInput,
    ReasoningResult, ReasoningRun,
};

fn api_key(state: &AppState) -> Result<String, String> {
    project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| "Gemini API key not configured. Add it in Settings.".to_string())
}

#[tauri::command]
pub async fn query_reasoning_graph(
    input: ReasoningQueryInput,
    state: State<'_, AppState>,
) -> Result<ReasoningResult, String> {
    super::brain::ensure_brain_fresh(state.inner()).await?;
    project_manager_shared::reasoning::query_reasoning_graph(
        &state.pool,
        &state.brain_path,
        &api_key(state.inner())?,
        input,
    )
    .await
}

#[tauri::command]
pub async fn refresh_reasoning_index(
    state: State<'_, AppState>,
) -> Result<ReasoningIndexSummary, String> {
    let mut result =
        project_manager_shared::reasoning::refresh_reasoning_index(&state.pool).await?;
    if let Some(key) = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    {
        result.candidate_claims_staged =
            project_manager_shared::reasoning::extract_candidate_claims(&state.pool, &key).await?;
    }
    super::brain::spawn_brain_rebuild(state.inner());
    Ok(result)
}

#[tauri::command]
pub async fn list_reasoning_runs(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<ReasoningRun>, String> {
    project_manager_shared::reasoning::list_reasoning_runs(&state.pool, limit).await
}

#[tauri::command]
pub async fn list_claim_review_items(
    status: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ClaimReviewItem>, String> {
    project_manager_shared::reasoning::list_claim_review_items(&state.pool, status.as_deref()).await
}

#[tauri::command]
pub async fn review_claim(
    claim_id: String,
    decision: String,
    state: State<'_, AppState>,
) -> Result<ClaimReviewItem, String> {
    let result =
        project_manager_shared::reasoning::review_claim(&state.pool, &claim_id, &decision).await?;
    super::brain::spawn_brain_rebuild(state.inner());
    Ok(result)
}

#[tauri::command]
pub async fn list_community_reports(
    status: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<CommunityReport>, String> {
    project_manager_shared::reasoning::list_community_reports(&state.pool, status.as_deref()).await
}

#[tauri::command]
pub async fn review_community_report(
    report_id: String,
    decision: String,
    state: State<'_, AppState>,
) -> Result<CommunityReport, String> {
    let result = project_manager_shared::reasoning::review_community_report(
        &state.pool,
        &report_id,
        &decision,
    )
    .await?;
    super::brain::spawn_brain_rebuild(state.inner());
    Ok(result)
}

#[tauri::command]
pub async fn list_reasoning_review_items(
    status: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<AgentReviewItem>, String> {
    project_manager_shared::reasoning::list_review_items(&state.pool, status.as_deref()).await
}

#[tauri::command]
pub async fn review_reasoning_item(
    review_item_id: String,
    decision: String,
    state: State<'_, AppState>,
) -> Result<AgentReviewItem, String> {
    project_manager_shared::reasoning::review_item(&state.pool, &review_item_id, &decision).await
}
