//! Per-thread classification overrides. Extracted from legacy.rs (13-std2).

use sqlx::SqlitePool;

use super::legacy::*;

pub async fn get_override(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Option<UserClassification>, String> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            i64,
            i64,
        ),
    >(
        "SELECT thread_id, category, priority, intent, action_required, thread_state,
                work_relevance, attention_state, message_type, note, source, rule_id,
                created_at, updated_at
           FROM gmail_user_classifications WHERE thread_id = ?",
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("read override: {e}"))?;

    Ok(row.map(
        |(
            thread_id,
            category,
            priority,
            intent,
            action_required,
            thread_state,
            work_relevance,
            attention_state,
            message_type,
            note,
            source,
            rule_id,
            created_at,
            updated_at,
        )| UserClassification {
            thread_id,
            category,
            priority,
            intent,
            action_required: action_required.map(|v| v != 0),
            thread_state,
            work_relevance,
            attention_state,
            message_type,
            note,
            source,
            rule_id,
            created_at,
            updated_at,
        },
    ))
}

pub async fn set_override(
    pool: &SqlitePool,
    input: SetOverrideInput,
) -> Result<UserClassification, String> {
    let ts = chrono::Utc::now().timestamp_millis();
    let action_int = input.action_required.map(|v| if v { 1_i64 } else { 0_i64 });
    sqlx::query(
        "INSERT INTO gmail_user_classifications
           (thread_id, category, priority, intent, action_required, thread_state,
            work_relevance, attention_state, message_type, note, source, rule_id,
            created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'manual', NULL, ?, ?)
         ON CONFLICT(thread_id) DO UPDATE SET
           category = excluded.category,
           priority = excluded.priority,
           intent = excluded.intent,
           action_required = excluded.action_required,
           thread_state = excluded.thread_state,
           work_relevance = excluded.work_relevance,
           attention_state = excluded.attention_state,
           message_type = excluded.message_type,
           note = excluded.note,
           source = 'manual',
           rule_id = NULL,
           updated_at = excluded.updated_at",
    )
    .bind(&input.thread_id)
    .bind(&input.category)
    .bind(&input.priority)
    .bind(&input.intent)
    .bind(action_int)
    .bind(&input.thread_state)
    .bind(&input.work_relevance)
    .bind(&input.attention_state)
    .bind(&input.message_type)
    .bind(&input.note)
    .bind(ts)
    .bind(ts)
    .execute(pool)
    .await
    .map_err(|e| format!("set override: {e}"))?;

    // Record RL feedback so future classifications can calibrate against this correction.
    let context = serde_json::json!({
        "category": input.category,
        "priority": input.priority,
        "work_relevance": input.work_relevance,
        "attention_state": input.attention_state,
        "message_type": input.message_type,
        "note": input.note,
    });
    let _ = sqlx::query(
        "INSERT INTO brain_rl_events
           (id, template, item_id, item_kind, event_type, reward, context_json, created_at)
         VALUES (?, 'email_classification', ?, 'gmail_thread', 'override', 1.0, ?, ?)",
    )
    .bind(ulid::Ulid::new().to_string())
    .bind(&input.thread_id)
    .bind(context.to_string())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await;
    let _ = crate::gmail::record_work_mail_agent_event(
        pool,
        Some(&input.thread_id),
        "override",
        "user",
        "Updated Trace understanding for thread.",
        serde_json::json!(["Thread correction overrides automated classification."]),
        serde_json::json!({
            "category": input.category,
            "priority": input.priority,
            "work_relevance": input.work_relevance,
            "attention_state": input.attention_state,
            "message_type": input.message_type
        }),
        None,
    )
    .await;

    get_override(pool, &input.thread_id)
        .await?
        .ok_or_else(|| "override not found after upsert".to_string())
}

pub async fn clear_override(pool: &SqlitePool, thread_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM gmail_user_classifications WHERE thread_id = ?")
        .bind(thread_id)
        .execute(pool)
        .await
        .map_err(|e| format!("clear override: {e}"))?;
    Ok(())
}

