use tauri::State;

use crate::db::AppState;

#[tauri::command]
pub async fn gmail_triage_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailTriageResult, String> {
    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err("Gemini API key not configured. Add it in Settings.".to_string());
    };
    project_manager_shared::gmail::triage_thread_with_gemini(&api_key, &state.pool, &thread_id)
        .await
        .map(|result| {
            crate::commands::brain::spawn_brain_rebuild(state.inner());
            result
        })
}

#[tauri::command]
pub async fn gmail_weekly_digest(
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailWeeklyDigest, String> {
    project_manager_shared::gmail::weekly_digest(&state.pool).await
}

#[tauri::command]
pub async fn gmail_send_email(
    input: project_manager_shared::gmail::GmailSendInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailSendResult, String> {
    project_manager_shared::gmail::send_email(&state.app_support_dir, &state.pool, input).await
}

#[tauri::command]
pub async fn gmail_archive_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::archive_thread(&state.app_support_dir, &state.pool, &thread_id)
        .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gmail_move_thread_to_spam(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::gmail::move_thread_to_spam(
        &state.app_support_dir,
        &state.pool,
        &thread_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gmail_mark_thread_important(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailThreadDetail, String> {
    project_manager_shared::gmail::mark_thread_important(
        &state.app_support_dir,
        &state.pool,
        &thread_id,
    )
    .await
    .map(|detail| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        detail
    })
}

#[tauri::command]
pub async fn gmail_star_thread(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailThreadDetail, String> {
    project_manager_shared::gmail::star_thread(&state.app_support_dir, &state.pool, &thread_id)
        .await
        .map(|detail| {
            crate::commands::brain::spawn_brain_rebuild(state.inner());
            detail
        })
}

#[tauri::command]
pub async fn gmail_mark_thread_read_in_gmail(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailThreadDetail, String> {
    project_manager_shared::gmail::mark_thread_read_in_gmail(
        &state.app_support_dir,
        &state.pool,
        &thread_id,
    )
    .await
    .map(|detail| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        detail
    })
}

#[tauri::command]
pub async fn gmail_mark_thread_unread_in_gmail(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailThreadDetail, String> {
    project_manager_shared::gmail::mark_thread_unread_in_gmail(
        &state.app_support_dir,
        &state.pool,
        &thread_id,
    )
    .await
    .map(|detail| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        detail
    })
}

// ============================================================================
// Local-first email draft commands
// ============================================================================

#[tauri::command]
pub async fn gmail_get_local_draft(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<Option<project_manager_shared::local_drafts::LocalEmailDraft>, String> {
    project_manager_shared::local_drafts::get_draft_for_thread(&state.pool, &thread_id).await
}

#[tauri::command]
pub async fn gmail_save_local_draft(
    input: project_manager_shared::local_drafts::SaveLocalDraftInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::local_drafts::LocalEmailDraft, String> {
    project_manager_shared::local_drafts::save_draft(&state.pool, input).await
}

#[tauri::command]
pub async fn gmail_delete_local_draft(
    draft_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::local_drafts::delete_draft(
        &state.pool,
        &state.app_support_dir,
        &draft_id,
    )
    .await
}

#[tauri::command]
pub async fn gmail_add_draft_attachment(
    draft_id: String,
    source_path: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::local_drafts::LocalEmailDraftAttachment, String> {
    let path = std::path::PathBuf::from(&source_path);
    project_manager_shared::local_drafts::add_attachment(
        &state.pool,
        &state.app_support_dir,
        &draft_id,
        &path,
    )
    .await
}

#[tauri::command]
pub async fn gmail_remove_draft_attachment(
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::local_drafts::remove_attachment(&state.pool, &attachment_id).await
}

// ============================================================================
// Agentic AI draft (memory + brain + thread context)
// ============================================================================

#[tauri::command]
pub async fn gmail_list_analysis_history(
    thread_id: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailAnalysisSnapshot>, String> {
    project_manager_shared::gmail::list_analysis_history(
        &state.pool,
        &thread_id,
        limit.unwrap_or(20),
    )
    .await
}

#[tauri::command]
pub async fn gmail_draft_reply_with_brain(
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| {
            "Gemini API key is not configured. Open Settings and add your key.".to_string()
        })?;
    project_manager_shared::gmail::draft_reply_with_brain(
        &api_key,
        &state.pool,
        &state.brain_path,
        &thread_id,
    )
    .await
}
