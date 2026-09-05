use std::{net::TcpListener, path::PathBuf, sync::Arc};

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex as AsyncMutex;

use crate::{db::AppState, models::FileRow};

#[derive(Serialize)]
pub struct DriveStatus {
    pub connected: bool,
    pub accounts: Vec<project_manager_shared::google_drive::DriveAccount>,
}

#[tauri::command]
pub async fn drive_status(state: State<'_, AppState>) -> Result<DriveStatus, String> {
    let connected = project_manager_shared::google_drive::drive_connected(&state.app_support_dir);
    let accounts = project_manager_shared::google_drive::list_accounts(&state.pool).await?;
    Ok(DriveStatus {
        connected,
        accounts,
    })
}

#[tauri::command]
pub async fn drive_connect(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::google_drive::DriveAccount, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("failed to bind local server: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to get local address: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let flow = project_manager_shared::google_drive::build_auth_url(&redirect_uri)?;

    app.opener()
        .open_url(&flow.auth_url, None::<String>)
        .map_err(|e| format!("failed to open browser: {e}"))?;

    const SUCCESS_HTML: &str = "<!doctype html><html lang='en'><head><meta charset='utf-8'><title>Trace connected</title></head><body><main><h1>Drive connected to Trace</h1><p>You can close this tab and return to the app.</p></main></body></html>";

    let expected_state = flow.state;
    let code_verifier = flow.code_verifier;
    let code = tokio::task::spawn_blocking(move || {
        project_manager_shared::oauth::wait_for_oauth_redirect(
            listener,
            &expected_state,
            Some(SUCCESS_HTML),
        )
    })
    .await
    .map_err(|e| format!("OAuth listener task failed: {e}"))??;

    let account = project_manager_shared::google_drive::complete_oauth(
        &state.pool,
        &state.app_support_dir,
        &code,
        &redirect_uri,
        &code_verifier,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(account)
}

#[tauri::command]
pub async fn drive_disconnect(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::google_drive::disconnect_account(
        &state.pool,
        &state.app_support_dir,
        &account_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn drive_list_children(
    parent_id: Option<String>,
    page_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::google_drive::DriveListing, String> {
    project_manager_shared::google_drive::list_children(
        &state.app_support_dir,
        parent_id.as_deref(),
        page_token.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn drive_import(
    account_id: String,
    drive_file_ids: Vec<String>,
    trace_folder_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<FileRow>, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?;
    let files = project_manager_shared::google_drive::import_files(
        &state.pool,
        &state.app_support_dir,
        api_key.as_deref(),
        &account_id,
        &drive_file_ids,
        trace_folder_id.as_deref(),
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(files)
}

#[tauri::command]
pub async fn drive_import_folder(
    account_id: String,
    drive_folder_id: String,
    folder_name: String,
    trace_folder_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::TraceFolder, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?;
    let folder = project_manager_shared::google_drive::import_drive_folder(
        &state.pool,
        &state.app_support_dir,
        api_key.as_deref(),
        &account_id,
        &drive_folder_id,
        &folder_name,
        trace_folder_id.as_deref(),
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(folder)
}

#[tauri::command]
pub async fn drive_pull_changes(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?;
    let count = project_manager_shared::google_drive::pull_changes(
        &state.pool,
        &state.app_support_dir,
        api_key.as_deref(),
        &account_id,
    )
    .await?;
    if count > 0 {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
    }
    Ok(count)
}

#[tauri::command]
pub async fn drive_sync_status(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::google_drive::DriveSyncStatus, String> {
    project_manager_shared::google_drive::get_sync_status(&state.pool, &account_id).await
}

#[tauri::command]
pub async fn drive_has_editor_scope(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(project_manager_shared::google_drive::has_editor_scope(
        &state.app_support_dir,
    ))
}

#[tauri::command]
pub async fn drive_get_file_metadata(
    file_id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::google_drive::DriveFile, String> {
    project_manager_shared::google_drive::get_metadata(&state.app_support_dir, &file_id).await
}

#[tauri::command]
pub async fn get_gmeet_folder(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<Option<project_manager_shared::google_drive::GmeetFolder>, String> {
    project_manager_shared::google_drive::get_gmeet_folder(&state.pool, &account_id).await
}

#[tauri::command]
pub async fn set_gmeet_folder(
    account_id: String,
    folder_id: String,
    folder_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::google_drive::set_gmeet_folder(
        &state.pool,
        &account_id,
        &folder_id,
        &folder_name,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn clear_gmeet_folder(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::google_drive::clear_gmeet_folder(&state.pool, &account_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

pub fn spawn_drive_background_sync(
    app: tauri::AppHandle,
    pool: sqlx::SqlitePool,
    app_support_dir: PathBuf,
    brain_path: PathBuf,
    brain_rebuild_lock: Arc<AsyncMutex<()>>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            let api_key = project_manager_shared::keychain::get_gemini_api_key(&app_support_dir)
                .ok()
                .flatten();
            match project_manager_shared::google_drive::list_accounts(&pool).await {
                Ok(accounts) => {
                    if !accounts.is_empty() {
                        crate::bg_events::emit_started(&app, crate::bg_events::SOURCE_DRIVE);
                        let mut total_changes: u32 = 0;
                        let mut first_error: Option<String> = None;
                        for account in accounts {
                            match project_manager_shared::google_drive::pull_changes(
                                &pool,
                                &app_support_dir,
                                api_key.as_deref(),
                                &account.id,
                            )
                            .await
                            {
                                Ok(count) => {
                                    total_changes += count;
                                    if count > 0 {
                                        let _guard = brain_rebuild_lock.lock().await;
                                        let _ = project_manager_shared::brain::rebuild_brain(
                                            &pool,
                                            &brain_path,
                                        )
                                        .await;
                                    }
                                }
                                Err(error) if first_error.is_none() => {
                                    first_error = Some(error);
                                }
                                Err(_) => {}
                            }
                        }
                        if let Some(error) = first_error {
                            crate::bg_events::emit_error(
                                &app,
                                crate::bg_events::SOURCE_DRIVE,
                                error,
                            );
                        } else {
                            let summary = if total_changes > 0 {
                                Some(format!(
                                    "{} change{}",
                                    total_changes,
                                    if total_changes == 1 { "" } else { "s" }
                                ))
                            } else {
                                None
                            };
                            crate::bg_events::emit_finished(
                                &app,
                                crate::bg_events::SOURCE_DRIVE,
                                summary,
                            );
                        }
                    }
                }
                Err(error) => {
                    crate::bg_events::emit_error(&app, crate::bg_events::SOURCE_DRIVE, error);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(180)).await;
        }
    });
}
