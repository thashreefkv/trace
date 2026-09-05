use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{CreateLabelInput, Label},
};

pub async fn list_labels(pool: &SqlitePool) -> Result<Vec<Label>, String> {
    sqlx::query_as::<_, Label>("SELECT id, name, color FROM labels ORDER BY name ASC")
        .fetch_all(pool)
        .await
        .map_err(sql_error)
}

pub async fn create_label(pool: &SqlitePool, input: CreateLabelInput) -> Result<Label, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("label name is required".to_string());
    }
    let color = if input.color.trim().is_empty() {
        "zinc".to_string()
    } else {
        input.color.trim().to_string()
    };
    let id = Ulid::new().to_string();
    sqlx::query("INSERT INTO labels (id, name, color) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&name)
        .bind(&color)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    sqlx::query_as::<_, Label>("SELECT id, name, color FROM labels WHERE id = ?")
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(sql_error)
}

pub async fn delete_label(pool: &SqlitePool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM labels WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn assign_label(
    pool: &SqlitePool,
    deliverable_id: &str,
    label_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO deliverable_labels (deliverable_id, label_id) VALUES (?, ?)",
    )
    .bind(deliverable_id)
    .bind(label_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn remove_label(
    pool: &SqlitePool,
    deliverable_id: &str,
    label_id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM deliverable_labels WHERE deliverable_id = ? AND label_id = ?")
        .bind(deliverable_id)
        .bind(label_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}
