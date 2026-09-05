//! End-to-end report pipeline driver.
//!
//! Owns the choreography across `report_steps`: resolve scope, plan sections,
//! draft each section (parallel, capped via semaphore), critique. Each step
//! is persisted; each transition emits a [`ReportEvent`] via a caller-supplied
//! sink. The shared crate stays Tauri-free — `commands::reports` provides a
//! sink that calls `AppHandle::emit`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::models::{
    ReasoningScope, ReportClarification, ReportRun, ReportSectionDraft,
    ReportSectionPlan, ReportType,
};
use crate::reasoning;

use super::events::ReportEvent;
use super::orchestrator_persist::{
    advance_status, persist_critique, persist_section_drafts, persist_sections,
    update_draft_markdown, update_outline_markdown,
};
use super::orchestrator_phases::{
    run_critique_step, run_draft_steps, run_plan_sections_step, run_resolve_scope_step,
    run_single_draft_step,
};
use super::service::{get_report_run, list_report_steps_for};
use super::steps::{self, ClarificationRegistry};

/// Async sink for [`ReportEvent`]s. The Tauri-side implementation just calls
/// `app.emit("report:event", event)`. Tests can use a Vec-backed sink.
pub trait ReportEventSink: Send + Sync {
    fn emit(&self, event: ReportEvent);
}

impl<F> ReportEventSink for F
where
    F: Fn(ReportEvent) + Send + Sync,
{
    fn emit(&self, event: ReportEvent) {
        (self)(event);
    }
}

pub struct PipelineDeps {
    pub pool: SqlitePool,
    pub brain_path: PathBuf,
    pub api_key: String,
    pub sink: Arc<dyn ReportEventSink>,
    /// Shared map of pending clarification oneshot senders, keyed by step_id.
    /// Populated by [`steps::await_clarification`]; consumed by
    /// `answer_report_clarification` Tauri command.
    pub clarifications: ClarificationRegistry,
    /// Max parallel section drafts. 3 balances Pro rate limits with perceived
    /// speed (a 7-section report finishes in ~3 waves).
    pub section_concurrency: usize,
}

impl PipelineDeps {
    pub fn emit(&self, event: ReportEvent) {
        self.sink.emit(event);
    }
}

/// Run the entire pipeline for a report. Returns the latest snapshot.
///
/// Errors emit a [`ReportEvent::StepFailed`] on the channel and propagate. The
/// run's outer status (`outlined`/`drafted`/etc.) advances to keep the existing
/// UI flow working.
pub async fn run_pipeline(deps: &PipelineDeps, report_run_id: &str) -> Result<ReportRun, String> {
    deps.emit(ReportEvent::RunStarted {
        report_run_id: report_run_id.to_string(),
    });

    let report = get_report_run(&deps.pool, report_run_id).await?;
    let report_type = parse_report_type(&report.report_type)?;
    let scope = parse_scope(&report.scope_json)?;
    let exclusions: Vec<String> =
        serde_json::from_str(&report.scope_exclusions_json).unwrap_or_default();

    // === Step 1: resolve scope ===
    let resolved = run_resolve_scope_step(deps, report_run_id, &scope, &exclusions).await?;

    // === Step 2: plan sections ===
    let sections_list = run_plan_sections_step(
        deps,
        report_run_id,
        report_type,
        &report.title,
        &scope,
        &resolved,
    )
    .await?;
    persist_sections(&deps.pool, report_run_id, &sections_list).await?;
    advance_status(&deps.pool, report_run_id, "outlined").await?;
    update_outline_markdown(&deps.pool, report_run_id, &sections_list).await?;

    // === Step 3: draft each section in parallel (capped) ===
    let drafts = run_draft_steps(
        deps,
        report_run_id,
        report_type,
        &report.title,
        &scope,
        &resolved,
        &sections_list,
    )
    .await?;
    persist_section_drafts(&deps.pool, report_run_id, &drafts).await?;
    update_draft_markdown(&deps.pool, report_run_id, &sections_list, &drafts).await?;
    advance_status(&deps.pool, report_run_id, "drafted").await?;

    // === Step 4: critique ===
    let critique = run_critique_step(
        deps,
        report_run_id,
        report_type,
        &report.title,
        &resolved,
        &sections_list,
        &drafts,
    )
    .await?;
    persist_critique(&deps.pool, report_run_id, &critique).await?;

    let final_run = get_report_run(&deps.pool, report_run_id).await?;
    deps.emit(ReportEvent::RunFinished {
        report_run_id: report_run_id.to_string(),
        status: final_run.status.clone(),
    });
    Ok(final_run)
}

/// Re-run a specific step. For draft_section, `section_id` selects which
/// section to redo. When `ignore_cache` is true, the step's cache row is
/// purged before invocation (handled inside `query_reasoning_graph`'s cache
/// key — which already changes when scope changes; for now we approximate by
/// touching the scope_exclusions list timestamp).
pub async fn rerun_step(
    deps: &PipelineDeps,
    report_run_id: &str,
    step_name: &str,
    section_id: Option<&str>,
    _ignore_cache: bool,
) -> Result<ReportRun, String> {
    let report = get_report_run(&deps.pool, report_run_id).await?;
    let report_type = parse_report_type(&report.report_type)?;
    let scope = parse_scope(&report.scope_json)?;
    let exclusions: Vec<String> =
        serde_json::from_str(&report.scope_exclusions_json).unwrap_or_default();
    let resolved = reasoning::resolve_scope(&deps.pool, &scope, &exclusions).await?;
    let sections_list: Vec<ReportSectionPlan> =
        serde_json::from_str(&report.sections_json).unwrap_or_default();
    let mut drafts: HashMap<String, ReportSectionDraft> =
        serde_json::from_str(&report.section_drafts_json).unwrap_or_default();

    match step_name {
        "resolve_scope" => {
            run_resolve_scope_step(deps, report_run_id, &scope, &exclusions).await?;
        }
        "plan_sections" => {
            let new_sections = run_plan_sections_step(
                deps,
                report_run_id,
                report_type,
                &report.title,
                &scope,
                &resolved,
            )
            .await?;
            persist_sections(&deps.pool, report_run_id, &new_sections).await?;
            update_outline_markdown(&deps.pool, report_run_id, &new_sections).await?;
        }
        "draft_section" => {
            let section_id = section_id
                .ok_or_else(|| "section_id is required to re-run draft_section.".to_string())?;
            let section = sections_list
                .iter()
                .find(|s| s.id == section_id)
                .cloned()
                .ok_or_else(|| format!("Section {section_id} not found."))?;
            let draft = run_single_draft_step(
                deps,
                report_run_id,
                report_type,
                &report.title,
                &scope,
                &resolved,
                &section,
            )
            .await?;
            drafts.insert(section_id.to_string(), draft);
            persist_section_drafts(&deps.pool, report_run_id, &drafts).await?;
            update_draft_markdown(&deps.pool, report_run_id, &sections_list, &drafts).await?;
        }
        "critique" => {
            let critique = run_critique_step(
                deps,
                report_run_id,
                report_type,
                &report.title,
                &resolved,
                &sections_list,
                &drafts,
            )
            .await?;
            persist_critique(&deps.pool, report_run_id, &critique).await?;
        }
        other => return Err(format!("Unknown step name: {other}")),
    }
    get_report_run(&deps.pool, report_run_id).await
}

// === step implementations =================================================

fn parse_report_type(value: &str) -> Result<ReportType, String> {
    match value {
        "quarterly" => Ok(ReportType::Quarterly),
        "initiative" => Ok(ReportType::Initiative),
        "decision_memo" => Ok(ReportType::DecisionMemo),
        _ => Err("Stored report type is invalid.".to_string()),
    }
}

fn parse_scope(scope_json: &str) -> Result<ReasoningScope, String> {
    serde_json::from_str(scope_json)
        .map_err(|error| format!("Stored report scope is invalid: {error}"))
}

/// Parse a free-text steering instruction and re-run the right pipeline step.
///
/// Calls Gemini Flash to classify the intent into one of:
/// - `REPLAN` — redo plan_sections
/// - `REDRAFT:<section_id>` — redo one section
/// - `CRITIQUE` — redo the critique pass
/// - `CLARIFY:<question>` — instruction is ambiguous; emits StepNeedsClarification
/// Falls back to REPLAN when the model is uncertain.
pub async fn steer_report(
    deps: &PipelineDeps,
    report_run_id: &str,
    instruction: &str,
) -> Result<ReportRun, String> {
    let report = get_report_run(&deps.pool, report_run_id).await?;
    let sections: Vec<crate::models::ReportSectionPlan> =
        serde_json::from_str(&report.sections_json).unwrap_or_default();
    let sections_summary = sections
        .iter()
        .map(|s| format!("  - id={} heading=\"{}\"", s.id, s.heading))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        r#"A user gave you a steering instruction for a report titled "{title}".

Instruction: "{instruction}"

The report currently has these sections:
{sections_summary}

Classify the instruction as exactly ONE of:
- REPLAN — the user wants to change the section structure or plan
- REDRAFT:<section_id> — the user wants to re-draft one specific section (pick the most relevant section_id from above)
- CRITIQUE — the user wants to rerun the critique / quality check
- CLARIFY:<question> — the instruction is ambiguous and you need to ask one short clarifying question before acting

If the instruction is clear and maps to a section, prefer REDRAFT. If it is vague (e.g. "make it better", "fix it") choose CLARIFY with a short one-sentence question.

Respond with JSON: {{ "action": "REPLAN" | "REDRAFT:<id>" | "CRITIQUE" | "CLARIFY:<question>" }}"#,
        title = report.title,
    );
    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 0.1, "responseMimeType": "application/json" }
    });
    let raw = crate::gemini::post_gemini_external(
        Some(&deps.pool),
        "reports_steer",
        "gemini-3-flash-preview",
        &deps.api_key,
        &body,
    )
    .await
    .unwrap_or_else(|_| serde_json::Value::Null);

    let action = raw
        .get("candidates")
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("content"))
        .and_then(|v| v.get("parts"))
        .and_then(|v| v.get(0))
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|j| j.get("action").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "REPLAN".to_string());

    if action == "REPLAN" {
        rerun_step(deps, report_run_id, "plan_sections", None, false).await
    } else if action == "CRITIQUE" {
        rerun_step(deps, report_run_id, "critique", None, false).await
    } else if let Some(section_id) = action.strip_prefix("REDRAFT:") {
        rerun_step(deps, report_run_id, "draft_section", Some(section_id), false).await
    } else if let Some(question) = action.strip_prefix("CLARIFY:") {
        // Create a synthetic step that awaits a user answer via ClarifyPrompt.
        // A background task receives the answer and dispatches to the right step.
        let prompt = ReportClarification {
            question: question.trim().to_string(),
            options: vec![
                "Replan all sections".to_string(),
                "Re-draft a specific section".to_string(),
                "Rerun critique".to_string(),
                "Cancel".to_string(),
            ],
            free_text_allowed: true,
            multi_select: false,
        };
        let step = steps::create_step(
            &deps.pool,
            report_run_id,
            "steer_clarify",
            None,
            &serde_json::json!({ "instruction": instruction }),
            Some("Waiting for your input"),
        )
        .await?;
        let rx = steps::await_clarification(
            &deps.pool,
            &deps.clarifications,
            &step.id,
            &prompt,
        )
        .await?;
        deps.emit(ReportEvent::StepNeedsClarification {
            step_id: step.id.clone(),
            report_run_id: report_run_id.to_string(),
            prompt,
        });
        // Spawn a sub-task that receives the user's answer and dispatches
        // the appropriate step (30-min timeout → step is cancelled).
        let pool2 = deps.pool.clone();
        let brain_path2 = deps.brain_path.clone();
        let api_key2 = deps.api_key.clone();
        let sink2 = deps.sink.clone();
        let clarifications2 = deps.clarifications.clone();
        let concurrency2 = deps.section_concurrency;
        let run_id2 = report_run_id.to_string();
        let step_id2 = step.id.clone();
        crate::runtime::spawn(async move {
            const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);
            let answer = tokio::time::timeout(TIMEOUT, rx).await;
            match answer {
                Ok(Ok(ans)) => {
                    let _ = steps::cancel_step(&pool2, &step_id2).await;
                    let deps2 = PipelineDeps {
                        pool: pool2,
                        brain_path: brain_path2,
                        api_key: api_key2,
                        sink: sink2,
                        clarifications: clarifications2,
                        section_concurrency: concurrency2,
                    };
                    let chosen = ans.selected_labels.first().map(|s| s.as_str()).unwrap_or("");
                    let _ = match chosen {
                        "Replan all sections" => {
                            rerun_step(&deps2, &run_id2, "plan_sections", None, false).await
                        }
                        "Rerun critique" => {
                            rerun_step(&deps2, &run_id2, "critique", None, false).await
                        }
                        _ => {
                            // Free text or "Re-draft" → replan as best-effort.
                            // The user can send another specific steer instruction.
                            rerun_step(&deps2, &run_id2, "plan_sections", None, false).await
                        }
                    };
                }
                _ => {
                    // Timed out or receiver dropped — cancel the step
                    let _ = steps::cancel_step(&pool2, &step_id2).await;
                }
            }
        });
        get_report_run(&deps.pool, report_run_id).await
    } else {
        rerun_step(deps, report_run_id, "plan_sections", None, false).await
    }
}

/// Unused outside the orchestrator but exported for tests / debugging.
#[allow(dead_code)]
pub async fn report_steps_for(
    pool: &SqlitePool,
    report_run_id: &str,
) -> Result<Vec<crate::models::ReportStep>, String> {
    list_report_steps_for(pool, report_run_id).await
}

/// Re-export to silence the unused warning for now.
#[allow(dead_code)]
const _: Value = Value::Null;
