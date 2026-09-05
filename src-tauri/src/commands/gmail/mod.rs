mod actions;
mod analysis;
mod links;
mod sync;
mod work_mail;

pub use actions::*;
pub use analysis::*;
pub use links::*;
pub use sync::*;
pub use work_mail::*;

use std::{path::PathBuf, sync::Arc, time::Duration};

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

#[derive(Serialize)]
pub struct GmailStatus {
    pub connected: bool,
    pub settings: Option<project_manager_shared::gmail::GmailSyncSettings>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailNewMailEvent {
    pub new_messages: i64,
    pub new_threads: i64,
}

pub fn spawn_background_sync(
    app: AppHandle,
    pool: SqlitePool,
    app_support_dir: PathBuf,
    brain_path: PathBuf,
    brain_rebuild_lock: Arc<tokio::sync::Mutex<()>>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            let poll_minutes = project_manager_shared::gmail::get_sync_settings(&pool)
                .await
                .map(|settings| settings.notification_poll_minutes.clamp(1, 240))
                .unwrap_or(5);
            tokio::time::sleep(Duration::from_secs((poll_minutes * 60) as u64)).await;
            if !project_manager_shared::gmail::gmail_connected(&app_support_dir) {
                continue;
            }
            let due = project_manager_shared::gmail::sync_due(&pool)
                .await
                .unwrap_or(false);
            if !due {
                continue;
            }
            crate::bg_events::emit_started(&app, crate::bg_events::SOURCE_GMAIL);
            match project_manager_shared::gmail::sync_mailbox(&app_support_dir, &pool).await {
                Ok(report) => {
                    let brain_pool = pool.clone();
                    let brain_path_inner = brain_path.clone();
                    let brain_rebuild_lock = brain_rebuild_lock.clone();
                    tauri::async_runtime::spawn(async move {
                        let _guard = brain_rebuild_lock.lock().await;
                        let _ = project_manager_shared::brain::rebuild_brain(
                            &brain_pool,
                            &brain_path_inner,
                        )
                        .await;
                    });
                    let summary = if report.new_messages > 0 {
                        Some(format!(
                            "{} new message{}",
                            report.new_messages,
                            if report.new_messages == 1 { "" } else { "s" }
                        ))
                    } else {
                        None
                    };
                    crate::bg_events::emit_finished(&app, crate::bg_events::SOURCE_GMAIL, summary);
                    let _ = app.emit("gmail:sync-finished", &report);
                    emit_new_mail_if_needed(&app, &report).await;
                    // Fire-and-forget auto-analyze for threads with new mail.
                    if report.new_messages > 0 {
                        let analyze_pool = pool.clone();
                        let analyze_dir = app_support_dir.clone();
                        let app_for_emit = app.clone();
                        tauri::async_runtime::spawn(async move {
                            spawn_auto_analyze(analyze_pool, analyze_dir, app_for_emit).await;
                        });
                    }
                }
                Err(error) => {
                    crate::bg_events::emit_error(&app, crate::bg_events::SOURCE_GMAIL, &error);
                    let _ = app.emit("gmail:sync-error", error);
                }
            }
        }
    });
}

async fn emit_new_mail_if_needed(
    app: &AppHandle,
    report: &project_manager_shared::gmail::GmailSyncReport,
) {
    if report.new_messages <= 0 {
        return;
    }
    let _ = app.emit(
        "gmail:new-mail",
        GmailNewMailEvent {
            new_messages: report.new_messages,
            new_threads: report.new_threads,
        },
    );
}

/// Background auto-analyze pass. Walks threads whose `message_count` exceeds
/// the count at last analysis (or that have never been analyzed) and runs
/// `analyze_thread_with_gemini` for each. Capped per pass to avoid stampedes.
async fn spawn_auto_analyze(pool: SqlitePool, app_support_dir: PathBuf, app: AppHandle) {
    let Some(api_key) = project_manager_shared::keychain::get_gemini_api_key(&app_support_dir)
        .ok()
        .flatten()
    else {
        return;
    };
    let Ok(stale) = project_manager_shared::gmail::list_threads_needing_reanalysis(&pool, 6).await
    else {
        return;
    };
    if stale.is_empty() {
        return;
    }
    for thread_id in stale {
        match project_manager_shared::gmail::analyze_thread_with_gemini_tagged(
            &api_key,
            &pool,
            &thread_id,
            false,
            "auto_new_mail",
        )
        .await
        {
            Ok(_) => {
                let _ = app.emit("gmail:auto-analyzed", &thread_id);
            }
            Err(error) => {
                eprintln!("auto-analyze failed for {thread_id}: {error}");
            }
        }
    }
}
