use std::{net::TcpListener, path::PathBuf, sync::Arc};

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex as AsyncMutex;

#[tauri::command]
pub async fn open_external_url(url: String, app: AppHandle) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|_| "invalid external URL".to_string())?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("only public HTTPS links can be opened".to_string());
    }
    app.opener()
        .open_url(parsed.as_str(), None::<String>)
        .map_err(|e| format!("failed to open URL: {e}"))
}

use crate::db::AppState;
use project_manager_shared::models::GCalStatus;

#[tauri::command]
pub async fn gcal_status(state: State<'_, AppState>) -> Result<GCalStatus, String> {
    Ok(GCalStatus {
        connected: project_manager_shared::google_calendar::calendar_connected(
            &state.app_support_dir,
        ),
    })
}

#[tauri::command]
pub async fn gcal_connect(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GCalStatus, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("failed to bind local server: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to get local address: {e}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");

    let flow = project_manager_shared::google_calendar::build_auth_url(&redirect_uri)?;

    app.opener()
        .open_url(&flow.auth_url, None::<String>)
        .map_err(|e| format!("failed to open browser: {e}"))?;

    // Wait for the OAuth callback on the local TCP listener.
    const SUCCESS_HTML: &str = "<html><body><h2>Google Calendar connected!</h2>\
<p>You can close this tab and return to Trace.</p></body></html>";

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
    .map_err(|e| format!("OAuth task panicked: {e}"))??;

    project_manager_shared::google_calendar::complete_oauth(
        &state.app_support_dir,
        &code,
        &redirect_uri,
        &code_verifier,
    )
    .await?;

    // Kick off an initial sync in the background
    let dir = state.app_support_dir.clone();
    let pool = state.pool.clone();
    let brain_path = state.brain_path.clone();
    let brain_lock = state.brain_rebuild_lock.clone();
    tauri::async_runtime::spawn(async move {
        if project_manager_shared::google_calendar::sync_calendar(&pool, &dir)
            .await
            .is_ok()
        {
            let _guard = brain_lock.lock().await;
            let _ = project_manager_shared::brain::rebuild_brain(&pool, &brain_path).await;
        }
    });

    Ok(GCalStatus { connected: true })
}

#[tauri::command]
pub async fn gcal_disconnect(state: State<'_, AppState>) -> Result<GCalStatus, String> {
    project_manager_shared::google_calendar::calendar_disconnect(&state.app_support_dir)?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(GCalStatus { connected: false })
}

#[tauri::command]
pub async fn gcal_sync(state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::google_calendar::sync_calendar(&state.pool, &state.app_support_dir)
        .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn gcal_stakeholder_events(
    email: String,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::models::GCalEvent>, String> {
    project_manager_shared::google_calendar::get_stakeholder_calendar_events(&state.pool, &email)
        .await
}

#[derive(serde::Deserialize)]
pub struct CreateGcalMeetingInput {
    pub title: String,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub time_zone: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub attendees: Vec<String>,
    pub add_meet: bool,
    pub zoom_url: Option<String>,
}

#[tauri::command]
pub async fn create_gcal_meeting(
    input: CreateGcalMeetingInput,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let event = project_manager_shared::google_calendar::create_gcal_meeting(
        &state.pool,
        &state.app_support_dir,
        &input.title,
        &input.date,
        &input.start_time,
        &input.end_time,
        input.time_zone.as_deref().unwrap_or("Asia/Kolkata"),
        input.description.as_deref(),
        input.location.as_deref(),
        input.attendees,
        input.add_meet,
        input.zoom_url.as_deref(),
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(event)
}

#[tauri::command]
pub async fn delete_gcal_meeting(
    gcal_event_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let result = project_manager_shared::google_calendar::tool_delete_calendar_event(
        &state.pool,
        &state.app_support_dir,
        &gcal_event_id,
    )
    .await;
    if result.get("error").is_some() {
        return Err(result["error"]
            .as_str()
            .unwrap_or("delete failed")
            .to_string());
    }
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(result)
}

pub fn spawn_calendar_background_sync(
    app: tauri::AppHandle,
    pool: sqlx::SqlitePool,
    app_support_dir: PathBuf,
    brain_path: PathBuf,
    brain_rebuild_lock: Arc<AsyncMutex<()>>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            if project_manager_shared::google_calendar::calendar_connected(&app_support_dir) {
                crate::bg_events::emit_started(&app, crate::bg_events::SOURCE_CALENDAR);
                match project_manager_shared::google_calendar::sync_calendar(
                    &pool,
                    &app_support_dir,
                )
                .await
                {
                    Ok(_) => {
                        crate::bg_events::emit_finished(
                            &app,
                            crate::bg_events::SOURCE_CALENDAR,
                            None,
                        );
                        let _guard = brain_rebuild_lock.lock().await;
                        let _ =
                            project_manager_shared::brain::rebuild_brain(&pool, &brain_path).await;
                    }
                    Err(error) => {
                        crate::bg_events::emit_error(
                            &app,
                            crate::bg_events::SOURCE_CALENDAR,
                            error,
                        );
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(15 * 60)).await;
        }
    });
}
