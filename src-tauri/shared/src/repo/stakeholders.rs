use chrono::Utc;
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{CreateStakeholderInput, Stakeholder, StakeholderDetail, UpdateStakeholderInput},
};

pub async fn list_stakeholders(pool: &SqlitePool) -> Result<Vec<Stakeholder>, String> {
    sqlx::query_as::<_, Stakeholder>(
        r#"
        SELECT id, name, display_order,
               COALESCE(email, '') AS email,
               COALESCE(role, '')  AS role,
               COALESCE(notes, '') AS notes,
               COALESCE(avatar_url, '') AS avatar_url,
               created_at, updated_at
        FROM stakeholders
        ORDER BY display_order ASC, name ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn create_stakeholder(
    pool: &SqlitePool,
    input: CreateStakeholderInput,
) -> Result<Stakeholder, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("stakeholder name is required".to_string());
    }
    let email = normalize_email_optional(&input.email)?;

    let id = Ulid::new().to_string();
    let display_order = next_display_order(pool).await?;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO stakeholders (id, name, email, role, notes, display_order, avatar_url, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&name)
    .bind(&email)
    .bind(input.role.trim())
    .bind(input.notes.trim())
    .bind(display_order)
    .bind("")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if !email.is_empty() {
        let _ = crate::gmail::backfill_stakeholder_thread_links(pool, &id, &email).await;
    }

    get_stakeholder(pool, &id).await
}

pub async fn get_stakeholder(pool: &SqlitePool, id: &str) -> Result<Stakeholder, String> {
    sqlx::query_as::<_, Stakeholder>(
        r#"
        SELECT id, name, display_order,
               COALESCE(email, '') AS email,
               COALESCE(role, '')  AS role,
               COALESCE(notes, '') AS notes,
               COALESCE(avatar_url, '') AS avatar_url,
               created_at, updated_at
        FROM stakeholders
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}

pub async fn update_stakeholder(
    pool: &SqlitePool,
    id: &str,
    input: UpdateStakeholderInput,
) -> Result<Stakeholder, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("stakeholder name is required".to_string());
    }
    let email = normalize_email_optional(&input.email)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE stakeholders SET name = ?, email = ?, role = ?, notes = ?, updated_at = ? WHERE id = ?")
        .bind(&name)
        .bind(&email)
        .bind(input.role.trim())
        .bind(input.notes.trim())
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    if !email.is_empty() {
        let _ = crate::gmail::backfill_stakeholder_thread_links(pool, id, &email).await;
    }

    get_stakeholder(pool, id).await
}

pub async fn delete_stakeholder(pool: &SqlitePool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM stakeholders WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn get_stakeholder_detail(
    pool: &SqlitePool,
    id: &str,
) -> Result<StakeholderDetail, String> {
    let stakeholder = get_stakeholder(pool, id).await?;

    #[derive(sqlx::FromRow)]
    struct StakeholderStats {
        total_count: i64,
        shipped_count: i64,
        in_flight_count: i64,
        last_shipped_at: Option<String>,
    }

    let stats = sqlx::query_as::<_, StakeholderStats>(
        r#"
        SELECT
          COUNT(*)                                                  AS total_count,
          SUM(CASE WHEN state = 'shipped' THEN 1 ELSE 0 END)       AS shipped_count,
          SUM(CASE WHEN state IN ('drafting','in_review') THEN 1 ELSE 0 END) AS in_flight_count,
          MAX(CASE WHEN state = 'shipped' THEN shipped_at END)     AS last_shipped_at
        FROM deliverables
        WHERE state != 'killed'
          AND (
            stakeholder_id = ?
            OR EXISTS (
              SELECT 1
              FROM deliverable_stakeholders ds
              WHERE ds.deliverable_id = deliverables.id
                AND ds.stakeholder_id = ?
            )
          )
        "#,
    )
    .bind(id)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    let days_since = stats.last_shipped_at.as_deref().and_then(|ts| {
        chrono::DateTime::parse_from_rfc3339(ts)
            .ok()
            .map(|dt| Utc::now().signed_duration_since(dt).num_days())
    });

    Ok(StakeholderDetail {
        stakeholder,
        deliverable_count: stats.total_count,
        shipped_count: stats.shipped_count,
        in_flight_count: stats.in_flight_count,
        days_since_last_delivery: days_since,
    })
}

pub async fn list_stakeholder_details(
    pool: &SqlitePool,
) -> Result<Vec<StakeholderDetail>, String> {
    let stakeholders = list_stakeholders(pool).await?;
    let mut details = Vec::with_capacity(stakeholders.len());
    for s in stakeholders {
        let id = s.id.clone();

        #[derive(sqlx::FromRow)]
        struct Stats {
            total_count: i64,
            shipped_count: i64,
            in_flight_count: i64,
            last_shipped_at: Option<String>,
        }

        let stats = sqlx::query_as::<_, Stats>(
            r#"
            SELECT
              COUNT(*)                                                  AS total_count,
              SUM(CASE WHEN state = 'shipped' THEN 1 ELSE 0 END)       AS shipped_count,
              SUM(CASE WHEN state IN ('drafting','in_review') THEN 1 ELSE 0 END) AS in_flight_count,
              MAX(CASE WHEN state = 'shipped' THEN shipped_at END)     AS last_shipped_at
            FROM deliverables
            WHERE state != 'killed'
              AND (
                stakeholder_id = ?
                OR EXISTS (
                  SELECT 1
                  FROM deliverable_stakeholders ds
                  WHERE ds.deliverable_id = deliverables.id
                    AND ds.stakeholder_id = ?
                )
              )
            "#,
        )
        .bind(&id)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;

        let days_since = stats.last_shipped_at.as_deref().and_then(|ts| {
            chrono::DateTime::parse_from_rfc3339(ts)
                .ok()
                .map(|dt| Utc::now().signed_duration_since(dt).num_days())
        });

        details.push(StakeholderDetail {
            stakeholder: s,
            deliverable_count: stats.total_count,
            shipped_count: stats.shipped_count,
            in_flight_count: stats.in_flight_count,
            days_since_last_delivery: days_since,
        });
    }
    Ok(details)
}

async fn next_display_order(pool: &SqlitePool) -> Result<i64, String> {
    let value: Option<i64> = sqlx::query_scalar("SELECT MAX(display_order) FROM stakeholders")
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;

    Ok(value.unwrap_or(0) + 1)
}

fn normalize_email_optional(value: &str) -> Result<String, String> {
    let email = value.trim().to_lowercase();
    if email.is_empty() {
        return Ok(String::new());
    }
    if email.contains(char::is_whitespace)
        || !email.contains('@')
        || email.starts_with('@')
        || email.ends_with('@')
    {
        return Err("stakeholder email must be a valid email address".to_string());
    }
    Ok(email)
}
