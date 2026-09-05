use std::sync::Arc;

use tauri::{Emitter, State};

use crate::db::AppState;
use crate::models::{
    AnswerClarificationInput, ApproveOutlineInput, CreateReportRunInput, Deliverable,
    ExportReportInput, LinkReportDeliverableInput, ReplaceScopeExclusionsInput, ReportExport,
    ReportRun, ReportScopePreview, ReportStep, RerunReportStepInput, SteerReportInput,
    UpdateReportSectionsInput,
};
use project_manager_shared::reports::{
    self, run_pipeline, ClarificationAnswer, PipelineDeps, ReportEvent, ReportEventSink,
};

fn api_key(state: &AppState) -> Result<String, String> {
    project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| "Gemini API key not configured. Add it in Settings.".to_string())
}

/// Sink that bridges the shared crate's `ReportEvent` to the `report:event`
/// Tauri channel. Cheap to clone — the inner AppHandle is shared.
struct AppHandleSink {
    handle: tauri::AppHandle,
}

impl ReportEventSink for AppHandleSink {
    fn emit(&self, event: ReportEvent) {
        let _ = self.handle.emit(ReportEvent::CHANNEL, event);
    }
}

fn pipeline_deps(state: &AppState, api_key: String) -> PipelineDeps {
    PipelineDeps {
        pool: state.pool.clone(),
        brain_path: state.brain_path.clone(),
        api_key,
        sink: Arc::new(AppHandleSink {
            handle: state.app_handle.clone(),
        }),
        clarifications: state.report_clarifications.clone(),
        section_concurrency: 3,
    }
}

#[tauri::command]
pub async fn list_report_runs(state: State<'_, AppState>) -> Result<Vec<ReportRun>, String> {
    project_manager_shared::reports::list_report_runs(&state.pool).await
}

#[tauri::command]
pub async fn get_report_run(id: String, state: State<'_, AppState>) -> Result<ReportRun, String> {
    project_manager_shared::reports::get_report_run(&state.pool, &id).await
}

#[tauri::command]
pub async fn create_report_run(
    input: CreateReportRunInput,
    state: State<'_, AppState>,
) -> Result<ReportRun, String> {
    project_manager_shared::reports::create_report_run(&state.pool, input).await
}

#[tauri::command]
pub async fn delete_report_run(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::reports::delete_report_run(&state.pool, &report_run_id).await
}

#[tauri::command]
pub async fn generate_report_outline(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<ReportRun, String> {
    super::brain::ensure_brain_fresh(state.inner()).await?;
    project_manager_shared::reports::generate_report_outline(
        &state.pool,
        &state.brain_path,
        &api_key(state.inner())?,
        &report_run_id,
    )
    .await
}

#[tauri::command]
pub async fn approve_report_outline(
    input: ApproveOutlineInput,
    state: State<'_, AppState>,
) -> Result<ReportRun, String> {
    project_manager_shared::reports::approve_report_outline(&state.pool, input).await
}

#[tauri::command]
pub async fn generate_report_draft(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<ReportRun, String> {
    super::brain::ensure_brain_fresh(state.inner()).await?;
    project_manager_shared::reports::generate_report_draft(
        &state.pool,
        &state.brain_path,
        &api_key(state.inner())?,
        &report_run_id,
    )
    .await
}

#[tauri::command]
pub async fn approve_report(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<ReportRun, String> {
    project_manager_shared::reports::approve_report(&state.pool, &report_run_id).await
}

#[tauri::command]
pub async fn export_report(
    input: ExportReportInput,
    state: State<'_, AppState>,
) -> Result<ReportExport, String> {
    project_manager_shared::reports::export_report(&state.pool, &state.app_support_dir, input).await
}

#[tauri::command]
pub async fn list_report_exports(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ReportExport>, String> {
    project_manager_shared::reports::list_report_exports(&state.pool, &report_run_id).await
}

#[tauri::command]
pub async fn link_report_deliverable(
    input: LinkReportDeliverableInput,
    state: State<'_, AppState>,
) -> Result<ReportRun, String> {
    let report =
        project_manager_shared::reports::link_report_deliverable(&state.pool, input).await?;
    super::brain::spawn_brain_rebuild(state.inner());
    Ok(report)
}

#[tauri::command]
pub async fn create_report_deliverable(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<Deliverable, String> {
    let deliverable =
        project_manager_shared::reports::create_report_deliverable(&state.pool, &report_run_id)
            .await?;
    super::brain::spawn_brain_rebuild(state.inner());
    Ok(deliverable)
}

/// Fire the full 4-step pipeline (resolve → plan → draft → critique).
/// Returns immediately; progress arrives via the `report:event` channel.
#[tauri::command]
pub async fn run_report_pipeline(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    super::brain::ensure_brain_fresh(state.inner()).await?;
    let deps = pipeline_deps(state.inner(), api_key(state.inner())?);
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_pipeline(&deps, &report_run_id).await {
            eprintln!("[reports] pipeline failed for {report_run_id}: {error}");
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn get_report_scope_preview(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<ReportScopePreview, String> {
    project_manager_shared::reports::scope_preview(&state.pool, &report_run_id).await
}

#[tauri::command]
pub async fn replace_scope_exclusions(
    input: ReplaceScopeExclusionsInput,
    state: State<'_, AppState>,
) -> Result<ReportRun, String> {
    project_manager_shared::reports::replace_scope_exclusions(
        &state.pool,
        &input.report_run_id,
        &input.excluded_entry_ids,
    )
    .await
}

#[tauri::command]
pub async fn update_report_sections(
    input: UpdateReportSectionsInput,
    state: State<'_, AppState>,
) -> Result<ReportRun, String> {
    project_manager_shared::reports::update_sections(
        &state.pool,
        &input.report_run_id,
        &input.sections,
    )
    .await
}

#[tauri::command]
pub async fn list_report_steps(
    report_run_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ReportStep>, String> {
    project_manager_shared::reports::list_report_steps_for(&state.pool, &report_run_id).await
}

#[tauri::command]
pub async fn answer_report_clarification(
    input: AnswerClarificationInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let answer = ClarificationAnswer {
        selected_labels: input.selected_labels,
        free_text: input.free_text,
    };
    project_manager_shared::reports::answer_clarification(
        &state.report_clarifications,
        &input.step_id,
        answer,
    )
}

/// Re-run a specific pipeline step. Spawns async; progress via `report:event`.
#[tauri::command]
pub async fn rerun_report_step(
    input: RerunReportStepInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    super::brain::ensure_brain_fresh(state.inner()).await?;
    let deps = pipeline_deps(state.inner(), api_key(state.inner())?);
    let report_run_id = input.report_run_id.clone();
    let step_name = input.step_name.clone();
    let section_id = input.section_id.clone();
    tauri::async_runtime::spawn(async move {
        let ignore_cache = input.ignore_cache;
        if let Err(error) = project_manager_shared::reports::rerun_step(
            &deps,
            &report_run_id,
            &step_name,
            section_id.as_deref(),
            ignore_cache,
        )
        .await
        {
            eprintln!("[reports] rerun step {step_name} failed: {error}");
        }
    });
    Ok(())
}

/// Route a free-text steering instruction to the appropriate pipeline step.
/// Spawns async; result arrives via `report:event`.
#[tauri::command]
pub async fn steer_report_cmd(
    input: SteerReportInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    super::brain::ensure_brain_fresh(state.inner()).await?;
    let deps = pipeline_deps(state.inner(), api_key(state.inner())?);
    let report_run_id = input.report_run_id.clone();
    let instruction = input.instruction.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) =
            project_manager_shared::reports::steer_report(&deps, &report_run_id, &instruction).await
        {
            eprintln!("[reports] steer_report failed: {error}");
        }
    });
    Ok(())
}
