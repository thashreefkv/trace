use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{CreateInitiativeInput, Initiative, InitiativeStatus, UpdateInitiativeInput},
};

use super::now_utc;

#[derive(Debug)]
pub struct CleanInitiativeInput {
    pub title: String,
    pub framing: String,
    pub status: InitiativeStatus,
    pub icon: String,
    pub icon_color: String,
}

pub async fn list_initiatives(pool: &SqlitePool) -> Result<Vec<Initiative>, String> {
    sqlx::query_as::<_, Initiative>(
        r#"
        SELECT id, title, framing, status,
               COALESCE(icon, 'target') AS icon,
               COALESCE(icon_color, '#6366f1') AS icon_color,
               created_at, updated_at
        FROM initiatives
        ORDER BY
          CASE status
            WHEN 'live' THEN 0
            WHEN 'paused' THEN 1
            WHEN 'shipped' THEN 2
            ELSE 3
          END,
          updated_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn get_initiative(pool: &SqlitePool, id: &str) -> Result<Initiative, String> {
    sqlx::query_as::<_, Initiative>(
        r#"
        SELECT id, title, framing, status,
               COALESCE(icon, 'target') AS icon,
               COALESCE(icon_color, '#6366f1') AS icon_color,
               created_at, updated_at
        FROM initiatives
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "initiative not found".to_string())
}

pub async fn create_initiative(
    pool: &SqlitePool,
    input: CreateInitiativeInput,
) -> Result<Initiative, String> {
    let input = validate_initiative_input(
        input.title,
        input.framing,
        input.status,
        input.icon,
        input.icon_color,
    )?;
    let id = Ulid::new().to_string();
    let now = now_utc();

    sqlx::query(
        r#"
        INSERT INTO initiatives (id, title, framing, status, icon, icon_color, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.title)
    .bind(&input.framing)
    .bind(input.status.as_str())
    .bind(&input.icon)
    .bind(&input.icon_color)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_initiative(pool, &id).await
}

pub async fn update_initiative(
    pool: &SqlitePool,
    id: &str,
    input: UpdateInitiativeInput,
) -> Result<Initiative, String> {
    let input = validate_initiative_input(
        input.title,
        input.framing,
        input.status,
        input.icon,
        input.icon_color,
    )?;
    let now = now_utc();

    let result = sqlx::query(
        r#"
        UPDATE initiatives
        SET title = ?, framing = ?, status = ?, icon = ?, icon_color = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&input.title)
    .bind(&input.framing)
    .bind(input.status.as_str())
    .bind(&input.icon)
    .bind(&input.icon_color)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("initiative not found".to_string());
    }

    get_initiative(pool, id).await
}

pub async fn delete_initiative(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM initiatives WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("initiative not found".to_string());
    }

    Ok(())
}

pub fn validate_initiative_input(
    title: String,
    framing: String,
    status: InitiativeStatus,
    icon: String,
    icon_color: String,
) -> Result<CleanInitiativeInput, String> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("initiative title is required".to_string());
    }
    let icon = if icon.trim().is_empty() {
        "target".to_string()
    } else {
        icon.trim().to_string()
    };
    let icon_color = if icon_color.trim().is_empty() {
        "#6366f1".to_string()
    } else {
        icon_color.trim().to_string()
    };

    Ok(CleanInitiativeInput {
        title,
        framing: framing.trim().to_string(),
        status,
        icon,
        icon_color,
    })
}
