use serde::Deserialize;

use tauri::State;

use crate::db::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct GmailThreadTaskInput {
    pub thread_id: String,
    pub deliverable_id: String,
    pub title: String,
    pub due_date: Option<String>,
}

#[tauri::command]
pub async fn gmail_create_task_from_thread(
    input: GmailThreadTaskInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::DeliverableTask, String> {
    project_manager_shared::gmail::create_task_from_thread(
        &state.pool,
        &input.thread_id,
        &input.deliverable_id,
        &input.title,
        input.due_date,
    )
    .await
    .map(|task| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        task
    })
}

#[tauri::command]
pub async fn gmail_get_effective_classification(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail_intel::EffectiveClassification, String> {
    project_manager_shared::gmail_intel::effective_for_thread(&state.pool, &thread_id).await
}

#[tauri::command]
pub async fn gmail_get_thread_override(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<Option<project_manager_shared::gmail_intel::UserClassification>, String> {
    project_manager_shared::gmail_intel::get_override(&state.pool, &thread_id).await
}

#[tauri::command]
pub async fn gmail_set_thread_override(
    input: project_manager_shared::gmail_intel::SetOverrideInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail_intel::UserClassification, String> {
    let result = project_manager_shared::gmail_intel::set_override(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(result)
}

#[tauri::command]
pub async fn gmail_clear_thread_override(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail_intel::clear_override(&state.pool, &thread_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gmail_list_sender_rules(
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail_intel::SenderRule>, String> {
    project_manager_shared::gmail_intel::list_sender_rules(&state.pool).await
}

#[tauri::command]
pub async fn gmail_create_sender_rule(
    input: project_manager_shared::gmail_intel::CreateSenderRuleInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail_intel::SenderRule, String> {
    project_manager_shared::gmail_intel::create_sender_rule(&state.pool, input).await
}

#[tauri::command]
pub async fn gmail_delete_sender_rule(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail_intel::delete_sender_rule(&state.pool, &id).await
}

#[tauri::command]
pub async fn gmail_toggle_sender_rule(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail_intel::toggle_sender_rule(&state.pool, &id, enabled).await
}

#[tauri::command]
pub async fn gmail_inbox_dashboard(
    hours: Option<i64>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail_intel::InboxDashboard, String> {
    project_manager_shared::gmail_intel::inbox_dashboard(&state.pool, hours.unwrap_or(24)).await
}

#[tauri::command]
pub async fn gmail_calibration_report(
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail_intel::CalibrationReport, String> {
    project_manager_shared::gmail_intel::calibration_report(&state.pool).await
}

#[tauri::command]
pub async fn gmail_retry_failed_analyses(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err("Gemini API key not configured.".to_string());
    };
    let ids = project_manager_shared::gmail_intel::pick_failed_for_retry(
        &state.pool,
        limit.unwrap_or(10),
    )
    .await?;
    let mut retried = 0_i64;
    for thread_id in ids {
        if (project_manager_shared::gmail::analyze_thread_with_gemini(
            &api_key,
            &state.pool,
            &thread_id,
            false,
        )
        .await)
            .is_ok()
        {
            retried += 1;
        }
    }
    if retried > 0 {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
    }
    Ok(retried)
}

#[tauri::command]
pub async fn gmail_analyze_thread(
    thread_id: String,
    include_reply: Option<bool>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailAiResult, String> {
    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err("Gemini API key not configured. Add it in Settings.".to_string());
    };
    project_manager_shared::gmail::analyze_thread_with_gemini(
        &api_key,
        &state.pool,
        &thread_id,
        include_reply.unwrap_or(false),
    )
    .await
    .map(|result| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        result
    })
}

#[tauri::command]
pub async fn gmail_batch_analyze(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    project_manager_shared::gmail::batch_analyze_unsummarized_threads(
        &state.app_support_dir,
        &state.pool,
        limit.unwrap_or(100),
    )
    .await
    .map(|count| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        count
    })
}

#[tauri::command]
pub async fn gmail_reanalyze_stale_threads(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailSyncReport, String> {
    project_manager_shared::gmail::reanalyze_stale_threads(
        &state.app_support_dir,
        &state.pool,
        limit.unwrap_or(50),
    )
    .await
    .map(|report| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        report
    })
}

#[tauri::command]
pub async fn gmail_auto_link_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailAutoLinkReport, String> {
    project_manager_shared::gmail::auto_link_thread(&state.pool, &thread_id)
        .await
        .map(|report| {
            crate::commands::brain::spawn_brain_rebuild(state.inner());
            report
        })
}

#[tauri::command]
pub async fn gmail_list_orphan_threads(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailLocalThread>, String> {
    project_manager_shared::gmail::list_orphan_threads(&state.pool, limit.unwrap_or(50)).await
}

#[tauri::command]
pub async fn gmail_list_thread_link_suggestions(
    thread_id: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailThreadLinkSuggestion>, String> {
    project_manager_shared::gmail::list_thread_link_suggestions(
        &state.pool,
        thread_id.as_deref(),
        limit.unwrap_or(50),
    )
    .await
}

#[tauri::command]
pub async fn gmail_accept_thread_link(
    suggestion_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::accept_thread_link(&state.pool, &suggestion_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gmail_reject_thread_link(
    suggestion_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::reject_thread_link(&state.pool, &suggestion_id).await
}

#[tauri::command]
pub async fn gmail_generate_work_intake(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::models::WorkIntakeSuggestion>, String> {
    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err("Gemini API key not configured. Add it in Settings.".to_string());
    };
    project_manager_shared::gmail::analyze_thread_with_gemini(
        &api_key,
        &state.pool,
        &thread_id,
        false,
    )
    .await?;
    let suggestions = project_manager_shared::repo::list_work_intake_suggestions(
        &state.pool,
        project_manager_shared::models::WorkIntakeFilters {
            source_kind: Some("gmail".to_string()),
            source_id: Some(thread_id),
            status: Some("pending".to_string()),
            limit: Some(50),
        },
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(suggestions)
}
