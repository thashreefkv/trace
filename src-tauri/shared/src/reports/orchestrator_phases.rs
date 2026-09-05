//! Report pipeline phase runners (resolve/plan/draft/critique). From orchestrator.rs (13-std7).

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use tokio::sync::Semaphore;

use crate::models::{
    ReasoningScope, ReportCritique, ReportSectionDraft,
    ReportSectionPlan, ReportType,
};
use crate::reasoning;
use super::events::ReportEvent;
use super::sections;
use super::steps;
use super::orchestrator::PipelineDeps;

pub(super) async fn run_resolve_scope_step(
    deps: &PipelineDeps,
    report_run_id: &str,
    scope: &ReasoningScope,
    exclusions: &[String],
) -> Result<reasoning::ResolvedScope, String> {
    let step = steps::create_step(
        &deps.pool,
        report_run_id,
        "resolve_scope",
        None,
        &json!({ "exclusion_count": exclusions.len() }),
        Some("Resolving scope…"),
    )
    .await?;
    deps.emit(steps::into_event_created(&step));
    deps.emit(steps::into_event_started(&step));
    let started = steps::start_step(&deps.pool, &step.id, Some("Walking initiative relationships")).await?;
    match reasoning::resolve_scope(&deps.pool, scope, exclusions).await {
        Ok(resolved) => {
            let summary = format!(
                "{} entities · {} initiatives · {} deliverables · {} emails · {} meetings · {} files · {} events",
                resolved.source_unit_membership.len(),
                resolved.initiative_ids.len(),
                resolved.deliverable_ids.len(),
                resolved.email_thread_ids.len(),
                resolved.meeting_ids.len(),
                resolved.file_ids.len(),
                resolved.calendar_event_ids.len(),
            );
            steps::complete_step(
                &deps.pool,
                &step.id,
                &json!({
                    "summary": &summary,
                    "initiative_ids": &resolved.initiative_ids,
                    "deliverable_ids": &resolved.deliverable_ids,
                    "stakeholder_ids": &resolved.stakeholder_ids,
                    "email_thread_ids": &resolved.email_thread_ids,
                    "meeting_ids": &resolved.meeting_ids,
                    "file_ids": &resolved.file_ids,
                    "calendar_event_ids": &resolved.calendar_event_ids,
                }),
                None,
                false,
                started.elapsed().as_millis() as i64,
            )
            .await?;
            deps.emit(ReportEvent::StepCompleted {
                step_id: step.id.clone(),
                report_run_id: report_run_id.to_string(),
                step_name: "resolve_scope".to_string(),
                output_summary: Some(summary),
                latency_ms: Some(started.elapsed().as_millis() as i64),
                model: None,
                cache_hit: false,
            });
            Ok(resolved)
        }
        Err(error) => {
            steps::fail_step(&deps.pool, &step.id, &error).await.ok();
            deps.emit(ReportEvent::StepFailed {
                step_id: step.id.clone(),
                report_run_id: report_run_id.to_string(),
                step_name: "resolve_scope".to_string(),
                error: error.clone(),
            });
            Err(error)
        }
    }
}

pub(super) async fn run_plan_sections_step(
    deps: &PipelineDeps,
    report_run_id: &str,
    report_type: ReportType,
    title: &str,
    scope: &ReasoningScope,
    resolved: &reasoning::ResolvedScope,
) -> Result<Vec<ReportSectionPlan>, String> {
    let step = steps::create_step(
        &deps.pool,
        report_run_id,
        "plan_sections",
        None,
        &json!({ "report_type": report_type.as_str(), "title": title }),
        Some("Planning sections…"),
    )
    .await?;
    deps.emit(steps::into_event_created(&step));
    deps.emit(steps::into_event_started(&step));
    let started = steps::start_step(&deps.pool, &step.id, Some("Asking Flash for a section list")).await?;
    match sections::plan_sections(&deps.pool, &deps.api_key, report_type, title, scope, resolved).await {
        Ok(plans) => {
            let summary = format!("{} sections proposed", plans.len());
            steps::complete_step(
                &deps.pool,
                &step.id,
                &json!({ "summary": &summary, "sections": &plans }),
                Some("gemini-3-flash-preview"),
                false,
                started.elapsed().as_millis() as i64,
            )
            .await?;
            deps.emit(ReportEvent::StepCompleted {
                step_id: step.id.clone(),
                report_run_id: report_run_id.to_string(),
                step_name: "plan_sections".to_string(),
                output_summary: Some(summary),
                latency_ms: Some(started.elapsed().as_millis() as i64),
                model: Some("gemini-3-flash-preview".to_string()),
                cache_hit: false,
            });
            Ok(plans)
        }
        Err(error) => {
            steps::fail_step(&deps.pool, &step.id, &error).await.ok();
            deps.emit(ReportEvent::StepFailed {
                step_id: step.id.clone(),
                report_run_id: report_run_id.to_string(),
                step_name: "plan_sections".to_string(),
                error: error.clone(),
            });
            Err(error)
        }
    }
}

pub(super) async fn run_draft_steps(
    deps: &PipelineDeps,
    report_run_id: &str,
    report_type: ReportType,
    title: &str,
    scope: &ReasoningScope,
    resolved: &reasoning::ResolvedScope,
    sections_list: &[ReportSectionPlan],
) -> Result<HashMap<String, ReportSectionDraft>, String> {
    let semaphore = Arc::new(Semaphore::new(deps.section_concurrency.max(1)));
    let mut futures = Vec::new();
    for (idx, section) in sections_list.iter().enumerate() {
        let semaphore = semaphore.clone();
        let section = section.clone();
        let report_title = title.to_string();
        let scope = scope.clone();
        let resolved = resolved.clone();
        let report_run_id = report_run_id.to_string();
        let pool = deps.pool.clone();
        let api_key = deps.api_key.clone();
        let brain_path = deps.brain_path.clone();
        let sink = deps.sink.clone();
        let section_index = idx as i64;
        futures.push(crate::runtime::spawn(async move {
            let _permit = semaphore.acquire_owned().await.ok();
            let inner_deps = PipelineDeps {
                pool: pool.clone(),
                brain_path: brain_path.clone(),
                api_key: api_key.clone(),
                sink: sink.clone(),
                clarifications: steps::new_clarification_registry(),
                section_concurrency: 1,
            };
            let result = single_draft(
                &inner_deps,
                &report_run_id,
                report_type,
                &report_title,
                &scope,
                &resolved,
                section_index,
                &section,
            )
            .await;
            (section.id.clone(), result)
        }));
    }
    let mut drafts: HashMap<String, ReportSectionDraft> = HashMap::new();
    for handle in futures {
        let (section_id, outcome) =
            handle.await.map_err(|error| format!("section task failed: {error}"))?;
        match outcome {
            Ok(draft) => {
                drafts.insert(section_id, draft);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(drafts)
}

pub(super) async fn run_single_draft_step(
    deps: &PipelineDeps,
    report_run_id: &str,
    report_type: ReportType,
    title: &str,
    scope: &ReasoningScope,
    resolved: &reasoning::ResolvedScope,
    section: &ReportSectionPlan,
) -> Result<ReportSectionDraft, String> {
    single_draft(
        deps,
        report_run_id,
        report_type,
        title,
        scope,
        resolved,
        section.position,
        section,
    )
    .await
}

pub(super) async fn single_draft(
    deps: &PipelineDeps,
    report_run_id: &str,
    report_type: ReportType,
    title: &str,
    scope: &ReasoningScope,
    resolved: &reasoning::ResolvedScope,
    section_index: i64,
    section: &ReportSectionPlan,
) -> Result<ReportSectionDraft, String> {
    let step = steps::create_step(
        &deps.pool,
        report_run_id,
        "draft_section",
        Some(section_index),
        &json!({ "section_id": section.id, "heading": section.heading }),
        Some(&format!("Drafting \"{}\"…", section.heading)),
    )
    .await?;
    deps.emit(steps::into_event_created(&step));
    deps.emit(steps::into_event_started(&step));
    let started = steps::start_step(
        &deps.pool,
        &step.id,
        Some(&format!("Drafting \"{}\"", section.heading)),
    )
    .await?;
    match sections::draft_section(
        &deps.pool,
        &deps.brain_path,
        &deps.api_key,
        report_run_id,
        report_type,
        title,
        scope,
        resolved,
        section,
    )
    .await
    {
        Ok((draft, result)) => {
            steps::complete_step(
                &deps.pool,
                &step.id,
                &json!({
                    "section_id": section.id,
                    "markdown_chars": draft.markdown.len(),
                    "citation_count": draft.citation_ids.len(),
                    "cache_hit": result.cache_hit,
                }),
                Some(&result.model),
                result.cache_hit,
                started.elapsed().as_millis() as i64,
            )
            .await?;
            deps.emit(ReportEvent::StepCompleted {
                step_id: step.id.clone(),
                report_run_id: report_run_id.to_string(),
                step_name: "draft_section".to_string(),
                output_summary: Some(format!("{} chars", draft.markdown.len())),
                latency_ms: Some(started.elapsed().as_millis() as i64),
                model: Some(result.model.clone()),
                cache_hit: result.cache_hit,
            });
            Ok(draft)
        }
        Err(error) => {
            steps::fail_step(&deps.pool, &step.id, &error).await.ok();
            deps.emit(ReportEvent::StepFailed {
                step_id: step.id.clone(),
                report_run_id: report_run_id.to_string(),
                step_name: "draft_section".to_string(),
                error: error.clone(),
            });
            Err(error)
        }
    }
}

pub(super) async fn run_critique_step(
    deps: &PipelineDeps,
    report_run_id: &str,
    report_type: ReportType,
    title: &str,
    resolved: &reasoning::ResolvedScope,
    sections_list: &[ReportSectionPlan],
    drafts: &HashMap<String, ReportSectionDraft>,
) -> Result<ReportCritique, String> {
    let scope_names = if resolved.initiative_titles.is_empty() {
        "the selected scope".to_string()
    } else {
        resolved
            .initiative_titles
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let step = steps::create_step(
        &deps.pool,
        report_run_id,
        "critique",
        None,
        &json!({ "section_count": sections_list.len() }),
        Some("Critique pass…"),
    )
    .await?;
    deps.emit(steps::into_event_created(&step));
    deps.emit(steps::into_event_started(&step));
    let started = steps::start_step(&deps.pool, &step.id, Some("Scanning for contradictions and scope leaks")).await?;
    match sections::critique_draft(
        &deps.pool,
        &deps.api_key,
        report_type,
        title,
        &scope_names,
        sections_list,
        drafts,
    )
    .await
    {
        Ok(critique) => {
            let summary = if critique.issues.is_empty() {
                "No issues flagged".to_string()
            } else {
                format!("{} issue(s) flagged", critique.issues.len())
            };
            steps::complete_step(
                &deps.pool,
                &step.id,
                &json!({ "summary": &summary, "issue_count": critique.issues.len() }),
                Some("gemini-3-flash-preview"),
                false,
                started.elapsed().as_millis() as i64,
            )
            .await?;
            deps.emit(ReportEvent::StepCompleted {
                step_id: step.id.clone(),
                report_run_id: report_run_id.to_string(),
                step_name: "critique".to_string(),
                output_summary: Some(summary),
                latency_ms: Some(started.elapsed().as_millis() as i64),
                model: Some("gemini-3-flash-preview".to_string()),
                cache_hit: false,
            });
            Ok(critique)
        }
        Err(error) => {
            steps::fail_step(&deps.pool, &step.id, &error).await.ok();
            deps.emit(ReportEvent::StepFailed {
                step_id: step.id.clone(),
                report_run_id: report_run_id.to_string(),
                step_name: "critique".to_string(),
                error: error.clone(),
            });
            Err(error)
        }
    }
}

// === persistence helpers ==================================================

