use std::sync::atomic::Ordering;

use tauri::State;

use crate::db::AppState;
use project_manager_shared::models::{
    BrainBrief, BrainContextResult, BrainCypherInput, BrainCypherResult, BrainFeedbackInput,
    BrainGraphFilters, BrainInferenceFilter, BrainInferenceListResult, BrainLayoutResult,
    BrainLearningEvent, BrainLearningEventInput, BrainLearningSnapshot, BrainRetrieveInput,
    BrainStatus, BrainTemplateInput, BrainTemplateResult, GraphCommunitySummary, NodeEmbedding,
    RLDigest, ReviewInferenceResult, SaveBrainViewInput, SavedBrainView, SupersessionRecord,
    TemplateDetail, WorkGraph, WriteBrainLayoutInput,
};

/// Mark the brain as dirty and wake the rebuild worker. Idempotent and cheap —
/// safe to call from every write path. The worker coalesces rapid bursts into a
/// single rebuild via the dirty atomic.
pub fn spawn_brain_rebuild(state: &AppState) {
    state.brain_dirty.store(true, Ordering::SeqCst);
    state.brain_rebuild_notify.notify_one();
}

/// Spawn the long-lived brain rebuild worker. Call once at app startup.
/// The worker awaits `brain_rebuild_notify`, drains `brain_dirty`, and runs
/// `rebuild_brain`. If new dirty writes arrive while a rebuild is in flight,
/// `notify_one` queues exactly one wake-up so the next iteration handles them.
pub fn start_brain_worker(state: &AppState) {
    let pool = state.pool.clone();
    let path = state.brain_path.clone();
    let lock = state.brain_rebuild_lock.clone();
    let dirty = state.brain_dirty.clone();
    let notify = state.brain_rebuild_notify.clone();
    let app = state.app_handle.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            notify.notified().await;
            if !dirty.swap(false, Ordering::SeqCst) {
                continue;
            }
            crate::bg_events::emit_started(&app, crate::bg_events::SOURCE_BRAIN);
            let _guard = lock.lock().await;
            if let Err(error) =
                project_manager_shared::reasoning::refresh_reasoning_index(&pool).await
            {
                eprintln!("[reasoning] source refresh failed during brain rebuild: {error}");
            }
            match project_manager_shared::brain::rebuild_brain(&pool, &path).await {
                Ok(_) => {
                    crate::bg_events::emit_finished(&app, crate::bg_events::SOURCE_BRAIN, None);
                }
                Err(error) => {
                    crate::bg_events::emit_error(&app, crate::bg_events::SOURCE_BRAIN, error);
                }
            }
        }
    });
}

pub(crate) async fn ensure_brain_fresh(state: &AppState) -> Result<(), String> {
    if state.brain_dirty.swap(false, Ordering::SeqCst) || !state.brain_path.exists() {
        let _guard = state.brain_rebuild_lock.lock().await;
        project_manager_shared::reasoning::refresh_reasoning_index(&state.pool).await?;
        project_manager_shared::brain::rebuild_brain(&state.pool, &state.brain_path).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_brain_status(state: State<'_, AppState>) -> Result<BrainStatus, String> {
    Ok(project_manager_shared::brain::get_brain_status(&state.brain_path).await)
}

#[tauri::command]
pub async fn rebuild_brain(state: State<'_, AppState>) -> Result<BrainStatus, String> {
    let _guard = state.brain_rebuild_lock.lock().await;
    let status =
        project_manager_shared::brain::rebuild_brain(&state.pool, &state.brain_path).await?;
    state.brain_dirty.store(false, Ordering::SeqCst);
    Ok(status)
}

#[tauri::command]
pub async fn get_brain_graph(
    filters: Option<BrainGraphFilters>,
    state: State<'_, AppState>,
) -> Result<WorkGraph, String> {
    ensure_brain_fresh(state.inner()).await?;
    project_manager_shared::brain::get_brain_graph(
        &state.pool,
        &state.brain_path,
        filters.unwrap_or_default(),
    )
    .await
}

#[tauri::command]
pub async fn retrieve_brain_context(
    input: BrainRetrieveInput,
    state: State<'_, AppState>,
) -> Result<BrainContextResult, String> {
    ensure_brain_fresh(state.inner()).await?;
    project_manager_shared::brain::retrieve_brain_context(&state.pool, &state.brain_path, input)
        .await
}

#[tauri::command]
pub async fn query_brain_cypher(
    input: BrainCypherInput,
    state: State<'_, AppState>,
) -> Result<BrainCypherResult, String> {
    ensure_brain_fresh(state.inner()).await?;
    project_manager_shared::brain::query_brain_cypher(&state.brain_path, input).await
}

#[tauri::command]
pub async fn run_brain_template(
    input: BrainTemplateInput,
    state: State<'_, AppState>,
) -> Result<BrainTemplateResult, String> {
    ensure_brain_fresh(state.inner()).await?;
    project_manager_shared::brain::run_brain_template(&state.pool, &state.brain_path, input).await
}

#[tauri::command]
pub async fn get_daily_brain_brief(state: State<'_, AppState>) -> Result<BrainBrief, String> {
    ensure_brain_fresh(state.inner()).await?;
    project_manager_shared::brain::get_daily_brain_brief(&state.pool, &state.brain_path).await
}

#[tauri::command]
pub async fn record_brain_feedback(
    input: BrainFeedbackInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::brain::record_brain_feedback(&state.pool, input).await?;
    spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn record_brain_learning_event(
    input: BrainLearningEventInput,
    state: State<'_, AppState>,
) -> Result<BrainLearningEvent, String> {
    project_manager_shared::brain::record_brain_learning_event(&state.pool, input).await
}

#[tauri::command]
pub async fn get_brain_learning_snapshot(
    template: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<BrainLearningSnapshot, String> {
    project_manager_shared::brain::get_brain_learning_snapshot(&state.pool, template, limit).await
}

#[tauri::command]
pub async fn get_brain_learning_summary(
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::BrainLearningSummary, String> {
    project_manager_shared::brain::get_brain_learning_summary(&state.pool).await
}

// ───────── Section 6.2 — RL feedback surface IPC ─────────

#[tauri::command]
pub async fn list_brain_inferences(
    filter: Option<BrainInferenceFilter>,
    state: State<'_, AppState>,
) -> Result<BrainInferenceListResult, String> {
    project_manager_shared::brain::list_brain_inferences(
        &state.pool,
        filter.unwrap_or(BrainInferenceFilter {
            status: None,
            template: None,
            limit: None,
            before_updated_at: None,
        }),
    )
    .await
}

#[tauri::command]
pub async fn review_inference(
    inference_id: String,
    decision: String,
    state: State<'_, AppState>,
) -> Result<ReviewInferenceResult, String> {
    let result =
        project_manager_shared::brain::review_inference(&state.pool, &inference_id, &decision)
            .await?;
    // The inference graph projection depends on inference status; trigger a
    // debounced rebuild so the brain graph reflects the user's decision.
    spawn_brain_rebuild(state.inner());
    Ok(result)
}

#[tauri::command]
pub async fn list_inference_supersessions(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<SupersessionRecord>, String> {
    project_manager_shared::brain::list_inference_supersessions(&state.pool, limit).await
}

#[tauri::command]
pub async fn revert_inference_supersession(
    loser_inference_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::brain::revert_inference_supersession(&state.pool, &loser_inference_id)
        .await?;
    spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn get_rl_digest(
    days: Option<i64>,
    state: State<'_, AppState>,
) -> Result<RLDigest, String> {
    project_manager_shared::brain::get_rl_digest(&state.pool, days).await
}

#[tauri::command]
pub async fn get_template_detail(
    template: String,
    state: State<'_, AppState>,
) -> Result<TemplateDetail, String> {
    project_manager_shared::brain::get_template_detail(&state.pool, &template).await
}

#[tauri::command]
pub async fn reset_brain_template_learning(
    template: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::brain::reset_brain_template_learning(&state.pool, &template).await
}

// ───────── Phase 2 brain explorer: saved views + communities ─────────

#[tauri::command]
pub async fn list_saved_brain_views(
    state: State<'_, AppState>,
) -> Result<Vec<SavedBrainView>, String> {
    project_manager_shared::brain::list_saved_brain_views(&state.pool).await
}

#[tauri::command]
pub async fn save_brain_view(
    input: SaveBrainViewInput,
    state: State<'_, AppState>,
) -> Result<SavedBrainView, String> {
    project_manager_shared::brain::save_brain_view(&state.pool, input).await
}

#[tauri::command]
pub async fn delete_brain_view(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::brain::delete_brain_view(&state.pool, &id).await
}

#[tauri::command]
pub async fn list_graph_communities(
    level: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<GraphCommunitySummary>, String> {
    project_manager_shared::brain::list_graph_communities(&state.pool, level).await
}

// ───────── Phase 3 brain explorer: layout cache + embeddings ─────────

#[tauri::command]
pub async fn get_brain_layout(
    mode: String,
    graph_version: String,
    state: State<'_, AppState>,
) -> Result<Option<BrainLayoutResult>, String> {
    project_manager_shared::brain::read_brain_layout(&state.pool, &mode, &graph_version).await
}

#[tauri::command]
pub async fn write_brain_layout(
    input: WriteBrainLayoutInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::brain::write_brain_layout(
        &state.pool,
        &input.mode,
        &input.graph_version,
        &input.points,
    )
    .await
}

#[tauri::command]
pub async fn invalidate_brain_layouts(state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::brain::invalidate_brain_layouts(&state.pool).await
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetNodeEmbeddingsInput {
    pub ids: Vec<NodeEmbeddingId>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NodeEmbeddingId {
    pub entity_kind: String,
    pub entity_id: String,
}

#[tauri::command]
pub async fn get_node_embeddings(
    input: GetNodeEmbeddingsInput,
    state: State<'_, AppState>,
) -> Result<Vec<NodeEmbedding>, String> {
    let ids: Vec<(String, String)> = input
        .ids
        .into_iter()
        .map(|p| (p.entity_kind, p.entity_id))
        .collect();
    project_manager_shared::brain::get_node_embeddings(&state.pool, &ids).await
}
