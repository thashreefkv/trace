use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex as StdMutex},
};

use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex as AsyncMutex, Notify};
use tokio_util::sync::CancellationToken;

// SQL schema constants live in `project_manager_shared::db` and are applied
// by `project_manager_shared::db::apply_migrations` during `connect` below.

pub struct AppState {
    pub pool: SqlitePool,
    pub app_support_dir: PathBuf,
    pub database_path: PathBuf,
    pub brain_path: PathBuf,
    pub brain_rebuild_lock: Arc<AsyncMutex<()>>,
    pub brain_dirty: Arc<AtomicBool>,
    pub brain_rebuild_notify: Arc<Notify>,
    pub ask_runs: Arc<StdMutex<std::collections::HashMap<String, CancellationToken>>>,
    pub ask_confirmations: project_manager_shared::models::AskConfirmationMap,
    pub report_clarifications: project_manager_shared::reports::ClarificationRegistry,
    pub app_handle: AppHandle,
}

pub async fn connect(app: &AppHandle) -> Result<AppState, String> {
    let app_support_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("failed to locate app config directory: {error}"))?;
    std::fs::create_dir_all(&app_support_dir)
        .map_err(|error| format!("failed to create app config directory: {error}"))?;
    project_manager_shared::keychain::harden_private_dir(&app_support_dir)?;

    let database_path = app_support_dir.join(project_manager_shared::db::DB_FILE_NAME);
    let brain_path = app_support_dir.join(project_manager_shared::brain::BRAIN_FILE_NAME);
    let pool = project_manager_shared::db::connect_path(&database_path).await?;

    Ok(AppState {
        pool,
        app_support_dir,
        database_path,
        brain_path,
        brain_rebuild_lock: Arc::new(AsyncMutex::new(())),
        brain_dirty: Arc::new(AtomicBool::new(false)),
        brain_rebuild_notify: Arc::new(Notify::new()),
        ask_runs: Arc::new(StdMutex::new(std::collections::HashMap::new())),
        ask_confirmations: Arc::new(StdMutex::new(std::collections::HashMap::new())),
        report_clarifications: project_manager_shared::reports::new_clarification_registry(),
        app_handle: app.clone(),
    })
}
