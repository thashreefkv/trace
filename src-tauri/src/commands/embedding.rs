use std::path::PathBuf;
use std::time::Duration;

use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::db::AppState;
use project_manager_shared::entity_embeddings::{self, EmbeddingProgress, DEFAULT_BATCH_SIZE};

const WORKER_INTERVAL_SECS: u64 = 60;
const FULL_SYNC_EVERY_N_CYCLES: u32 = 30; // ~30 min: refresh stale-hash check
const MEMORY_BACKFILL_BATCH_SIZE: i64 = 20;
const FILE_BACKFILL_BATCH_SIZE: i64 = 2;

pub fn spawn_embedding_worker(app: AppHandle, pool: SqlitePool, app_support_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        // Initial enqueue of all entities lacking embeddings. Cheap — `enqueue`
        // short-circuits when the content hash hasn't changed.
        if let Err(error) = entity_embeddings::enqueue_all_missing(&pool).await {
            eprintln!("[embedding] initial enqueue failed: {error}");
        }

        let mut cycles: u32 = 0;
        loop {
            cycles = cycles.wrapping_add(1);
            tokio::time::sleep(Duration::from_secs(WORKER_INTERVAL_SECS)).await;

            // Periodic re-scan to catch entities edited after their last embed.
            if cycles.is_multiple_of(FULL_SYNC_EVERY_N_CYCLES) {
                let _ = entity_embeddings::enqueue_all_missing(&pool).await;
            }

            let progress = entity_embeddings::progress(&pool).await;
            let Some(api_key) = project_manager_shared::runtime::gemini_api_key() else {
                // No key configured — can't embed. Stay idle.
                continue;
            };

            let mut started = false;
            let mut summaries: Vec<String> = Vec::new();

            if progress.pending > 0 {
                crate::bg_events::emit_started(&app, "embedding");
                started = true;
                match entity_embeddings::embed_pending(&pool, &api_key, DEFAULT_BATCH_SIZE).await {
                    Ok(processed) => {
                        if processed > 0 {
                            let after = entity_embeddings::progress(&pool).await;
                            summaries.push(format!(
                                "embedded {} entities · {} remaining",
                                processed, after.pending
                            ));
                        }
                    }
                    Err(error) => {
                        crate::bg_events::emit_error(&app, "embedding", error);
                        continue;
                    }
                }
            }

            match project_manager_shared::repo::ensure_active_memory_embeddings(
                &pool,
                &api_key,
                MEMORY_BACKFILL_BATCH_SIZE,
            )
            .await
            {
                Ok(processed) if processed > 0 => {
                    if !started {
                        crate::bg_events::emit_started(&app, "embedding");
                        started = true;
                    }
                    summaries.push(format!("embedded {} memories", processed));
                }
                Err(error) => {
                    if !started {
                        crate::bg_events::emit_started(&app, "embedding");
                    }
                    crate::bg_events::emit_error(&app, "embedding", error);
                    continue;
                }
                _ => {}
            }

            match project_manager_shared::files::embed_missing_drive_file_contents(
                &pool,
                &api_key,
                &app_support_dir,
                FILE_BACKFILL_BATCH_SIZE,
            )
            .await
            {
                Ok(processed) if processed > 0 => {
                    if !started {
                        crate::bg_events::emit_started(&app, "embedding");
                        started = true;
                    }
                    summaries.push(format!("embedded {} files", processed));
                }
                Err(error) => {
                    if !started {
                        crate::bg_events::emit_started(&app, "embedding");
                    }
                    crate::bg_events::emit_error(&app, "embedding", error);
                    continue;
                }
                _ => {}
            }

            if started {
                let summary = (!summaries.is_empty()).then(|| summaries.join(" · "));
                crate::bg_events::emit_finished(&app, "embedding", summary);
            }
        }
    });
}

#[tauri::command]
pub async fn get_embedding_progress(
    state: State<'_, AppState>,
) -> Result<EmbeddingProgress, String> {
    Ok(entity_embeddings::progress(&state.pool).await)
}

#[tauri::command]
pub async fn enqueue_all_embeddings(state: State<'_, AppState>) -> Result<i64, String> {
    entity_embeddings::enqueue_all_missing(&state.pool).await
}

#[tauri::command]
pub async fn embed_now(
    state: State<'_, AppState>,
    batch_size: Option<usize>,
) -> Result<usize, String> {
    let Some(api_key) = project_manager_shared::runtime::gemini_api_key() else {
        return Err("Gemini API key is not configured.".to_string());
    };
    entity_embeddings::embed_pending(
        &state.pool,
        &api_key,
        batch_size.unwrap_or(DEFAULT_BATCH_SIZE),
    )
    .await
}
