//! Report pipeline persistence helpers. From orchestrator.rs (13-std7).

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::db::sql_error;
use crate::models::{ReportCritique, ReportSectionDraft, ReportSectionPlan};

pub(super) async fn persist_sections(
    pool: &SqlitePool,
    report_run_id: &str,
    sections_list: &[ReportSectionPlan],
) -> Result<(), String> {
    let value = serde_json::to_string(sections_list).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        "UPDATE report_runs SET sections_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(value)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub(super) async fn persist_section_drafts(
    pool: &SqlitePool,
    report_run_id: &str,
    drafts: &HashMap<String, ReportSectionDraft>,
) -> Result<(), String> {
    let value = serde_json::to_string(drafts).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        "UPDATE report_runs SET section_drafts_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(value)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub(super) async fn persist_critique(
    pool: &SqlitePool,
    report_run_id: &str,
    critique: &ReportCritique,
) -> Result<(), String> {
    let value = serde_json::to_string(critique).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        "UPDATE report_runs SET critique_json = ?, updated_at = ? WHERE id = ?",
    )
    .bind(value)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub(super) async fn update_outline_markdown(
    pool: &SqlitePool,
    report_run_id: &str,
    sections_list: &[ReportSectionPlan],
) -> Result<(), String> {
    let mut md = String::new();
    for (idx, section) in sections_list.iter().enumerate() {
        md.push_str(&format!("### {}. {}\n\n", idx + 1, section.heading));
        if !section.instructions.is_empty() {
            md.push_str(&format!("> {}\n\n", section.instructions));
        }
    }
    sqlx::query(
        "UPDATE report_runs SET outline_markdown = ?, updated_at = ? WHERE id = ?",
    )
    .bind(md)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub(super) async fn update_draft_markdown(
    pool: &SqlitePool,
    report_run_id: &str,
    sections_list: &[ReportSectionPlan],
    drafts: &HashMap<String, ReportSectionDraft>,
) -> Result<(), String> {
    let mut md = String::new();
    for section in sections_list {
        md.push_str(&format!("## {}\n\n", section.heading));
        if let Some(draft) = drafts.get(&section.id) {
            md.push_str(draft.markdown.trim());
            md.push_str("\n\n");
        } else {
            md.push_str("_(no draft)_\n\n");
        }
    }
    sqlx::query(
        "UPDATE report_runs SET draft_markdown = ?, updated_at = ? WHERE id = ?",
    )
    .bind(md)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub(super) async fn advance_status(
    pool: &SqlitePool,
    report_run_id: &str,
    status: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"UPDATE report_runs SET status = ?, updated_at = ? WHERE id = ?
           AND status NOT IN ('approved', 'discarded')"#,
    )
    .bind(status)
    .bind(crate::repo::now_utc())
    .bind(report_run_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

