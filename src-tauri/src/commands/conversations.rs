use tauri::{AppHandle, State};

use crate::{
    commands::memory::auto_extract_from_text,
    db::AppState,
    models::{
        CommitConversationIngestInput, Conversation, ConversationExtractionInput,
        ConversationExtractionResult, ConversationIngestResult,
    },
};

const EXTRACTION_PROMPT: &str = include_str!("../prompts/extract_conversation.md");

#[tauri::command]
pub async fn extract_conversation(
    input: ConversationExtractionInput,
    state: State<'_, AppState>,
) -> Result<ConversationExtractionResult, String> {
    let Some(key) = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err(
            "Gemini API key is required for conversation extraction. Save a key in Settings first."
                .to_string(),
        );
    };

    let result = project_manager_shared::gemini::extract_conversation(
        &state.pool,
        &key,
        input,
        EXTRACTION_PROMPT,
    )
    .await?;
    project_manager_shared::repo::annotate_extraction_mappings(&state.pool, result).await
}

#[tauri::command]
pub async fn commit_conversation_ingest(
    input: CommitConversationIngestInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ConversationIngestResult, String> {
    let result =
        project_manager_shared::repo::commit_conversation_ingest(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    spawn_conversation_memory_extraction(&state, &app, &result.conversation);
    Ok(result)
}

#[tauri::command]
pub async fn promote_claude_capture_to_ingest(
    capture_id: String,
    input: CommitConversationIngestInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ConversationIngestResult, String> {
    let result = project_manager_shared::repo::promote_claude_capture_to_ingest(
        &state.pool,
        &capture_id,
        input,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    let _ = crate::menu_bar::refresh_menu_bar_inner(&app).await;
    spawn_conversation_memory_extraction(&state, &app, &result.conversation);
    Ok(result)
}

#[tauri::command]
pub async fn get_conversation(
    id: String,
    state: State<'_, AppState>,
) -> Result<Conversation, String> {
    project_manager_shared::repo::get_conversation(&state.pool, &id).await
}

#[tauri::command]
pub async fn list_conversations(state: State<'_, AppState>) -> Result<Vec<Conversation>, String> {
    project_manager_shared::repo::list_conversations(&state.pool).await
}

fn spawn_conversation_memory_extraction(
    state: &State<'_, AppState>,
    app: &AppHandle,
    conversation: &Conversation,
) {
    let pool = state.pool.clone();
    let app_support_dir = state.app_support_dir.clone();
    let app = app.clone();
    let conversation_id = conversation.id.clone();
    let title = conversation
        .title
        .clone()
        .unwrap_or_else(|| "Untitled conversation".to_string());
    let summary = conversation
        .summary
        .clone()
        .unwrap_or_else(|| "No summary saved.".to_string());
    let occurred = conversation
        .occurred_at
        .clone()
        .unwrap_or_else(|| "unknown date".to_string());
    let chat_url = conversation.chat_url.clone();

    tauri::async_runtime::spawn(async move {
        let source = format!(
            "Conversation '{title}' on {occurred}.\nChat URL: {chat_url}\nSummary: {summary}"
        );
        auto_extract_from_text(
            &pool,
            &app_support_dir,
            &app,
            "conversation",
            Some(&conversation_id),
            &source,
        )
        .await;
    });
}
