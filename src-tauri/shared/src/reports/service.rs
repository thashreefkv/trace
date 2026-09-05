use std::path::Path;

use serde_json::json;
use sqlx::SqlitePool;

use super::{citations, provenance, steps, templates};
use crate::db::sql_error;
use crate::models::{
    ApproveOutlineInput, CreateDeliverableInput, CreateReportRunInput, Deliverable,
    DeliverableState, DeliverableType, LinkReportDeliverableInput, ReasoningDepth,
    ReasoningQueryInput, ReasoningScope, ReportRun, ReportScopeEntry, ReportScopePreview,
    ReportSectionPlan, ReportStep, ReportType,
};
use crate::reasoning;

pub async fn create_report_run(
    pool: &SqlitePool,
    input: CreateReportRunInput,
) -> Result<ReportRun, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("Report title is required.".to_string());
    }
    let id = format!("report:{}", ulid::Ulid::new());
    let now = crate::repo::now_utc();
    let scope_json = serde_json::to_string(&input.scope).map_err(|error| error.to_string())?;
    sqlx::query(
        r#"INSERT INTO report_runs (
            id, report_type, title, status, scope_json, outline_markdown, draft_markdown,
            citations_json, unsupported_json, created_at, updated_at
        ) VALUES (?, ?, ?, 'created', ?, '', '', '[]', '[]', ?, ?)"#,
    )
    .bind(&id)
    .bind(input.report_type.as_str())
    .bind(title)
    .bind(scope_json)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    provenance::record_event(
        pool,
        &id,
        "created",
        json!({ "report_type": input.report_type.as_str() }),
    )
    .await?;
    get_report_run(pool, &id).await
}

pub async fn list_report_runs(pool: &SqlitePool) -> Result<Vec<ReportRun>, String> {
    sqlx::query_as::<_, ReportRun>(
        r#"SELECT id, report_type, title, status, scope_json, outline_markdown,
           draft_markdown, citations_json, unsupported_json, reasoning_run_id,
           deliverable_id, created_at, updated_at, approved_at,
           COALESCE(scope_exclusions_json, '[]') AS scope_exclusions_json,
           COALESCE(sections_json, '[]') AS sections_json,
           COALESCE(section_drafts_json, '{}') AS section_drafts_json,
           COALESCE(critique_json, '{}') AS critique_json
           FROM report_runs ORDER BY updated_at DESC"#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn get_report_run(pool: &SqlitePool, id: &str) -> Result<ReportRun, String> {
    sqlx::query_as::<_, ReportRun>(
        r#"SELECT id, report_type, title, status, scope_json, outline_markdown,
           draft_markdown, citations_json, unsupported_json, reasoning_run_id,
           deliverable_id, created_at, updated_at, approved_at,
           COALESCE(scope_exclusions_json, '[]') AS scope_exclusions_json,
           COALESCE(sections_json, '[]') AS sections_json,
           COALESCE(section_drafts_json, '{}') AS section_drafts_json,
           COALESCE(critique_json, '{}') AS critique_json
           FROM report_runs WHERE id = ?"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "Report run not found.".to_string())
}

pub async fn delete_report_run(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(sql_error)?;
    let reasoning_run_id: Option<String> =
        sqlx::query_scalar::<_, Option<String>>("SELECT reasoning_run_id FROM report_runs WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(sql_error)?
            .flatten();
    let deleted = sqlx::query("DELETE FROM report_runs WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    if deleted.rows_affected() == 0 {
        return Err("Report run not found.".to_string());
    }
    sqlx::query("DELETE FROM entity_events WHERE entity_kind = 'report_run' AND entity_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    if let Some(reasoning_run_id) = reasoning_run_id {
        sqlx::query("DELETE FROM claim_versions WHERE reasoning_run_id = ? AND status = 'pending'")
            .bind(&reasoning_run_id)
            .execute(&mut *tx)
            .await
            .map_err(sql_error)?;
        sqlx::query(
            "DELETE FROM agent_review_items WHERE source_kind = 'reasoning_run' AND source_id = ? AND status = 'pending'",
        )
        .bind(&reasoning_run_id)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    }
    tx.commit().await.map_err(sql_error)
}

pub async fn generate_report_outline(
    pool: &SqlitePool,
    brain_path: &Path,
    api_key: &str,
    report_run_id: &str,
) -> Result<ReportRun, String> {
    let report = get_report_run(pool, report_run_id).await?;
    let report_type = parse_report_type(&report.report_type)?;
    let scope = parse_scope(&report.scope_json)?;
    let initiative_titles = load_initiative_titles(pool, &scope.initiative_ids).await?;
    let result = crate::reasoning::query_reasoning_graph(
        pool,
        brain_path,
        api_key,
        ReasoningQueryInput {
            query: templates::outline_prompt(
                report_type,
                &report.title,
                &scope,
                &initiative_titles,
            ),
            retrieval_query: Some(format!(
                "{} {} outline",
                initiative_titles.join(" "),
                report.title
            )),
            depth: ReasoningDepth::Deep,
            query_mode: templates::query_mode(report_type, "outline"),
            scope,
            purpose: Some(format!("report:{}:outline", report.id)),
        },
    )
    .await?;
    let now = crate::repo::now_utc();
    sqlx::query(
        r#"UPDATE report_runs SET outline_markdown = ?, status = 'outlined',
           citations_json = ?, unsupported_json = ?, reasoning_run_id = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&result.answer_markdown)
    .bind(serde_json::to_string(&result.citations).unwrap_or_else(|_| "[]".to_string()))
    .bind(
        serde_json::to_string(&result.unsupported_assertions).unwrap_or_else(|_| "[]".to_string()),
    )
    .bind(&result.run_id)
    .bind(now)
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    citations::replace_report_sources(pool, report_run_id, &result.citations).await?;
    provenance::record_event(
        pool,
        report_run_id,
        "outline_generated",
        json!({ "reasoning_run_id": result.run_id, "mode": result.query_mode }),
    )
    .await?;
    get_report_run(pool, report_run_id).await
}

pub async fn approve_report_outline(
    pool: &SqlitePool,
    input: ApproveOutlineInput,
) -> Result<ReportRun, String> {
    if input.outline_markdown.trim().is_empty() {
        return Err("An approved outline cannot be empty.".to_string());
    }
    sqlx::query(
        "UPDATE report_runs SET outline_markdown = ?, status = 'outline_approved', updated_at = ? WHERE id = ?",
    )
    .bind(&input.outline_markdown)
    .bind(crate::repo::now_utc())
    .bind(&input.report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    provenance::record_event(pool, &input.report_run_id, "outline_approved", json!({})).await?;
    get_report_run(pool, &input.report_run_id).await
}

pub async fn generate_report_draft(
    pool: &SqlitePool,
    brain_path: &Path,
    api_key: &str,
    report_run_id: &str,
) -> Result<ReportRun, String> {
    let report = get_report_run(pool, report_run_id).await?;
    if report.status != "outline_approved" && report.status != "drafted" {
        return Err("Approve the outline before generating the report draft.".to_string());
    }
    let report_type = parse_report_type(&report.report_type)?;
    let scope = parse_scope(&report.scope_json)?;
    let initiative_titles = load_initiative_titles(pool, &scope.initiative_ids).await?;
    let result = crate::reasoning::query_reasoning_graph(
        pool,
        brain_path,
        api_key,
        ReasoningQueryInput {
            query: templates::draft_prompt(
                report_type,
                &report.title,
                &scope,
                &initiative_titles,
                &report.outline_markdown,
            ),
            retrieval_query: Some(format!(
                "{} {} draft",
                initiative_titles.join(" "),
                report.title
            )),
            depth: ReasoningDepth::Deep,
            query_mode: templates::query_mode(report_type, "draft"),
            scope,
            purpose: Some(format!("report:{}:draft", report.id)),
        },
    )
    .await?;
    sqlx::query(
        r#"UPDATE report_runs SET draft_markdown = ?, status = 'drafted',
           citations_json = ?, unsupported_json = ?, reasoning_run_id = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&result.answer_markdown)
    .bind(serde_json::to_string(&result.citations).unwrap_or_else(|_| "[]".to_string()))
    .bind(
        serde_json::to_string(&result.unsupported_assertions).unwrap_or_else(|_| "[]".to_string()),
    )
    .bind(&result.run_id)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    citations::replace_report_sources(pool, report_run_id, &result.citations).await?;
    provenance::record_event(
        pool,
        report_run_id,
        "draft_generated",
        json!({ "reasoning_run_id": result.run_id, "mode": result.query_mode }),
    )
    .await?;
    get_report_run(pool, report_run_id).await
}

pub async fn approve_report(pool: &SqlitePool, report_run_id: &str) -> Result<ReportRun, String> {
    let report = get_report_run(pool, report_run_id).await?;
    if report.status != "drafted" {
        return Err("Only a drafted report can be approved.".to_string());
    }
    let now = crate::repo::now_utc();
    sqlx::query(
        "UPDATE report_runs SET status = 'approved', approved_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    provenance::record_event(pool, report_run_id, "approved", json!({})).await?;
    get_report_run(pool, report_run_id).await
}

pub async fn link_report_deliverable(
    pool: &SqlitePool,
    input: LinkReportDeliverableInput,
) -> Result<ReportRun, String> {
    sqlx::query("UPDATE report_runs SET deliverable_id = ?, updated_at = ? WHERE id = ?")
        .bind(&input.deliverable_id)
        .bind(crate::repo::now_utc())
        .bind(&input.report_run_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    provenance::record_event(
        pool,
        &input.report_run_id,
        "deliverable_linked",
        json!({ "deliverable_id": input.deliverable_id }),
    )
    .await?;
    get_report_run(pool, &input.report_run_id).await
}

pub async fn create_report_deliverable(
    pool: &SqlitePool,
    report_run_id: &str,
) -> Result<Deliverable, String> {
    let report = get_report_run(pool, report_run_id).await?;
    if report.status != "approved" {
        return Err("Approve the report before creating its deliverable.".to_string());
    }
    if report.deliverable_id.is_some() {
        return Err("This report already has a linked deliverable.".to_string());
    }
    let scope = parse_scope(&report.scope_json)?;
    let claim = report
        .draft_markdown
        .lines()
        .find(|line| !line.trim().is_empty() && !line.starts_with('>'))
        .unwrap_or("Approved strategic report")
        .trim_matches('#')
        .trim()
        .to_string();
    let deliverable = crate::repo::create_deliverable(
        pool,
        CreateDeliverableInput {
            title: report.title.clone(),
            deliverable_type: DeliverableType::Report,
            state: DeliverableState::Drafting,
            claim,
            artifact_url: None,
            conversation_id: None,
            stakeholder_id: scope.stakeholder_ids.first().cloned(),
            stakeholder_ids: scope.stakeholder_ids,
            initiative_ids: scope.initiative_ids,
        },
    )
    .await?;
    link_report_deliverable(
        pool,
        LinkReportDeliverableInput {
            report_run_id: report_run_id.to_string(),
            deliverable_id: deliverable.id.clone(),
        },
    )
    .await?;
    Ok(deliverable)
}

fn parse_scope(scope_json: &str) -> Result<ReasoningScope, String> {
    serde_json::from_str(scope_json)
        .map_err(|error| format!("Stored report scope is invalid: {error}"))
}

async fn load_initiative_titles(
    pool: &SqlitePool,
    initiative_ids: &[String],
) -> Result<Vec<String>, String> {
    if initiative_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; initiative_ids.len()].join(",");
    let sql = format!("SELECT title FROM initiatives WHERE id IN ({placeholders})");
    let mut query = sqlx::query_scalar::<_, String>(&sql);
    for id in initiative_ids {
        query = query.bind(id);
    }
    query.fetch_all(pool).await.map_err(sql_error)
}

fn parse_report_type(value: &str) -> Result<ReportType, String> {
    match value {
        "quarterly" => Ok(ReportType::Quarterly),
        "initiative" => Ok(ReportType::Initiative),
        "decision_memo" => Ok(ReportType::DecisionMemo),
        _ => Err("Stored report type is invalid.".to_string()),
    }
}

fn parse_exclusions(report: &ReportRun) -> Vec<String> {
    serde_json::from_str(&report.scope_exclusions_json).unwrap_or_default()
}

pub async fn list_report_steps_for(
    pool: &SqlitePool,
    report_run_id: &str,
) -> Result<Vec<ReportStep>, String> {
    steps::list_steps(pool, report_run_id).await
}

pub async fn replace_scope_exclusions(
    pool: &SqlitePool,
    report_run_id: &str,
    excluded_entry_ids: &[String],
) -> Result<ReportRun, String> {
    let value =
        serde_json::to_string(excluded_entry_ids).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        "UPDATE report_runs SET scope_exclusions_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&value)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    provenance::record_event(
        pool,
        report_run_id,
        "scope_exclusions_updated",
        json!({ "count": excluded_entry_ids.len() }),
    )
    .await?;
    get_report_run(pool, report_run_id).await
}

pub async fn update_sections(
    pool: &SqlitePool,
    report_run_id: &str,
    sections: &[ReportSectionPlan],
) -> Result<ReportRun, String> {
    let value = serde_json::to_string(sections).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        "UPDATE report_runs SET sections_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&value)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    provenance::record_event(
        pool,
        report_run_id,
        "sections_updated",
        json!({ "count": sections.len() }),
    )
    .await?;
    get_report_run(pool, report_run_id).await
}

/// Resolve the report's scope to a flat list of entities for the ScopePreview
/// panel. The frontend renders the entries grouped by kind and lets the user
/// untick any item; the un-ticked ids land back via [`replace_scope_exclusions`].
pub async fn scope_preview(
    pool: &SqlitePool,
    report_run_id: &str,
) -> Result<ReportScopePreview, String> {
    let report = get_report_run(pool, report_run_id).await?;
    let scope = parse_scope(&report.scope_json)?;
    let exclusions = parse_exclusions(&report);
    let resolved = reasoning::resolve_scope(pool, &scope, &exclusions).await?;

    let mut entries: Vec<ReportScopeEntry> = Vec::new();
    for (id, title) in zip_ids_titles(
        pool,
        "SELECT id, title FROM initiatives WHERE id",
        &resolved.initiative_ids,
    )
    .await?
    {
        entries.push(ReportScopeEntry {
            id: scope_entry_id("initiative", &id),
            kind: "initiative".to_string(),
            title,
            subtitle: None,
        });
    }
    for (id, title) in zip_ids_titles(
        pool,
        "SELECT id, title FROM deliverables WHERE id",
        &resolved.deliverable_ids,
    )
    .await?
    {
        entries.push(ReportScopeEntry {
            id: scope_entry_id("deliverable", &id),
            kind: "deliverable".to_string(),
            title,
            subtitle: None,
        });
    }
    for (id, name) in zip_ids_titles(
        pool,
        "SELECT id, name FROM stakeholders WHERE id",
        &resolved.stakeholder_ids,
    )
    .await?
    {
        entries.push(ReportScopeEntry {
            id: scope_entry_id("stakeholder", &id),
            kind: "stakeholder".to_string(),
            title: name,
            subtitle: None,
        });
    }
    for (thread_id, subject) in zip_ids_titles(
        pool,
        "SELECT thread_id AS id, COALESCE(subject, '(no subject)') AS title FROM gmail_threads WHERE thread_id",
        &resolved.email_thread_ids,
    )
    .await?
    {
        entries.push(ReportScopeEntry {
            id: scope_entry_id("email_thread", &thread_id),
            kind: "email_thread".to_string(),
            title: subject,
            subtitle: None,
        });
    }
    for (id, title) in zip_ids_titles(
        pool,
        "SELECT id, title FROM meetings WHERE id",
        &resolved.meeting_ids,
    )
    .await?
    {
        entries.push(ReportScopeEntry {
            id: scope_entry_id("meeting", &id),
            kind: "meeting".to_string(),
            title,
            subtitle: None,
        });
    }
    for (id, name) in zip_ids_titles(
        pool,
        "SELECT id, name AS title FROM files WHERE id",
        &resolved.file_ids,
    )
    .await?
    {
        entries.push(ReportScopeEntry {
            id: scope_entry_id("file", &id),
            kind: "file".to_string(),
            title: name,
            subtitle: None,
        });
    }
    for (id, title) in zip_ids_titles(
        pool,
        "SELECT id, COALESCE(title, '(no title)') AS title FROM gcal_events WHERE id",
        &resolved.calendar_event_ids,
    )
    .await?
    {
        entries.push(ReportScopeEntry {
            id: scope_entry_id("calendar_event", &id),
            kind: "calendar_event".to_string(),
            title,
            subtitle: None,
        });
    }

    let total = entries.len() as i64;
    Ok(ReportScopePreview {
        initiative_ids: resolved.initiative_ids,
        initiative_titles: resolved.initiative_titles,
        deliverable_ids: resolved.deliverable_ids,
        stakeholder_ids: resolved.stakeholder_ids,
        email_thread_ids: resolved.email_thread_ids,
        meeting_ids: resolved.meeting_ids,
        file_ids: resolved.file_ids,
        calendar_event_ids: resolved.calendar_event_ids,
        entries,
        total_entries: total,
        excluded_entry_ids: exclusions,
    })
}

/// Stable id used by the ScopePreview UI for exclusion checkboxes — matches the
/// `(source_kind, source_id)` tuple in `ResolvedScope::source_unit_membership`.
fn scope_entry_id(kind: &str, source_id: &str) -> String {
    format!("{kind}:{source_id}")
}

async fn zip_ids_titles(
    pool: &SqlitePool,
    sql_prefix: &str,
    ids: &[String],
) -> Result<Vec<(String, String)>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("{sql_prefix} IN ({placeholders})");
    let mut query = sqlx::query_as::<_, (String, String)>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    query.fetch_all(pool).await.map_err(sql_error)
}
