use std::net::TcpListener;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::{emit_new_mail_if_needed, GmailStatus};
use crate::db::AppState;

#[tauri::command]
pub async fn gmail_status(state: State<'_, AppState>) -> Result<GmailStatus, String> {
    let settings = project_manager_shared::gmail::get_sync_settings(&state.pool)
        .await
        .ok();
    Ok(GmailStatus {
        connected: project_manager_shared::gmail::gmail_connected(&state.app_support_dir),
        settings,
    })
}

#[tauri::command]
pub async fn gmail_connect(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GmailStatus, String> {
    // Bind on a random port so Google can redirect back to us
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("failed to bind local server: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to get local address: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let flow = project_manager_shared::gmail::build_auth_url(&redirect_uri)?;

    app.opener()
        .open_url(&flow.auth_url, None::<String>)
        .map_err(|e| format!("failed to open browser: {e}"))?;

    // Block until Google redirects back with the auth code.
    let expected_state = flow.state;
    let code_verifier = flow.code_verifier;
    let code = tokio::task::spawn_blocking(move || {
        project_manager_shared::oauth::wait_for_oauth_redirect(listener, &expected_state, None)
    })
    .await
    .map_err(|e| format!("OAuth listener task failed: {e}"))??;

    project_manager_shared::gmail::complete_oauth(
        &state.app_support_dir,
        &code,
        &redirect_uri,
        &code_verifier,
    )
    .await?;

    Ok(GmailStatus {
        connected: true,
        settings: project_manager_shared::gmail::get_sync_settings(&state.pool)
            .await
            .ok(),
    })
}

#[tauri::command]
pub async fn gmail_disconnect(state: State<'_, AppState>) -> Result<GmailStatus, String> {
    project_manager_shared::gmail::gmail_disconnect(&state.app_support_dir)?;
    Ok(GmailStatus {
        connected: false,
        settings: project_manager_shared::gmail::get_sync_settings(&state.pool)
            .await
            .ok(),
    })
}

#[tauri::command]
pub async fn gmail_search_threads(
    query: String,
    max_results: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailThread>, String> {
    project_manager_shared::gmail::search_threads(
        &state.app_support_dir,
        &query,
        max_results.unwrap_or(10),
    )
    .await
}

#[tauri::command]
pub async fn gmail_get_sync_settings(
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailSyncSettings, String> {
    project_manager_shared::gmail::get_sync_settings(&state.pool).await
}

#[tauri::command]
pub async fn gmail_update_sync_settings(
    input: project_manager_shared::gmail::GmailSyncSettingsInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailSyncSettings, String> {
    project_manager_shared::gmail::update_sync_settings(&state.pool, input).await
}

#[tauri::command]
pub async fn gmail_sync_now(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gmail::GmailSyncReport, String> {
    let report =
        project_manager_shared::gmail::sync_mailbox(&state.app_support_dir, &state.pool).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    emit_new_mail_if_needed(&app, &report).await;
    Ok(report)
}

#[tauri::command]
pub async fn gmail_list_local_threads(
    filters: Option<project_manager_shared::gmail::GmailThreadFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::gmail::GmailLocalThread>, String> {
    project_manager_shared::gmail::list_local_threads(&state.pool, filters.unwrap_or_default())
        .await
}
