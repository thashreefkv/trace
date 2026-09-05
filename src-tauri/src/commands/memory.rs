use std::path::Path;

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};

use crate::{
    db::AppState,
    models::{
        CreateMemoryInput, ExtractMemoriesFromConversationInput, ListMemoryEventsFilters,
        ListMemoryFilters, MemoryConsolidationResult, MemoryEvent, MemoryExtractionResult,
        MemoryFeedbackInput, MemoryRecord, MemoryRetrievalResult, MemorySettings,
        RetrieveMemoryInput, UpdateMemoryInput, UpdateMemorySettingsInput,
    },
};

const MEMORY_EXTRACTION_PROMPT: &str = include_str!("../prompts/extract_memories.md");

#[derive(Debug, Serialize, Clone)]
struct MemoryExtractedEvent {
    source_kind: String,
    source_id: Option<String>,
    created: i64,
    updated: i64,
    skipped: i64,
}

pub async fn auto_extract_from_text(
    pool: &SqlitePool,
    app_support_dir: &Path,
    app: &AppHandle,
    source_kind: &str,
    source_id: Option<&str>,
    source_text: &str,
) {
    let settings = match project_manager_shared::repo::get_memory_settings(pool).await {
        Ok(settings) => settings,
        Err(_) => return,
    };
    if !settings.enabled || !settings.auto_extract_enabled {
        return;
    }
    let api_key = match project_manager_shared::keychain::get_gemini_api_key(app_support_dir) {
        Ok(Some(key)) => key,
        _ => return,
    };

    match project_manager_shared::repo::extract_memories_from_text(
        pool,
        source_kind,
        source_id,
        source_text,
        &api_key,
        MEMORY_EXTRACTION_PROMPT,
    )
    .await
    {
        Ok(result) => {
            if result.created_count > 0 || result.updated_count > 0 {
                let _ = app.emit(
                    "memory:extracted",
                    MemoryExtractedEvent {
                        source_kind: source_kind.to_string(),
                        source_id: source_id.map(|id| id.to_string()),
                        created: result.created_count,
                        updated: result.updated_count,
                        skipped: result.skipped_count,
                    },
                );
            }
        }
        Err(error) => {
            eprintln!("memory auto-extract failed for {source_kind}: {error}");
        }
    }
}

#[tauri::command]
pub async fn get_memory_settings(state: State<'_, AppState>) -> Result<MemorySettings, String> {
    project_manager_shared::repo::get_memory_settings(&state.pool).await
}

#[tauri::command]
pub async fn update_memory_settings(
    input: UpdateMemorySettingsInput,
    state: State<'_, AppState>,
) -> Result<MemorySettings, String> {
    project_manager_shared::repo::update_memory_settings(&state.pool, input).await
}

#[tauri::command]
pub async fn list_memories(
    filters: Option<ListMemoryFilters>,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryRecord>, String> {
    project_manager_shared::repo::list_memories(&state.pool, filters.unwrap_or_default()).await
}

#[tauri::command]
pub async fn create_memory(
    input: CreateMemoryInput,
    state: State<'_, AppState>,
) -> Result<MemoryRecord, String> {
    let memory = project_manager_shared::repo::create_memory(&state.pool, input).await?;
    if let Ok(Some(api_key)) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)
    {
        let combined = format!("{}\n{}", memory.title, memory.body);
        let _ = project_manager_shared::repo::upsert_memory_embedding(
            &state.pool,
            &memory.id,
            &combined,
            &api_key,
        )
        .await;
    }
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(memory)
}

#[tauri::command]
pub async fn update_memory(
    id: String,
    input: UpdateMemoryInput,
    state: State<'_, AppState>,
) -> Result<MemoryRecord, String> {
    let memory = project_manager_shared::repo::update_memory(&state.pool, &id, input).await?;
    if let Ok(Some(api_key)) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)
    {
        let combined = format!("{}\n{}", memory.title, memory.body);
        let _ = project_manager_shared::repo::upsert_memory_embedding(
            &state.pool,
            &memory.id,
            &combined,
            &api_key,
        )
        .await;
    }
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(memory)
}

#[tauri::command]
pub async fn delete_memory(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_memory(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn retrieve_memories(
    input: RetrieveMemoryInput,
    state: State<'_, AppState>,
) -> Result<MemoryRetrievalResult, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)
        .ok()
        .flatten();
    project_manager_shared::repo::retrieve_memories_with_key(&state.pool, input, api_key.as_deref())
        .await
}

#[tauri::command]
pub async fn consolidate_memories(
    state: State<'_, AppState>,
) -> Result<MemoryConsolidationResult, String> {
    let result = project_manager_shared::repo::consolidate_memories(&state.pool).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(result)
}

#[tauri::command]
pub async fn extract_memories_from_conversation(
    input: ExtractMemoriesFromConversationInput,
    state: State<'_, AppState>,
) -> Result<MemoryExtractionResult, String> {
    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err(
            "Gemini API key is required for memory extraction. Save a key in Settings first."
                .to_string(),
        );
    };
    project_manager_shared::repo::extract_memories_from_conversation(
        &state.pool,
        &input.conversation_id,
        &api_key,
        MEMORY_EXTRACTION_PROMPT,
    )
    .await
    .map(|result| {
        crate::commands::brain::spawn_brain_rebuild(state.inner());
        result
    })
}

#[tauri::command]
pub async fn record_memory_feedback(
    input: MemoryFeedbackInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::repo::record_memory_feedback(
        &state.pool,
        &input.retrieval_id,
        &input.feedback,
    )
    .await
}

#[tauri::command]
pub async fn list_memory_events(
    filters: Option<ListMemoryEventsFilters>,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryEvent>, String> {
    project_manager_shared::repo::list_memory_events(&state.pool, filters.unwrap_or_default()).await
}

#[tauri::command]
pub async fn embed_memories_now(state: State<'_, AppState>) -> Result<i64, String> {
    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err(
            "Gemini API key is required for memory embeddings. Save a key in Settings first."
                .to_string(),
        );
    };
    project_manager_shared::repo::ensure_active_memory_embeddings(&state.pool, &api_key, 60).await
}
