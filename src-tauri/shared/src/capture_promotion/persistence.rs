//! Capture-promotion suggestion persistence + RL event logging.
//! Extracted from legacy.rs (Section 13).

use sqlx::SqlitePool;
use ulid::Ulid;

use crate::models::{BrainLearningEventInput, Capture};
use super::legacy::*;
use super::prompt::ParsedSuggestion;

// ---------- Persistence ----------

#[derive(sqlx::FromRow)]
pub(crate) struct SuggestionRow {
    id: String,
    capture_id: String,
    kind: String,
    target_id: Option<String>,
    target_kind: Option<String>,
    confidence: f64,
    rationale: String,
    alternatives_json: String,
    status: String,
    error_reason: Option<String>,
    applied_entity_kind: Option<String>,
    applied_entity_id: Option<String>,
    model: String,
    latency_ms: i64,
    created_at: i64,
    resolved_at: Option<i64>,
}

pub(crate) async fn hydrate(pool: &SqlitePool, row: SuggestionRow) -> Result<PromotionSuggestion, String> {
    let alternatives: Vec<PromotionAlternative> = serde_json::from_str(&row.alternatives_json)
        .unwrap_or_default();
    let target_title = match (&row.target_kind, &row.target_id) {
        (Some(kind), Some(id)) => resolve_target_title(pool, kind, id).await,
        _ => None,
    };
    Ok(PromotionSuggestion {
        id: row.id,
        capture_id: row.capture_id,
        kind: row.kind,
        target_id: row.target_id,
        target_kind: row.target_kind,
        target_title,
        confidence: row.confidence,
        rationale: row.rationale,
        alternatives,
        status: row.status,
        error_reason: row.error_reason,
        applied_entity_kind: row.applied_entity_kind,
        applied_entity_id: row.applied_entity_id,
        model: row.model,
        latency_ms: row.latency_ms,
        created_at: row.created_at,
        resolved_at: row.resolved_at,
    })
}

pub(crate) async fn resolve_target_title(pool: &SqlitePool, kind: &str, id: &str) -> Option<String> {
    match kind {
        "deliverable" => sqlx::query_scalar::<_, String>("SELECT title FROM deliverables WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten(),
        "initiative" => sqlx::query_scalar::<_, String>("SELECT title FROM initiatives WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten(),
        _ => None,
    }
}

pub(crate) async fn persist_pending(
    pool: &SqlitePool,
    capture: &Capture,
    parsed: ParsedSuggestion,
    latency_ms: i64,
) -> Result<PromotionSuggestion, String> {
    let id = format!("cps_{}", Ulid::new());
    let now = chrono::Utc::now().timestamp_millis();
    let alternatives_json = serde_json::to_string(&parsed.alternatives).unwrap_or_else(|_| "[]".to_string());

    let mut tx = pool.begin().await.map_err(|e| format!("tx begin: {e}"))?;
    sqlx::query(
        "UPDATE capture_promotion_suggestions SET status = ?, resolved_at = ?
           WHERE capture_id = ? AND status = ?",
    )
    .bind(STATUS_STALE)
    .bind(now)
    .bind(&capture.id)
    .bind(STATUS_PENDING)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("mark stale: {e}"))?;

    sqlx::query(
        r#"
        INSERT INTO capture_promotion_suggestions
          (id, capture_id, kind, target_id, target_kind, confidence, rationale,
           alternatives_json, status, model, latency_ms, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&capture.id)
    .bind(&parsed.kind)
    .bind(parsed.target_id.as_deref())
    .bind(parsed.target_kind.as_deref())
    .bind(parsed.confidence)
    .bind(&parsed.rationale)
    .bind(&alternatives_json)
    .bind(STATUS_PENDING)
    .bind(SUGGESTER_MODEL)
    .bind(latency_ms)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("insert suggestion: {e}"))?;

    tx.commit().await.map_err(|e| format!("tx commit: {e}"))?;

    let suggestion = PromotionSuggestion {
        id: id.clone(),
        capture_id: capture.id.clone(),
        kind: parsed.kind,
        target_id: parsed.target_id,
        target_kind: parsed.target_kind,
        target_title: parsed.target_title,
        confidence: parsed.confidence,
        rationale: parsed.rationale,
        alternatives: parsed.alternatives,
        status: STATUS_PENDING.to_string(),
        error_reason: None,
        applied_entity_kind: None,
        applied_entity_id: None,
        model: SUGGESTER_MODEL.to_string(),
        latency_ms,
        created_at: now,
        resolved_at: None,
    };

    record_rl_event(pool, &id, "shown", 0.0, &suggestion, None, None, None).await;
    Ok(suggestion)
}

pub(crate) async fn persist_errored(
    pool: &SqlitePool,
    capture_id: &str,
    error: &str,
    latency_ms: i64,
) -> Result<PromotionSuggestion, String> {
    let id = format!("cps_{}", Ulid::new());
    let now = chrono::Utc::now().timestamp_millis();

    let mut tx = pool.begin().await.map_err(|e| format!("tx begin: {e}"))?;
    sqlx::query(
        "UPDATE capture_promotion_suggestions SET status = ?, resolved_at = ?
           WHERE capture_id = ? AND status = ?",
    )
    .bind(STATUS_STALE)
    .bind(now)
    .bind(capture_id)
    .bind(STATUS_PENDING)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("mark stale: {e}"))?;

    sqlx::query(
        r#"
        INSERT INTO capture_promotion_suggestions
          (id, capture_id, kind, target_id, target_kind, confidence, rationale,
           alternatives_json, status, error_reason, model, latency_ms, created_at, resolved_at)
        VALUES (?, ?, '', NULL, NULL, 0.0, '', '[]', ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(capture_id)
    .bind(STATUS_ERRORED)
    .bind(error)
    .bind(SUGGESTER_MODEL)
    .bind(latency_ms)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("insert errored suggestion: {e}"))?;

    tx.commit().await.map_err(|e| format!("tx commit: {e}"))?;

    Ok(PromotionSuggestion {
        id,
        capture_id: capture_id.to_string(),
        kind: String::new(),
        target_id: None,
        target_kind: None,
        target_title: None,
        confidence: 0.0,
        rationale: String::new(),
        alternatives: Vec::new(),
        status: STATUS_ERRORED.to_string(),
        error_reason: Some(error.to_string()),
        applied_entity_kind: None,
        applied_entity_id: None,
        model: SUGGESTER_MODEL.to_string(),
        latency_ms,
        created_at: now,
        resolved_at: Some(now),
    })
}

// ---------- RL feedback ----------

#[allow(clippy::too_many_arguments)]
pub(crate) async fn record_rl_event(
    pool: &SqlitePool,
    suggestion_id: &str,
    event_type: &str,
    reward: f64,
    suggestion: &PromotionSuggestion,
    override_kind: Option<&str>,
    override_target_id: Option<&str>,
    alternative_index: Option<usize>,
) {
    let context = serde_json::json!({
        "confidence": suggestion.confidence,
        "kind": suggestion.kind,
        "target_id": suggestion.target_id,
        "target_kind": suggestion.target_kind,
        "override_kind": override_kind,
        "override_target_id": override_target_id,
        "alternative_index": alternative_index,
    });
    let input = BrainLearningEventInput {
        template: Some(TEMPLATE.to_string()),
        item_id: suggestion_id.to_string(),
        item_kind: Some(ITEM_KIND.to_string()),
        event_type: event_type.to_string(),
        reward: Some(reward),
        context: Some(context),
    };
    let _ = crate::brain::record_brain_learning_event(pool, input).await;
    // Mirror the same item-id to the node_importance bandit when a concrete
    // target was suggested. Keeps the existing per-entity learning surface fed
    // without introducing a second template path.
    if let Some(target_id) = suggestion.target_id.as_deref() {
        if reward.abs() > f64::EPSILON {
            let _ = crate::brain::record_brain_learning_event(
                pool,
                BrainLearningEventInput {
                    template: Some("node_importance".to_string()),
                    item_id: target_id.to_string(),
                    item_kind: suggestion.target_kind.clone(),
                    event_type: format!("capture_{event_type}"),
                    reward: Some(reward * 0.5),
                    context: Some(serde_json::json!({
                        "source": "capture_promotion",
                        "suggestion_id": suggestion_id,
                    })),
                },
            )
            .await;
        }
    }
    let _ = override_target_id;
}
