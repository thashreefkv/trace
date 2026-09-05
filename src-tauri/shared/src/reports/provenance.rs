use serde_json::Value;
use sqlx::SqlitePool;

use crate::db::sql_error;

pub(super) async fn record_event(
    pool: &SqlitePool,
    entity_id: &str,
    event_type: &str,
    payload: Value,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO entity_events
           (id, entity_kind, entity_id, event_kind, actor, detail_json, created_at)
           VALUES (?, 'report_run', ?, ?, 'user_or_system', ?, ?)"#,
    )
    .bind(format!("event:{}", ulid::Ulid::new()))
    .bind(entity_id)
    .bind(event_type)
    .bind(payload.to_string())
    .bind(crate::repo::now_utc())
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}
