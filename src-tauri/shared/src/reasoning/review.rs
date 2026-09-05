use sqlx::SqlitePool;

use crate::db::sql_error;
use crate::models::AgentReviewItem;

use super::sources::new_id;

pub(super) async fn record_proposal(
    pool: &SqlitePool,
    source_kind: &str,
    source_id: &str,
    proposal_type: &str,
    proposed_change: &serde_json::Value,
    rationale: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO agent_review_items
           (id, source_kind, source_id, proposal_type, proposed_change_json, rationale, status, created_at)
           VALUES (?, ?, ?, ?, ?, ?, 'pending', ?)"#,
    )
    .bind(new_id("review"))
    .bind(source_kind)
    .bind(source_id)
    .bind(proposal_type)
    .bind(proposed_change.to_string())
    .bind(rationale)
    .bind(crate::repo::now_utc())
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn list_review_items(
    pool: &SqlitePool,
    status: Option<&str>,
) -> Result<Vec<AgentReviewItem>, String> {
    sqlx::query_as::<_, AgentReviewItem>(
        r#"SELECT id, source_kind, source_id, proposal_type, proposed_change_json,
           rationale, status, created_at, resolved_at FROM agent_review_items
           WHERE (? IS NULL OR status = ?) ORDER BY created_at DESC LIMIT 100"#,
    )
    .bind(status)
    .bind(status)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn review_item(
    pool: &SqlitePool,
    id: &str,
    decision: &str,
) -> Result<AgentReviewItem, String> {
    if !matches!(decision, "approved" | "rejected") {
        return Err("review decision must be approved or rejected".to_string());
    }
    let changed = sqlx::query(
        "UPDATE agent_review_items SET status = ?, resolved_at = ? WHERE id = ? AND status = 'pending'",
    )
    .bind(decision)
    .bind(crate::repo::now_utc())
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    if changed.rows_affected() == 0 {
        return Err("pending review item not found".to_string());
    }
    sqlx::query_as::<_, AgentReviewItem>(
        r#"SELECT id, source_kind, source_id, proposal_type, proposed_change_json,
           rationale, status, created_at, resolved_at FROM agent_review_items WHERE id = ?"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}
