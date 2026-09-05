use sqlx::SqlitePool;

use crate::db::sql_error;
use crate::models::ReasoningCitation;

pub(super) async fn replace_report_sources(
    pool: &SqlitePool,
    report_run_id: &str,
    citations: &[ReasoningCitation],
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(sql_error)?;
    sqlx::query("DELETE FROM report_sources WHERE report_run_id = ?")
        .bind(report_run_id)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    for citation in citations {
        sqlx::query(
            r#"INSERT INTO report_sources
               (report_run_id, source_unit_id, citation_label, used_in_sections_json)
               VALUES (?, ?, ?, '[]')"#,
        )
        .bind(report_run_id)
        .bind(&citation.source_unit_id)
        .bind(&citation.label)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    }
    tx.commit().await.map_err(sql_error)
}
