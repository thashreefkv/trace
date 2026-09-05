use sqlx::SqlitePool;

use crate::{
    db::sql_error,
    models::{Deliverable, DeliverableFilters, DeliverableRow},
};

use super::super::{clean_ids, fts_query};
use super::hydrate_deliverables;

pub async fn search_deliverables(
    pool: &SqlitePool,
    query: &str,
    filters: DeliverableFilters,
    limit: i64,
) -> Result<Vec<Deliverable>, String> {
    let Some(match_query) = fts_query(query) else {
        return Ok(Vec::new());
    };

    let state_filter = filters.state.map(|value| value.as_str().to_string());
    let type_filter = filters
        .deliverable_type
        .map(|value| value.as_str().to_string());
    let priority_filter = filters.priority.clone();
    let limit = limit.clamp(1, 50);

    let rows = sqlx::query_as::<_, DeliverableRow>(
        r#"
        SELECT
          d.id,
          d.title,
          d.type AS deliverable_type,
          d.state,
          d.claim,
          d.artifact_url,
          d.conversation_id,
          c.chat_url AS conversation_url,
          d.stakeholder_id,
          s.name AS stakeholder_name,
          d.created_at,
          d.shipped_at,
          d.updated_at,
          d.deadline,
          COALESCE(d.is_focused, 0) AS is_focused,
          d.effort,
          d.impact,
          d.blocker_reason,
          d.state_changed_at,
          COALESCE(d.display_order, 0) AS display_order,
          d.priority
        FROM deliverable_search
        JOIN deliverables d ON d.rowid = deliverable_search.rowid
        LEFT JOIN conversations c ON c.id = d.conversation_id
        LEFT JOIN stakeholders s ON s.id = d.stakeholder_id
        WHERE deliverable_search MATCH ?
          AND (? IS NULL OR d.state = ?)
          AND (? IS NULL OR d.type = ?)
          AND (
            ? IS NULL
            OR d.stakeholder_id = ?
            OR EXISTS (
              SELECT 1
              FROM deliverable_stakeholders ds
              WHERE ds.deliverable_id = d.id
                AND ds.stakeholder_id = ?
            )
          )
          AND (
            ? IS NULL OR EXISTS (
              SELECT 1
              FROM deliverable_initiatives di
              WHERE di.deliverable_id = d.id
                AND di.initiative_id = ?
            )
          )
          AND (? IS NULL OR d.priority = ?)
        ORDER BY bm25(deliverable_search), d.updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(match_query)
    .bind(state_filter.clone())
    .bind(state_filter)
    .bind(type_filter.clone())
    .bind(type_filter)
    .bind(filters.stakeholder_id.clone())
    .bind(filters.stakeholder_id.clone())
    .bind(filters.stakeholder_id)
    .bind(filters.initiative_id.clone())
    .bind(filters.initiative_id)
    .bind(priority_filter.clone())
    .bind(priority_filter)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    hydrate_deliverables(pool, rows).await
}
pub async fn resolve_initiative_title(pool: &SqlitePool, title: &str) -> Result<String, String> {
    let title = title.trim();
    let found: Option<String> = sqlx::query_scalar("SELECT id FROM initiatives WHERE title = ?")
        .bind(title)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;

    match found {
        Some(id) => Ok(id),
        None => Err(format!(
            "initiative not found: {title}. Valid initiatives: {}",
            valid_initiative_titles(pool).await?.join(", ")
        )),
    }
}

pub async fn resolve_initiative_titles(
    pool: &SqlitePool,
    titles: &[String],
) -> Result<Vec<String>, String> {
    let titles = clean_ids(titles.to_vec());
    if titles.is_empty() {
        return Err("at least one initiative title is required".to_string());
    }

    let mut ids = Vec::with_capacity(titles.len());
    for title in titles {
        ids.push(resolve_initiative_title(pool, &title).await?);
    }
    Ok(ids)
}

pub async fn resolve_stakeholder_name(pool: &SqlitePool, name: &str) -> Result<String, String> {
    let name = name.trim();
    let found: Option<String> = sqlx::query_scalar("SELECT id FROM stakeholders WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;

    match found {
        Some(id) => Ok(id),
        None => Err(format!(
            "stakeholder not found: {name}. Valid stakeholders: {}",
            valid_stakeholder_names(pool).await?.join(", ")
        )),
    }
}

pub async fn valid_initiative_titles(pool: &SqlitePool) -> Result<Vec<String>, String> {
    sqlx::query_scalar("SELECT title FROM initiatives ORDER BY title ASC")
        .fetch_all(pool)
        .await
        .map_err(sql_error)
}

pub async fn valid_stakeholder_names(pool: &SqlitePool) -> Result<Vec<String>, String> {
    sqlx::query_scalar("SELECT name FROM stakeholders ORDER BY display_order ASC, name ASC")
        .fetch_all(pool)
        .await
        .map_err(sql_error)
}
