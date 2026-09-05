use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::db::AppState;
use project_manager_shared::models::{
    AppendAskTurnInput, AskAttachment, AskChatDetail, AskChatRecord, AskChatSearchHit,
    AskProgressEvent, AskRunControls, AskRunEvent, AskSearchResult, AskTurnRecord, GraphQueryMode,
    ListAskChatsFilters, ReasoningDepth, ReasoningQueryInput, ReasoningScope, SearchResult,
    UpsertAskChatInput,
};

#[tauri::command]
pub async fn search_all(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    project_manager_shared::repo::search_all(&state.pool, &query).await
}

#[tauri::command]
pub async fn ask_search(
    question: String,
    context: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AskSearchResult, String> {
    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err("Gemini API key not configured. Add it in Settings.".to_string());
    };

    // Forward progress events to the frontend as they arrive (legacy non-streaming path).
    let (tx, mut rx) = mpsc::unbounded_channel::<AskProgressEvent>();
    tauri::async_runtime::spawn(async move {
        while let Some(evt) = rx.recv().await {
            let _ = app.emit("ask:progress", evt);
        }
    });

    project_manager_shared::gemini::ask_search(
        &api_key,
        &question,
        context.as_deref(),
        &state.pool,
        Some(&state.brain_path),
        Some(tx),
    )
    .await
}

/// Begin a streaming agent run. Returns immediately with the run id; results stream via
/// the `ask:event` Tauri event channel as `AskRunEvent` JSON.
#[tauri::command]
pub async fn start_ask_run(
    question: String,
    context: Option<String>,
    mode: Option<String>,
    attachments: Option<Vec<AskAttachment>>,
    permission_mode: Option<String>,
    auto_confirmed_tools: Option<Vec<String>>,
    reasoning_depth: Option<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let Some(api_key) =
        project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Err("Gemini API key not configured. Add it in Settings.".to_string());
    };

    let run_id = ulid::Ulid::new().to_string();
    let cancel = CancellationToken::new();
    state
        .ask_runs
        .lock()
        .map_err(|_| "ask run registry poisoned".to_string())?
        .insert(run_id.clone(), cancel.clone());

    let use_deep_reasoning = reasoning_depth.as_deref() == Some("deep");
    if use_deep_reasoning {
        super::brain::ensure_brain_fresh(state.inner()).await?;
    }
    let pool = state.pool.clone();
    let brain_path = state.brain_path.clone();
    let app_support_dir = state.app_support_dir.clone();
    let runs = state.ask_runs.clone();
    let confirmations = state.ask_confirmations.clone();
    let app_for_task = app.clone();
    let run_id_owned = run_id.clone();
    let api_key_owned = api_key;
    let attachments = attachments.unwrap_or_default();
    let mode_owned = mode.unwrap_or_else(|| "ask".to_string());
    let controls = AskRunControls {
        confirmations: Some(state.ask_confirmations.clone()),
        auto_confirmed_tools: auto_confirmed_tools.unwrap_or_default(),
        permission_mode,
    };
    let run_id_for_cleanup = run_id_owned.clone();

    tauri::async_runtime::spawn(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<AskRunEvent>();
        let app_for_emit = app_for_task.clone();
        let emitter = tauri::async_runtime::spawn(async move {
            while let Some(evt) = rx.recv().await {
                let _ = app_for_emit.emit("ask:event", evt);
            }
        });

        if use_deep_reasoning {
            let _ = tx.send(AskRunEvent::Started {
                run_id: run_id_owned.clone(),
            });
            let deep_query = if mode_owned == "act" {
                format!(
                    "Propose review-gated actions only; do not execute any writes. {}",
                    question
                )
            } else {
                question.clone()
            };
            match project_manager_shared::reasoning::query_reasoning_graph(
                &pool,
                &brain_path,
                &api_key_owned,
                ReasoningQueryInput {
                    query: deep_query,
                    retrieval_query: None,
                    depth: ReasoningDepth::Deep,
                    query_mode: GraphQueryMode::Auto,
                    scope: ReasoningScope::default(),
                    purpose: Some(format!("deep_{mode_owned}")),
                },
            )
            .await
            {
                Ok(result) => {
                    let mut answer = result.answer_markdown.clone();
                    if !result.action_proposals.is_empty() {
                        answer.push_str("\n\n### Proposed actions (review required)\n");
                        for proposal in &result.action_proposals {
                            answer.push_str(&format!("\n- {}", proposal.description));
                        }
                    }
                    if !result.contradictions.is_empty() {
                        answer.push_str("\n\n### Contradictions\n");
                        for contradiction in &result.contradictions {
                            answer.push_str(&format!("\n- {contradiction}"));
                        }
                    }
                    if !result.unsupported_assertions.is_empty() {
                        answer.push_str("\n\n### Unsupported assertions\n");
                        for assertion in &result.unsupported_assertions {
                            answer.push_str(&format!("\n- {assertion}"));
                        }
                    }
                    let _ = tx.send(AskRunEvent::Done {
                        run_id: run_id_owned.clone(),
                        result: AskSearchResult {
                            answer,
                            refs: project_manager_shared::reasoning::to_ask_refs(&result),
                            questions: vec![],
                            scored_nodes: vec![],
                            retrieval_query: Some(format!(
                                "{} reasoning via {}",
                                result.query_mode, result.model
                            )),
                        },
                    });
                }
                Err(message) => {
                    let _ = tx.send(AskRunEvent::Error {
                        run_id: run_id_owned.clone(),
                        message,
                    });
                }
            }
            drop(tx);
        } else {
            project_manager_shared::gemini::ask_search_stream(
                &api_key_owned,
                &run_id_owned,
                &question,
                context.as_deref(),
                &mode_owned,
                &attachments,
                &pool,
                Some(&brain_path),
                Some(&app_support_dir),
                tx,
                cancel,
                controls,
            )
            .await;
        }

        // Drop the sender by waiting for the emitter task to drain.
        let _ = emitter.await;

        if let Ok(mut guard) = runs.lock() {
            guard.remove(&run_id_owned);
        }
        // Scrub any stale confirmation entries belonging to this run so the
        // map doesn't accumulate dead keys when a run completes without all
        // of its destructive calls being confirmed.
        if let Ok(mut guard) = confirmations.lock() {
            guard.retain(|key, _| !key.starts_with(&format!("{run_id_for_cleanup}:")));
        }
    });

    Ok(run_id)
}

#[tauri::command]
pub async fn confirm_ask_tool_decision(
    run_id: String,
    call_id: String,
    approved: bool,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let key = format!("{run_id}:{call_id}");
    let sender = {
        let Ok(mut guard) = state.ask_confirmations.lock() else {
            return Err("confirmation registry poisoned".to_string());
        };
        guard.remove(&key)
    };
    match sender {
        Some(tx) => {
            let _ = tx.send(approved);
            Ok(true)
        }
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn list_prompt_injection_log(
    filter: Option<project_manager_shared::prompt_injection_log::PromptInjectionFilter>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::prompt_injection_log::PromptInjectionSnapshot, String> {
    project_manager_shared::prompt_injection_log::list(&state.pool, filter.unwrap_or_default())
        .await
}

#[tauri::command]
pub async fn cancel_ask_run(run_id: String, state: State<'_, AppState>) -> Result<bool, String> {
    let Ok(mut guard) = state.ask_runs.lock() else {
        return Err("ask run registry poisoned".to_string());
    };
    if let Some(token) = guard.remove(&run_id) {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn list_ask_chats(
    filters: Option<ListAskChatsFilters>,
    state: State<'_, AppState>,
) -> Result<Vec<AskChatRecord>, String> {
    project_manager_shared::repo::list_ask_chats(&state.pool, filters.unwrap_or_default()).await
}

#[tauri::command]
pub async fn get_ask_chat(
    chat_id: String,
    state: State<'_, AppState>,
) -> Result<AskChatDetail, String> {
    project_manager_shared::repo::get_ask_chat(&state.pool, &chat_id).await
}

#[tauri::command]
pub async fn upsert_ask_chat(
    input: UpsertAskChatInput,
    state: State<'_, AppState>,
) -> Result<AskChatRecord, String> {
    let chat = project_manager_shared::repo::upsert_ask_chat(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(chat)
}

#[tauri::command]
pub async fn delete_ask_chat(chat_id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_ask_chat(&state.pool, &chat_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn archive_ask_chat(chat_id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::archive_ask_chat(&state.pool, &chat_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn append_ask_turn(
    input: AppendAskTurnInput,
    state: State<'_, AppState>,
) -> Result<AskTurnRecord, String> {
    let turn = project_manager_shared::repo::append_ask_turn(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(turn)
}

#[tauri::command]
pub async fn search_ask_chats(
    query: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<AskChatSearchHit>, String> {
    project_manager_shared::repo::search_ask_chats(&state.pool, &query, limit).await
}
