mod links;
mod search;
mod state;

pub use links::{
    fetch_initiatives_for_deliverable, fetch_labels_for_deliverable,
    fetch_stakeholders_for_deliverable, replace_initiative_links, replace_stakeholder_links,
};
pub use search::{
    resolve_initiative_title, resolve_initiative_titles, resolve_stakeholder_name,
    search_deliverables, valid_initiative_titles, valid_stakeholder_names,
};
pub use state::{
    shipped_at_for_state, update_deliverable_state, update_deliverable_state_friction,
    update_deliverable_state_with_friction,
};

use sqlx::{Sqlite, SqlitePool, Transaction};
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{
        CreateDeliverableInput, Deliverable, DeliverableFilters, DeliverableRow, DeliverableState,
        DeliverableType, UpdateDeliverableInput,
    },
};

use super::{clean_ids, clean_optional, create_or_get_conversation, now_utc};

#[derive(Debug, Clone)]
pub struct CleanDeliverableInput {
    pub title: String,
    pub deliverable_type: DeliverableType,
    pub state: DeliverableState,
    pub claim: String,
    pub artifact_url: Option<String>,
    pub conversation_id: Option<String>,
    pub stakeholder_id: Option<String>,
    pub stakeholder_ids: Vec<String>,
    pub initiative_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CreateDeliverableByNameInput {
    pub title: String,
    pub deliverable_type: DeliverableType,
    pub claim: String,
    pub initiative_titles: Vec<String>,
    pub stakeholder_name: Option<String>,
    pub artifact_url: Option<String>,
    pub conversation_url: Option<String>,
}

pub async fn list_deliverables(
    pool: &SqlitePool,
    filters: DeliverableFilters,
) -> Result<Vec<Deliverable>, String> {
    let state_filter = filters.state.map(|value| value.as_str().to_string());
    let type_filter = filters
        .deliverable_type
        .map(|value| value.as_str().to_string());
    let priority_filter = filters.priority.clone();

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
        FROM deliverables d
        LEFT JOIN conversations c ON c.id = d.conversation_id
        LEFT JOIN stakeholders s ON s.id = d.stakeholder_id
        WHERE (? IS NULL OR d.state = ?)
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
        ORDER BY
          CASE d.state
            WHEN 'backlog'   THEN 0
            WHEN 'todo'      THEN 1
            WHEN 'drafting'  THEN 2
            WHEN 'in_review' THEN 3
            WHEN 'shipped'   THEN 4
            ELSE 5
          END,
          COALESCE(d.display_order, 0) ASC,
          d.updated_at DESC
        "#,
    )
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
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut result = hydrate_deliverables(pool, rows).await?;

    // state_in: post-fetch filter (OR semantics across multiple states)
    if let Some(states) = filters.state_in {
        if !states.is_empty() {
            let state_strs: Vec<&'static str> = states.iter().map(|s| s.as_str()).collect();
            result.retain(|d| state_strs.iter().any(|s| *s == d.state));
        }
    }

    Ok(result)
}
pub async fn list_deliverables_for_initiative(
    pool: &SqlitePool,
    initiative_id: &str,
) -> Result<Vec<Deliverable>, String> {
    list_deliverables(
        pool,
        DeliverableFilters {
            initiative_id: Some(initiative_id.to_string()),
            ..DeliverableFilters::default()
        },
    )
    .await
}
pub async fn get_deliverable(pool: &SqlitePool, id: &str) -> Result<Deliverable, String> {
    let row = sqlx::query_as::<_, DeliverableRow>(
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
        FROM deliverables d
        LEFT JOIN conversations c ON c.id = d.conversation_id
        LEFT JOIN stakeholders s ON s.id = d.stakeholder_id
        WHERE d.id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "deliverable not found".to_string())?;

    let initiatives = fetch_initiatives_for_deliverable(pool, id).await?;
    let stakeholders = fetch_stakeholders_for_deliverable(pool, id).await?;
    let labels = fetch_labels_for_deliverable(pool, id).await?;
    Ok(row.with_refs(initiatives, stakeholders, labels))
}
pub async fn create_deliverable(
    pool: &SqlitePool,
    input: CreateDeliverableInput,
) -> Result<Deliverable, String> {
    let input = validate_deliverable_input(
        input.title,
        input.deliverable_type,
        input.state,
        input.claim,
        input.artifact_url,
        input.conversation_id,
        input.stakeholder_id,
        input.stakeholder_ids,
        input.initiative_ids,
    )?;

    ensure_references_exist(
        pool,
        &input.initiative_ids,
        &input.stakeholder_ids,
        input.conversation_id.as_deref(),
    )
    .await?;

    let id = Ulid::new().to_string();
    let now = now_utc();
    let shipped_at = shipped_at_for_state(input.state, None, &now);
    let mut tx = pool.begin().await.map_err(sql_error)?;

    insert_deliverable_in_tx(&mut tx, &id, &input, &now, shipped_at.as_deref()).await?;
    replace_initiative_links(&mut tx, &id, &input.initiative_ids).await?;
    replace_stakeholder_links(&mut tx, &id, &input.stakeholder_ids).await?;
    tx.commit().await.map_err(sql_error)?;

    get_deliverable(pool, &id).await
}

pub async fn update_deliverable(
    pool: &SqlitePool,
    id: &str,
    input: UpdateDeliverableInput,
) -> Result<Deliverable, String> {
    let input = validate_deliverable_input(
        input.title,
        input.deliverable_type,
        input.state,
        input.claim,
        input.artifact_url,
        input.conversation_id,
        input.stakeholder_id,
        input.stakeholder_ids,
        input.initiative_ids,
    )?;

    ensure_references_exist(
        pool,
        &input.initiative_ids,
        &input.stakeholder_ids,
        input.conversation_id.as_deref(),
    )
    .await?;

    let current_shipped_at = current_shipped_at(pool, id).await?;
    let now = now_utc();
    let shipped_at = shipped_at_for_state(input.state, current_shipped_at, &now);
    let mut tx = pool.begin().await.map_err(sql_error)?;

    let result = sqlx::query(
        r#"
        UPDATE deliverables
        SET title = ?,
            type = ?,
            state = ?,
            claim = ?,
            artifact_url = ?,
            conversation_id = ?,
            stakeholder_id = ?,
            shipped_at = ?,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&input.title)
    .bind(input.deliverable_type.as_str())
    .bind(input.state.as_str())
    .bind(&input.claim)
    .bind(&input.artifact_url)
    .bind(&input.conversation_id)
    .bind(&input.stakeholder_id)
    .bind(&shipped_at)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("deliverable not found".to_string());
    }

    replace_initiative_links(&mut tx, id, &input.initiative_ids).await?;
    replace_stakeholder_links(&mut tx, id, &input.stakeholder_ids).await?;
    tx.commit().await.map_err(sql_error)?;

    get_deliverable(pool, id).await
}
pub async fn delete_deliverable(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM deliverables WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("deliverable not found".to_string());
    }

    Ok(())
}
pub async fn create_deliverable_by_name(
    pool: &SqlitePool,
    input: CreateDeliverableByNameInput,
) -> Result<Deliverable, String> {
    let initiative_ids = resolve_initiative_titles(pool, &input.initiative_titles).await?;
    let stakeholder_id = match input.stakeholder_name {
        Some(name) if !name.trim().is_empty() => Some(resolve_stakeholder_name(pool, &name).await?),
        _ => None,
    };
    let conversation_id = match input.conversation_url {
        Some(url) if !url.trim().is_empty() => Some(
            create_or_get_conversation(pool, url.trim(), None, None, None)
                .await?
                .id,
        ),
        _ => None,
    };

    create_deliverable(
        pool,
        CreateDeliverableInput {
            title: input.title,
            deliverable_type: input.deliverable_type,
            state: DeliverableState::Drafting,
            claim: input.claim,
            artifact_url: input.artifact_url,
            conversation_id,
            stakeholder_id,
            stakeholder_ids: Vec::new(),
            initiative_ids,
        },
    )
    .await
}

pub async fn hydrate_deliverables(
    pool: &SqlitePool,
    rows: Vec<DeliverableRow>,
) -> Result<Vec<Deliverable>, String> {
    let mut deliverables = Vec::with_capacity(rows.len());

    for row in rows {
        let initiatives = fetch_initiatives_for_deliverable(pool, &row.id).await?;
        let stakeholders = fetch_stakeholders_for_deliverable(pool, &row.id).await?;
        let labels = fetch_labels_for_deliverable(pool, &row.id).await?;
        deliverables.push(row.with_refs(initiatives, stakeholders, labels));
    }

    Ok(deliverables)
}
pub async fn current_shipped_at(pool: &SqlitePool, id: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar::<_, Option<String>>("SELECT shipped_at FROM deliverables WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?
        .ok_or_else(|| "deliverable not found".to_string())
}

pub async fn ensure_references_exist(
    pool: &SqlitePool,
    initiative_ids: &[String],
    stakeholder_ids: &[String],
    conversation_id: Option<&str>,
) -> Result<(), String> {
    for initiative_id in initiative_ids {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM initiatives WHERE id = ?")
            .bind(initiative_id)
            .fetch_one(pool)
            .await
            .map_err(sql_error)?;

        if exists == 0 {
            return Err(format!("initiative not found: {initiative_id}"));
        }
    }

    for stakeholder_id in stakeholder_ids {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stakeholders WHERE id = ?")
            .bind(stakeholder_id)
            .fetch_one(pool)
            .await
            .map_err(sql_error)?;

        if exists == 0 {
            return Err(format!("stakeholder not found: {stakeholder_id}"));
        }
    }

    if let Some(conversation_id) = conversation_id {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE id = ?")
            .bind(conversation_id)
            .fetch_one(pool)
            .await
            .map_err(sql_error)?;

        if exists == 0 {
            return Err(format!("conversation not found: {conversation_id}"));
        }
    }

    Ok(())
}
pub fn validate_deliverable_input(
    title: String,
    deliverable_type: DeliverableType,
    state: DeliverableState,
    claim: String,
    artifact_url: Option<String>,
    conversation_id: Option<String>,
    stakeholder_id: Option<String>,
    stakeholder_ids: Vec<String>,
    initiative_ids: Vec<String>,
) -> Result<CleanDeliverableInput, String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("deliverable title is required".to_string());
    }

    let claim = claim.trim().to_string();
    if claim.is_empty() {
        return Err("deliverable claim is required".to_string());
    }

    let initiative_ids = clean_ids(initiative_ids);
    // Backlog items are allowed without an initiative (they are unrefined ideas)
    if initiative_ids.is_empty() && state != DeliverableState::Backlog {
        return Err("at least one initiative is required".to_string());
    }

    let mut stakeholder_ids = clean_ids(stakeholder_ids);
    if stakeholder_ids.is_empty() {
        if let Some(stakeholder_id) = clean_optional(stakeholder_id) {
            stakeholder_ids.push(stakeholder_id);
        }
    }
    let stakeholder_id = stakeholder_ids.first().cloned();

    Ok(CleanDeliverableInput {
        title,
        deliverable_type,
        state,
        claim,
        artifact_url: clean_optional(artifact_url),
        conversation_id: clean_optional(conversation_id),
        stakeholder_id,
        stakeholder_ids,
        initiative_ids,
    })
}
pub async fn insert_deliverable_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    input: &CleanDeliverableInput,
    now: &str,
    shipped_at: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO deliverables
          (id, title, type, state, claim, artifact_url, conversation_id, stakeholder_id,
           created_at, shipped_at, updated_at, state_changed_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(id)
    .bind(&input.title)
    .bind(input.deliverable_type.as_str())
    .bind(input.state.as_str())
    .bind(&input.claim)
    .bind(&input.artifact_url)
    .bind(&input.conversation_id)
    .bind(&input.stakeholder_id)
    .bind(now)
    .bind(shipped_at)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(sql_error)?;

    Ok(())
}
