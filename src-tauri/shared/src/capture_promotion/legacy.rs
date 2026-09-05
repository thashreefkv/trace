//! Section 4 — Capture promotion AI with RL feedback loop.
//!
//! When a capture is created, this module asks Gemini Flash to propose a
//! promotion target (task on an existing deliverable, a new deliverable, or
//! a new initiative) with structured output `{ kind, target_id?, confidence,
//! rationale, alternatives[] }`. The result is persisted to
//! `capture_promotion_suggestions`. The CaptureInbox UI exposes Apply /
//! Override / Alternatives buttons; every outcome is fed back into
//! `brain_rl_events` under `template = "capture_promotion"` so the confidence
//! signal calibrates over time.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::entity_embeddings::{cosine, load_embeddings_for};
use crate::models::{
    Capture, CaptureStatus, DeliverableFilters,
};

use super::persistence::{
    hydrate, persist_errored, persist_pending, record_rl_event, SuggestionRow,
};
use super::prompt::{
    build_suggester_body, ephemeral_capture, parse_suggester_response,
};

pub const SUGGESTER_MODEL: &str = "gemini-3-flash-preview";
pub const FEATURE_LABEL: &str = "capture_promote";
pub const TEMPLATE: &str = "capture_promotion";
pub const ITEM_KIND: &str = "capture_promotion_suggestion";

pub const MAX_DELIVERABLE_CANDIDATES: usize = 8;
pub const MAX_INITIATIVE_CANDIDATES: usize = 4;
pub const MAX_ALTERNATIVES: usize = 3;
pub const CONFIDENCE_HINT_THRESHOLD: f64 = 0.55;
pub const STATUS_PENDING: &str = "pending";
pub const STATUS_STALE: &str = "stale";
pub const STATUS_ERRORED: &str = "errored";
pub const STATUS_ACCEPTED: &str = "accepted";
pub const STATUS_ACCEPTED_ALTERNATIVE: &str = "accepted_alternative";
pub const STATUS_OVERRIDDEN: &str = "overridden";
pub const STATUS_UNDONE: &str = "undone";

// ---------- Public types ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionAlternative {
    pub kind: String,
    pub target_id: Option<String>,
    pub target_kind: Option<String>,
    pub target_title: Option<String>,
    pub confidence: f64,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromotionSuggestion {
    pub id: String,
    pub capture_id: String,
    pub kind: String,
    pub target_id: Option<String>,
    pub target_kind: Option<String>,
    pub target_title: Option<String>,
    pub confidence: f64,
    pub rationale: String,
    pub alternatives: Vec<PromotionAlternative>,
    pub status: String,
    pub error_reason: Option<String>,
    pub applied_entity_kind: Option<String>,
    pub applied_entity_id: Option<String>,
    pub model: String,
    pub latency_ms: i64,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromotionAccuracySummary {
    pub sample_count: i64,
    pub accept_rate: f64,
    pub target_match_rate: f64,
    pub errored_count_7d: i64,
    pub hint_threshold: f64,
}

#[derive(Debug, Clone)]
pub enum ApplyOutcome {
    Accepted,
    AcceptedAlternative { index: usize },
    Overridden { used_kind: String, used_target_id: Option<String> },
}

#[derive(Debug, Clone, Serialize)]
pub struct PromotionEvalOutcome {
    pub kind: String,
    pub target_id: Option<String>,
    pub confidence: f64,
    pub rationale: String,
    pub score: f64,
    pub passed: bool,
    pub kind_matches: bool,
    pub target_matches: bool,
    pub model: String,
    pub raw: serde_json::Value,
}

// ---------- Entry points ----------

pub async fn suggest_capture_promotion(
    pool: &SqlitePool,
    api_key: &str,
    capture_id: &str,
) -> Result<PromotionSuggestion, String> {
    let capture = crate::repo::get_capture(pool, capture_id).await?;
    if capture.status != CaptureStatus::Inbox.as_str()
        && capture.status != CaptureStatus::Suggested.as_str()
    {
        return Err("capture is no longer in inbox/suggested state".to_string());
    }

    let started = std::time::Instant::now();
    let candidates = build_candidates(pool, &capture).await;

    // Sanitize the capture body before it lands in the prompt; log any
    // noteworthy events (flags or truncation) so the audit panel reflects
    // adversarial captures.
    let sanitized = crate::prompt_safety::sanitize_plain_text(
        &capture.body,
        crate::prompt_safety::CAPTURE_CAP,
    );
    crate::prompt_safety::log_if_noteworthy(
        Some(pool),
        "capture",
        "capture",
        Some(&capture.id),
        None,
        &capture.body,
        &sanitized,
    )
    .await;
    let body = build_suggester_body(&capture, &candidates, &sanitized);
    let raw = match crate::gemini::post_gemini_external(
        Some(pool),
        FEATURE_LABEL,
        SUGGESTER_MODEL,
        api_key,
        &body,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            return persist_errored(pool, capture_id, &error, started.elapsed().as_millis() as i64)
                .await;
        }
    };

    let latency_ms = started.elapsed().as_millis() as i64;
    let parsed = match parse_suggester_response(&raw, &candidates) {
        Ok(parsed) => parsed,
        Err(error) => return persist_errored(pool, capture_id, &error, latency_ms).await,
    };

    persist_pending(pool, &capture, parsed, latency_ms).await
}

pub async fn get_current_suggestion(
    pool: &SqlitePool,
    capture_id: &str,
) -> Result<Option<PromotionSuggestion>, String> {
    let row = sqlx::query_as::<_, SuggestionRow>(
        r#"
        SELECT id, capture_id, kind, target_id, target_kind, confidence,
               rationale, alternatives_json, status, error_reason,
               applied_entity_kind, applied_entity_id, model, latency_ms,
               created_at, resolved_at
        FROM capture_promotion_suggestions
        WHERE capture_id = ?
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(capture_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("get suggestion: {e}"))?;
    match row {
        Some(row) => Ok(Some(hydrate(pool, row).await?)),
        None => Ok(None),
    }
}

pub async fn get_suggestion_by_id(
    pool: &SqlitePool,
    suggestion_id: &str,
) -> Result<PromotionSuggestion, String> {
    let row = sqlx::query_as::<_, SuggestionRow>(
        r#"
        SELECT id, capture_id, kind, target_id, target_kind, confidence,
               rationale, alternatives_json, status, error_reason,
               applied_entity_kind, applied_entity_id, model, latency_ms,
               created_at, resolved_at
        FROM capture_promotion_suggestions
        WHERE id = ?
        "#,
    )
    .bind(suggestion_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("get suggestion by id: {e}"))?
    .ok_or_else(|| "suggestion not found".to_string())?;
    hydrate(pool, row).await
}

pub async fn record_apply_outcome(
    pool: &SqlitePool,
    suggestion_id: &str,
    outcome: ApplyOutcome,
    applied_entity_kind: &str,
    applied_entity_id: &str,
) -> Result<(), String> {
    let (status, override_kind, override_target_id, alternative_index) = match &outcome {
        ApplyOutcome::Accepted => (STATUS_ACCEPTED, None, None, None),
        ApplyOutcome::AcceptedAlternative { index } => {
            (STATUS_ACCEPTED_ALTERNATIVE, None, None, Some(*index))
        }
        ApplyOutcome::Overridden { used_kind, used_target_id } => (
            STATUS_OVERRIDDEN,
            Some(used_kind.clone()),
            used_target_id.clone(),
            None,
        ),
    };

    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        r#"
        UPDATE capture_promotion_suggestions
        SET status = ?, applied_entity_kind = ?, applied_entity_id = ?, resolved_at = ?
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(applied_entity_kind)
    .bind(applied_entity_id)
    .bind(now)
    .bind(suggestion_id)
    .execute(pool)
    .await
    .map_err(|e| format!("record apply outcome: {e}"))?;

    let suggestion = get_suggestion_by_id(pool, suggestion_id).await?;
    record_rl_event(
        pool,
        suggestion_id,
        match outcome {
            ApplyOutcome::Accepted => "accepted",
            ApplyOutcome::AcceptedAlternative { .. } => "accepted_alternative",
            ApplyOutcome::Overridden { .. } => "overridden",
        },
        match status {
            STATUS_ACCEPTED => 1.0,
            STATUS_ACCEPTED_ALTERNATIVE => 0.5,
            STATUS_OVERRIDDEN => -0.5,
            _ => 0.0,
        },
        &suggestion,
        override_kind.as_deref(),
        override_target_id.as_deref(),
        alternative_index,
    )
    .await;

    Ok(())
}

pub async fn record_shown(pool: &SqlitePool, suggestion_id: &str) -> Result<(), String> {
    let suggestion = get_suggestion_by_id(pool, suggestion_id).await?;
    record_rl_event(pool, suggestion_id, "shown", 0.0, &suggestion, None, None, None).await;
    Ok(())
}

pub struct UndoTarget {
    pub capture_id: String,
    pub applied_entity_kind: String,
    pub applied_entity_id: String,
}

pub async fn record_undo(
    pool: &SqlitePool,
    suggestion_id: &str,
) -> Result<UndoTarget, String> {
    let suggestion = get_suggestion_by_id(pool, suggestion_id).await?;
    let applied_kind = suggestion
        .applied_entity_kind
        .clone()
        .ok_or_else(|| "suggestion was never applied".to_string())?;
    let applied_id = suggestion
        .applied_entity_id
        .clone()
        .ok_or_else(|| "suggestion was never applied".to_string())?;

    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        r#"
        UPDATE capture_promotion_suggestions
        SET status = ?, resolved_at = ?
        WHERE id = ?
        "#,
    )
    .bind(STATUS_UNDONE)
    .bind(now)
    .bind(suggestion_id)
    .execute(pool)
    .await
    .map_err(|e| format!("record undo: {e}"))?;

    record_rl_event(pool, suggestion_id, "undo", -1.0, &suggestion, None, None, None).await;

    Ok(UndoTarget {
        capture_id: suggestion.capture_id,
        applied_entity_kind: applied_kind,
        applied_entity_id: applied_id,
    })
}

pub async fn promotion_accuracy_summary(
    pool: &SqlitePool,
) -> Result<PromotionAccuracySummary, String> {
    let stats = sqlx::query_as::<_, (i64, Option<f64>, Option<f64>)>(
        r#"
        WITH resolved AS (
          SELECT status, target_id
          FROM capture_promotion_suggestions
          WHERE status IN ('accepted','accepted_alternative','overridden','undone')
          ORDER BY resolved_at DESC
          LIMIT 50
        )
        SELECT
          COUNT(*) AS sample_count,
          ROUND(1.0 * SUM(CASE WHEN status IN ('accepted','accepted_alternative') THEN 1 ELSE 0 END)
                 / NULLIF(COUNT(*), 0), 3) AS accept_rate,
          ROUND(1.0 * SUM(CASE WHEN status = 'accepted' AND target_id IS NOT NULL THEN 1 ELSE 0 END)
                 / NULLIF(SUM(CASE WHEN status IN ('accepted','accepted_alternative') THEN 1 ELSE 0 END), 0), 3) AS target_match_rate
        FROM resolved
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("accuracy summary: {e}"))?;

    let cutoff_7d = chrono::Utc::now().timestamp_millis() - 7 * 24 * 60 * 60 * 1000;
    let errored: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM capture_promotion_suggestions
          WHERE status = 'errored' AND created_at >= ?",
    )
    .bind(cutoff_7d)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("errored count: {e}"))?;

    Ok(PromotionAccuracySummary {
        sample_count: stats.0,
        accept_rate: stats.1.unwrap_or(0.0),
        target_match_rate: stats.2.unwrap_or(0.0),
        errored_count_7d: errored.0,
        hint_threshold: CONFIDENCE_HINT_THRESHOLD,
    })
}

pub async fn evaluate_for_fixture(
    pool: &SqlitePool,
    api_key: &str,
    capture_text: &str,
    expected_kind: &str,
    expected_target_id: Option<&str>,
) -> Result<PromotionEvalOutcome, String> {
    // Hermetic: don't persist a suggestion row, but route through
    // `post_gemini_external` so the call lands in `gemini_usage_log` and the
    // budget gate applies (matching the Ask judge's accounting).
    let ephemeral = ephemeral_capture(capture_text);
    let candidates = build_candidates(pool, &ephemeral).await;
    let sanitized = crate::prompt_safety::sanitize_plain_text(
        &ephemeral.body,
        crate::prompt_safety::CAPTURE_CAP,
    );
    let body = build_suggester_body(&ephemeral, &candidates, &sanitized);
    let raw = crate::gemini::post_gemini_external(
        Some(pool),
        FEATURE_LABEL,
        SUGGESTER_MODEL,
        api_key,
        &body,
    )
    .await?;
    let parsed = parse_suggester_response(&raw, &candidates)?;

    let kind_matches = parsed.kind.eq_ignore_ascii_case(expected_kind);
    let target_matches = match (expected_target_id, parsed.target_id.as_deref()) {
        (Some(expected), Some(got)) => expected == got,
        (None, _) => true,
        _ => false,
    };
    let score = if expected_target_id.is_some() {
        let mut s = 0.0;
        if kind_matches {
            s += 0.5;
        }
        if target_matches && kind_matches {
            s += 0.5;
        }
        s
    } else if kind_matches {
        1.0
    } else {
        0.0
    };
    let passed = score >= 0.5 && kind_matches;

    Ok(PromotionEvalOutcome {
        kind: parsed.kind,
        target_id: parsed.target_id,
        confidence: parsed.confidence,
        rationale: parsed.rationale,
        score,
        passed,
        kind_matches,
        target_matches,
        model: SUGGESTER_MODEL.to_string(),
        raw,
    })
}

// ---------- Candidate building ----------

#[derive(Debug, Clone)]
pub(crate) struct Candidate {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) similarity: f32,
}

async fn build_candidates(pool: &SqlitePool, capture: &Capture) -> Vec<Candidate> {
    let deliverables = crate::repo::list_deliverables(
        pool,
        DeliverableFilters {
            initiative_id: None,
            stakeholder_id: None,
            deliverable_type: None,
            state: None,
            state_in: None,
            priority: None,
        },
    )
    .await
    .unwrap_or_default();
    let initiatives = crate::repo::list_initiatives(pool).await.unwrap_or_default();

    let mut deliv: Vec<Candidate> = deliverables
        .into_iter()
        .filter(|d| d.state != "shipped" && d.state != "killed")
        .map(|d| Candidate {
            id: d.id,
            kind: "deliverable".to_string(),
            title: d.title,
            summary: d.claim,
            similarity: 0.0,
        })
        .collect();
    let mut init: Vec<Candidate> = initiatives
        .into_iter()
        .filter(|i| i.status == "live")
        .map(|i| Candidate {
            id: i.id,
            kind: "initiative".to_string(),
            title: i.title,
            summary: i.framing,
            similarity: 0.0,
        })
        .collect();

    // Try cosine ranking against the capture's stored embedding. If anything
    // is missing (capture not yet embedded, candidate not yet embedded) the
    // candidate simply scores 0.0 and falls back to recency order.
    let mut keys: Vec<(String, String)> = Vec::with_capacity(deliv.len() + init.len() + 1);
    keys.push(("capture".to_string(), capture.id.clone()));
    for c in deliv.iter().chain(init.iter()) {
        keys.push((c.kind.clone(), c.id.clone()));
    }
    if let Ok(map) = load_embeddings_for(pool, &keys).await {
        if let Some((capture_vec, capture_norm)) = map.get(&("capture".to_string(), capture.id.clone())) {
            for c in deliv.iter_mut().chain(init.iter_mut()) {
                if let Some((vec, norm)) = map.get(&(c.kind.clone(), c.id.clone())) {
                    c.similarity = cosine(capture_vec, *capture_norm, vec, *norm);
                }
            }
        }
    }

    deliv.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    init.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    deliv.truncate(MAX_DELIVERABLE_CANDIDATES);
    init.truncate(MAX_INITIATIVE_CANDIDATES);

    let mut all = deliv;
    all.extend(init);
    all
}

