use sqlx::SqlitePool;

use crate::db::sql_error;
use crate::models::CommunityReport;

use super::sources::{content_hash, new_id};

pub(super) async fn refresh_canonical_communities(pool: &SqlitePool) -> Result<i64, String> {
    let now = crate::repo::now_utc();
    let initiatives: Vec<(String, String)> =
        sqlx::query_as("SELECT id, title FROM initiatives WHERE status != 'parked'")
            .fetch_all(pool)
            .await
            .map_err(sql_error)?;
    let stakeholders: Vec<(String, String)> = sqlx::query_as("SELECT id, name FROM stakeholders")
        .fetch_all(pool)
        .await
        .map_err(sql_error)?;
    for (id, title) in &initiatives {
        upsert_community(pool, "initiative", id, title, &now).await?;
        let community_id = format!("community:initiative:{id}");
        sqlx::query("DELETE FROM graph_community_members WHERE community_id = ?")
            .bind(&community_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        sqlx::query(
            "INSERT OR IGNORE INTO graph_community_members (community_id, entity_kind, entity_id, source, added_at) VALUES (?, 'initiative', ?, 'canonical', ?)",
        ).bind(&community_id).bind(id).bind(&now).execute(pool).await.map_err(sql_error)?;
        sqlx::query(
            r#"INSERT OR IGNORE INTO graph_community_members (community_id, entity_kind, entity_id, source, added_at)
               SELECT ?, 'deliverable', deliverable_id, 'canonical', ? FROM deliverable_initiatives WHERE initiative_id = ?"#,
        ).bind(&community_id).bind(&now).bind(id).execute(pool).await.map_err(sql_error)?;
    }
    for (id, name) in &stakeholders {
        upsert_community(pool, "stakeholder", id, name, &now).await?;
        let community_id = format!("community:stakeholder:{id}");
        sqlx::query("DELETE FROM graph_community_members WHERE community_id = ?")
            .bind(&community_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        sqlx::query(
            "INSERT OR IGNORE INTO graph_community_members (community_id, entity_kind, entity_id, source, added_at) VALUES (?, 'stakeholder', ?, 'canonical', ?)",
        ).bind(&community_id).bind(id).bind(&now).execute(pool).await.map_err(sql_error)?;
        sqlx::query(
            r#"INSERT OR IGNORE INTO graph_community_members (community_id, entity_kind, entity_id, source, added_at)
               SELECT ?, 'deliverable', deliverable_id, 'canonical', ? FROM deliverable_stakeholders WHERE stakeholder_id = ?"#,
        ).bind(&community_id).bind(&now).bind(id).execute(pool).await.map_err(sql_error)?;
    }
    Ok((initiatives.len() + stakeholders.len()) as i64)
}

async fn upsert_community(
    pool: &SqlitePool,
    kind: &str,
    key: &str,
    title: &str,
    now: &str,
) -> Result<(), String> {
    let id = format!("community:{kind}:{key}");
    sqlx::query(
        r#"INSERT INTO graph_communities
           (id, community_kind, scope_key, title, level, version_hash, status, created_at, updated_at)
           VALUES (?, ?, ?, ?, 0, ?, 'active', ?, ?)
           ON CONFLICT(id) DO UPDATE SET title = excluded.title,
             version_hash = excluded.version_hash, updated_at = excluded.updated_at"#,
    )
    .bind(id).bind(kind).bind(key).bind(title).bind(content_hash(title)).bind(now).bind(now)
    .execute(pool).await.map_err(sql_error)?;
    Ok(())
}

pub(super) async fn stage_report(
    pool: &SqlitePool,
    title: &str,
    markdown: &str,
    evidence_json: &str,
    model: &str,
) -> Result<(), String> {
    let now = crate::repo::now_utc();
    let community_id = "community:workspace:strategic";
    upsert_community(pool, "workspace", "strategic", "Strategic workspace", &now).await?;
    sqlx::query("DELETE FROM graph_community_members WHERE community_id = ?")
        .bind(community_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    for (entity_kind, table, condition) in [
        ("initiative", "initiatives", "status != 'parked'"),
        ("deliverable", "deliverables", "state != 'killed'"),
        ("stakeholder", "stakeholders", "1 = 1"),
    ] {
        let statement = format!(
            "INSERT OR IGNORE INTO graph_community_members (community_id, entity_kind, entity_id, source, added_at) SELECT ?, ?, id, 'canonical', ? FROM {table} WHERE {condition}"
        );
        sqlx::query(&statement)
            .bind(community_id)
            .bind(entity_kind)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }
    sqlx::query(
        r#"INSERT INTO community_reports
           (id, community_id, title, summary_markdown, evidence_json, version_hash, status, model, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?)"#,
    )
    .bind(new_id("community_report"))
    .bind(community_id)
    .bind(title)
    .bind(markdown)
    .bind(evidence_json)
    .bind(content_hash(markdown))
    .bind(model)
    .bind(&now)
    .bind(&now)
    .execute(pool).await.map_err(sql_error)?;
    Ok(())
}

pub async fn list_community_reports(
    pool: &SqlitePool,
    status: Option<&str>,
) -> Result<Vec<CommunityReport>, String> {
    sqlx::query_as::<_, CommunityReport>(
        r#"SELECT id, community_id, title, summary_markdown, evidence_json, version_hash,
           status, model, created_at, updated_at, reviewed_at
           FROM community_reports WHERE (? IS NULL OR status = ?) ORDER BY updated_at DESC LIMIT 100"#,
    )
    .bind(status).bind(status).fetch_all(pool).await.map_err(sql_error)
}

pub async fn review_community_report(
    pool: &SqlitePool,
    id: &str,
    decision: &str,
) -> Result<CommunityReport, String> {
    if !matches!(decision, "approved" | "rejected") {
        return Err("community report decision must be approved or rejected".to_string());
    }
    let now = crate::repo::now_utc();
    let changed = sqlx::query("UPDATE community_reports SET status = ?, reviewed_at = ?, updated_at = ? WHERE id = ? AND status = 'pending'")
        .bind(decision).bind(&now).bind(&now).bind(id).execute(pool).await.map_err(sql_error)?;
    if changed.rows_affected() == 0 {
        return Err("pending community report not found".to_string());
    }
    sqlx::query_as::<_, CommunityReport>(
        "SELECT id, community_id, title, summary_markdown, evidence_json, version_hash, status, model, created_at, updated_at, reviewed_at FROM community_reports WHERE id = ?",
    ).bind(id).fetch_one(pool).await.map_err(sql_error)
}
