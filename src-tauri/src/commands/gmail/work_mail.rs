use tauri::State;

use crate::db::AppState;

#[tauri::command]
pub async fn gmail_list_work_mail_threads(
    query: Option<project_manager_shared::gmail::WorkMailQuery>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailLocalThread>, String> {
    project_manager_shared::gmail::list_work_mail_threads(&state.pool, query.unwrap_or_default())
        .await
}

#[tauri::command]
pub async fn gmail_work_mail_view_counts(
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::WorkMailViewCounts, String> {
    project_manager_shared::gmail::work_mail_view_counts(&state.pool).await
}

#[tauri::command]
pub async fn gmail_work_mail_brief(
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::WorkMailBrief, String> {
    project_manager_shared::gmail::work_mail_brief(&state.pool).await
}

#[tauri::command]
pub async fn gmail_list_work_mail_domains(
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::WorkMailDomain>, String> {
    project_manager_shared::gmail::list_work_mail_domains(&state.pool).await
}

#[tauri::command]
pub async fn gmail_upsert_work_mail_domain(
    input: project_manager_shared::gmail::UpsertWorkMailDomainInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::WorkMailDomain, String> {
    project_manager_shared::gmail::upsert_work_mail_domain(&state.pool, input).await
}

#[tauri::command]
pub async fn gmail_delete_work_mail_domain(
    domain: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::delete_work_mail_domain(&state.pool, &domain).await
}

#[tauri::command]
pub async fn gmail_list_work_mail_agent_events(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::WorkMailAgentEvent>, String> {
    project_manager_shared::gmail::list_work_mail_agent_events(&state.pool, limit.unwrap_or(50))
        .await
}

#[tauri::command]
pub async fn gmail_mark_work_mail_thread_seen(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::WorkMailReviewSummary, String> {
    project_manager_shared::gmail::mark_work_mail_thread_seen(&state.pool, &thread_id).await
}

#[tauri::command]
pub async fn gmail_set_work_mail_review_state(
    thread_id: String,
    input: project_manager_shared::gmail::WorkMailReviewUpdate,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::WorkMailReviewSummary, String> {
    project_manager_shared::gmail::set_work_mail_review_state(&state.pool, &thread_id, input).await
}

#[tauri::command]
pub async fn gmail_defer_work_mail_thread(
    thread_id: String,
    deferred_until: Option<String>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::WorkMailReviewSummary, String> {
    project_manager_shared::gmail::defer_work_mail_thread(&state.pool, &thread_id, deferred_until)
        .await
}

#[tauri::command]
pub async fn gmail_reopen_work_mail_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::WorkMailReviewSummary, String> {
    project_manager_shared::gmail::reopen_work_mail_thread(&state.pool, &thread_id).await
}

#[tauri::command]
pub async fn gmail_promote_work_mail_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail_intel::UserClassification, String> {
    project_manager_shared::gmail::promote_work_mail_thread(&state.pool, &thread_id).await
}

#[tauri::command]
pub async fn gmail_restore_work_mail_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail_intel::UserClassification, String> {
    project_manager_shared::gmail::restore_work_mail_thread(&state.pool, &thread_id).await
}

#[tauri::command]
pub async fn gmail_exclude_work_mail_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail_intel::UserClassification, String> {
    project_manager_shared::gmail::exclude_work_mail_thread(&state.pool, &thread_id).await
}
