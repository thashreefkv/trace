use sqlx::SqlitePool;

use crate::{
    db::sql_error,
    models::{Deliverable, DeliverableState},
};

use super::super::{get_deliverable, now_utc, parse_deliverable_state};

pub async fn update_deliverable_state(
    pool: &SqlitePool,
    id: &str,
    state: DeliverableState,
) -> Result<Deliverable, String> {
    update_deliverable_state_with_friction(pool, id, state, None).await
}

pub async fn update_deliverable_state_with_friction(
    pool: &SqlitePool,
    id: &str,
    state: DeliverableState,
    friction_note: Option<String>,
) -> Result<Deliverable, String> {
    let current = get_deliverable(pool, id).await?;
    let current_shipped_at = current.shipped_at.clone();
    let from_state = current.state.clone();
    let now = now_utc();
    let shipped_at = shipped_at_for_state(state, current_shipped_at, &now);

    let mut tx = pool.begin().await.map_err(sql_error)?;

    let result = sqlx::query(
        r#"
        UPDATE deliverables
        SET state = ?, shipped_at = ?, updated_at = ?, state_changed_at = ?
        WHERE id = ?
        "#,
    )
    .bind(state.as_str())
    .bind(&shipped_at)
    .bind(&now)
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("deliverable not found".to_string());
    }

    // Log state history
    let history_id = ulid::Ulid::new().to_string();
    sqlx::query(
        r#"INSERT INTO deliverable_state_history
           (id, deliverable_id, from_state, to_state, friction_note, moved_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&history_id)
    .bind(id)
    .bind(&from_state)
    .bind(state.as_str())
    .bind(&friction_note)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    tx.commit().await.map_err(sql_error)?;
    get_deliverable(pool, id).await
}
pub fn shipped_at_for_state(
    state: DeliverableState,
    current_shipped_at: Option<String>,
    now: &str,
) -> Option<String> {
    if state == DeliverableState::Shipped {
        current_shipped_at.or_else(|| Some(now.to_string()))
    } else {
        None
    }
}
// ── State move with friction note (for board drag) ────────────────────────────

pub async fn update_deliverable_state_friction(
    pool: &SqlitePool,
    id: &str,
    state_str: &str,
    friction_note: Option<String>,
) -> Result<Deliverable, String> {
    let state = parse_deliverable_state(state_str)?;
    update_deliverable_state_with_friction(pool, id, state, friction_note).await
}
