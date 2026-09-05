//! Phase 2 brain explorer extras: saved views + graph community summaries.
//!
//! These read/write SQLite directly — no Kuzu involvement. Both surfaces are
//! pure projections of existing data, so they don't require a brain rebuild
//! after a mutation.

use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{FromRow, Row, SqlitePool};
use ulid::Ulid;

use crate::db::sql_error;
use crate::models::{
    GraphCommunityMemberSummary, GraphCommunitySummary, SaveBrainViewInput, SavedBrainView,
};

// ──────────────────────────────────────────────────────────────────────────
// Saved views
// ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow)]
struct SavedBrainViewRow {
    id: String,
    name: String,
    description: String,
    filters_json: String,
    layout_json: String,
    viewport_json: String,
    pinned_json: String,
    created_at: i64,
    updated_at: i64,
}

impl From<SavedBrainViewRow> for SavedBrainView {
    fn from(row: SavedBrainViewRow) -> Self {
        SavedBrainView {
            id: row.id,
            name: row.name,
            description: row.description,
            filters: parse_value(&row.filters_json).unwrap_or_else(|| json!({})),
            layout: parse_value(&row.layout_json).unwrap_or_else(|| json!({})),
            viewport: parse_value(&row.viewport_json).unwrap_or_else(|| json!({})),
            pinned: parse_value(&row.pinned_json).unwrap_or_else(|| json!([])),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn parse_value(raw: &str) -> Option<Value> {
    serde_json::from_str(raw).ok()
}

pub async fn list_saved_brain_views(pool: &SqlitePool) -> Result<Vec<SavedBrainView>, String> {
    let rows = sqlx::query_as::<_, SavedBrainViewRow>(
        r#"
        SELECT id, name, description, filters_json, layout_json,
               viewport_json, pinned_json, created_at, updated_at
        FROM brain_saved_views
        ORDER BY updated_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    Ok(rows.into_iter().map(SavedBrainView::from).collect())
}

pub async fn save_brain_view(
    pool: &SqlitePool,
    input: SaveBrainViewInput,
) -> Result<SavedBrainView, String> {
    let now = Utc::now().timestamp_millis();
    let id = input.id.clone().unwrap_or_else(|| Ulid::new().to_string());
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("saved view name cannot be empty".into());
    }
    let description = input.description.unwrap_or_default();
    let filters_json = input.filters.to_string();
    let layout_json = input.layout.to_string();
    let viewport_json = input.viewport.to_string();
    let pinned_json = input.pinned.to_string();

    sqlx::query(
        r#"
        INSERT INTO brain_saved_views
            (id, name, description, filters_json, layout_json, viewport_json,
             pinned_json, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name           = excluded.name,
            description    = excluded.description,
            filters_json   = excluded.filters_json,
            layout_json    = excluded.layout_json,
            viewport_json  = excluded.viewport_json,
            pinned_json    = excluded.pinned_json,
            updated_at     = excluded.updated_at
        "#,
    )
    .bind(&id)
    .bind(&name)
    .bind(&description)
    .bind(&filters_json)
    .bind(&layout_json)
    .bind(&viewport_json)
    .bind(&pinned_json)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    let row = sqlx::query_as::<_, SavedBrainViewRow>(
        r#"
        SELECT id, name, description, filters_json, layout_json,
               viewport_json, pinned_json, created_at, updated_at
        FROM brain_saved_views
        WHERE id = ?
        "#,
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    Ok(SavedBrainView::from(row))
}

pub async fn delete_brain_view(pool: &SqlitePool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM brain_saved_views WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────
// Graph communities (GraphRAG clusters)
// ──────────────────────────────────────────────────────────────────────────

pub async fn list_graph_communities(
    pool: &SqlitePool,
    level: Option<i64>,
) -> Result<Vec<GraphCommunitySummary>, String> {
    let level_filter = level;
    let community_rows = if let Some(lvl) = level_filter {
        sqlx::query(
            r#"
            SELECT id, community_kind, scope_key, title, level, status,
                   created_at, updated_at
            FROM graph_communities
            WHERE status = 'active' AND level = ?
            ORDER BY level ASC, updated_at DESC
            "#,
        )
        .bind(lvl)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            r#"
            SELECT id, community_kind, scope_key, title, level, status,
                   created_at, updated_at
            FROM graph_communities
            WHERE status = 'active'
            ORDER BY level ASC, updated_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
    }
    .map_err(sql_error)?;

    let mut summaries: Vec<GraphCommunitySummary> = Vec::with_capacity(community_rows.len());
    for row in community_rows {
        let id: String = row.get("id");
        let community_kind: String = row.get("community_kind");
        let scope_key: String = row.get("scope_key");
        let title: String = row.get("title");
        let level: i64 = row.get("level");
        let status: String = row.get("status");
        let created_at: String = row.get("created_at");
        let updated_at: String = row.get("updated_at");

        let report_summary: Option<String> = sqlx::query_scalar::<_, String>(
            r#"
            SELECT summary_markdown
            FROM community_reports
            WHERE community_id = ? AND status IN ('approved','pending')
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;

        let member_rows = sqlx::query(
            r#"
            SELECT entity_kind, entity_id
            FROM graph_community_members
            WHERE community_id = ?
            "#,
        )
        .bind(&id)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?;

        let members: Vec<GraphCommunityMemberSummary> = member_rows
            .into_iter()
            .map(|m| GraphCommunityMemberSummary {
                entity_kind: m.get("entity_kind"),
                entity_id: m.get("entity_id"),
            })
            .collect();

        summaries.push(GraphCommunitySummary {
            id,
            community_kind,
            scope_key,
            title,
            level,
            status,
            summary_markdown: report_summary,
            member_count: members.len() as i64,
            members,
            created_at,
            updated_at,
        });
    }
    Ok(summaries)
}
