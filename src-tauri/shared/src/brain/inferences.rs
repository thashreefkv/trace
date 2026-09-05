//! Brain inference layer: structural predicates derived from the projection
//! plus their review / supersession lifecycle.
//!
//! Three concerns live here:
//!
//! 1. **Candidate generation** — `refresh_brain_inferences` scans the
//!    projection-backed signals, calls `upsert_brain_inference` to write
//!    pending rows (auto-accepting when confidence ≥ the per-template
//!    threshold from `super::state::InferenceThresholdCache`), and
//!    `add_inferences` materializes accepted rows back onto the graph.
//! 2. **Threshold learning** — `recompute_inference_thresholds*` walks
//!    feedback events in a sliding window and moves each template's
//!    threshold toward the value that hits `INFERENCE_TARGET_PRECISION`.
//! 3. **Review / supersession** — `list_brain_inferences`, `review_inference`,
//!    `list_inference_supersessions`, `revert_inference_supersession` drive
//!    the Brain UI's review queue. `auto_supersede_on_accept` enforces
//!    `PREDICATE_OPPOSITES` (accepting "blocked_by" rejects the matching
//!    "unblocks" sibling, etc.). `apply_feedback_to_inferences` is the
//!    shared writer used by both Ask feedback and inference-review feedback.

use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::models::{
    BrainInferenceFilter, BrainInferenceListResult, BrainInferenceRecord, BrainInferenceRow,
    BrainLearningEventInput, ReviewInferenceResult, SupersessionRecord,
};

use super::legacy::{
    entity, graph_node_id, json_from_string, now_utc, relation, source_node_id, sql_error,
    stable_suffix, truncate,
};
use super::projection::BrainProjection;
use super::rl::record_brain_learning_event;
use super::state::InferenceThresholdCache;

pub(super) async fn apply_feedback_to_inferences(
    pool: &SqlitePool,
    feedback: &str,
    corrected: &serde_json::Value,
    now: &str,
) -> Result<(), String> {
    if let Some(inference_id) = corrected
        .get("inference_id")
        .and_then(|value| value.as_str())
    {
        let status = if feedback == "useful" {
            Some("accepted")
        } else if feedback == "wrong" {
            Some("rejected")
        } else {
            None
        };
        if let Some(status) = status {
            sqlx::query(
                "UPDATE brain_inferences SET status = ?, reviewed_at = ?, updated_at = ? WHERE id = ?",
            )
            .bind(status)
            .bind(now)
            .bind(now)
            .bind(inference_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
            if status == "accepted" {
                auto_supersede_on_accept(pool, inference_id, now).await?;
            }
        }
    }

    for key_status in [
        ("accepted_inference_ids", "accepted"),
        ("rejected_inference_ids", "rejected"),
    ] {
        if let Some(ids) = corrected
            .get(key_status.0)
            .and_then(|value| value.as_array())
        {
            for id in ids.iter().filter_map(|value| value.as_str()) {
                sqlx::query(
                    "UPDATE brain_inferences SET status = ?, reviewed_at = ?, updated_at = ? WHERE id = ?",
                )
                .bind(key_status.1)
                .bind(now)
                .bind(now)
                .bind(id)
                .execute(pool)
                .await
                .map_err(sql_error)?;
                if key_status.1 == "accepted" {
                    auto_supersede_on_accept(pool, id, now).await?;
                }
            }
        }
    }

    if let Some(relationship) = corrected
        .get("corrected_relationship")
        .and_then(|value| value.as_object())
    {
        let required = |key: &str| {
            relationship
                .get(key)
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .ok_or_else(|| format!("corrected_relationship missing {key}"))
        };
        let source_kind = required("source_kind")?;
        let source_id = required("source_id")?;
        let relation_kind = required("relation_kind")?;
        let target_kind = required("target_kind")?;
        let target_id = required("target_id")?;
        let confidence = relationship
            .get("confidence")
            .and_then(|value| value.as_f64())
            .unwrap_or(0.95)
            .clamp(0.0, 1.0);
        let rationale = relationship
            .get("rationale")
            .and_then(|value| value.as_str())
            .unwrap_or("User-corrected relationship.")
            .to_string();
        let evidence = relationship
            .get("evidence")
            .cloned()
            .unwrap_or_else(|| json!({ "source": "brain_answer_feedback" }));
        sqlx::query(
            r#"
            INSERT INTO brain_inferences (
              id, source_kind, source_id, relation_kind, target_kind, target_id,
              confidence, rationale, evidence_json, status, generated_by,
              created_at, updated_at, reviewed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'accepted', 'feedback', ?, ?, ?)
            ON CONFLICT(source_kind, source_id, relation_kind, target_kind, target_id, generated_by)
            DO UPDATE SET
              confidence = excluded.confidence,
              rationale = excluded.rationale,
              evidence_json = excluded.evidence_json,
              status = 'accepted',
              updated_at = excluded.updated_at,
              reviewed_at = excluded.reviewed_at
            "#,
        )
        .bind(format!("inf_{}", Ulid::new()))
        .bind(&source_kind)
        .bind(&source_id)
        .bind(&relation_kind)
        .bind(&target_kind)
        .bind(&target_id)
        .bind(confidence)
        .bind(rationale)
        .bind(serde_json::to_string(&evidence).unwrap_or_else(|_| "{}".to_string()))
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(sql_error)?;

        // Find the resulting row (whether newly inserted or upserted) and
        // run the auto-supersede pass against its actual id.
        let resolved: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT id FROM brain_inferences
            WHERE source_kind = ? AND source_id = ?
              AND relation_kind = ? AND target_kind = ? AND target_id = ?
              AND generated_by = 'feedback'
            "#,
        )
        .bind(&source_kind)
        .bind(&source_id)
        .bind(&relation_kind)
        .bind(&target_kind)
        .bind(&target_id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;
        if let Some((id,)) = resolved {
            auto_supersede_on_accept(pool, &id, now).await?;
        }
    }

    Ok(())
}
#[derive(Debug)]
struct BrainInferenceCandidate {
    source_kind: String,
    source_id: String,
    relation_kind: String,
    target_kind: String,
    target_id: String,
    confidence: f64,
    rationale: String,
    evidence: serde_json::Value,
    /// The RL template that produced this candidate. Used by Section 6.2's
    /// inference queue to filter and to JOIN against `inference_thresholds`.
    template: Option<String>,
}

pub(super) async fn refresh_brain_inferences(pool: &SqlitePool) -> Result<(), String> {
    // Pull the learned thresholds once so the per-row inference inserts
    // don't each issue their own SELECT.
    let threshold_cache = InferenceThresholdCache::load(pool).await;
    let meeting_exact = threshold_cache.get("meeting_action_exact", 0.86);
    let meeting_fuzzy = threshold_cache.get("meeting_action_fuzzy", 0.72);
    let email_mention = threshold_cache.get("email_thread_mention", 0.64);
    let blocker_email = threshold_cache.get("blocker_email_match", 0.88);
    let blocker_fuzzy = threshold_cache.get("blocker_fuzzy", 0.74);
    let meeting_candidates = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        r#"
        SELECT ma.id, COALESCE(ma.target_title, '') AS target_title, ma.body,
               d.id, d.title, ma.created_at
        FROM meeting_actions ma
        JOIN deliverables d
          ON LENGTH(TRIM(d.title)) >= 6
         AND (
              LOWER(COALESCE(ma.target_title, '')) = LOWER(d.title)
              OR LOWER(ma.body) LIKE '%' || LOWER(d.title) || '%'
         )
        WHERE ma.target_id IS NULL
        LIMIT 80
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (action_id, target_title, body, deliverable_id, deliverable_title, created_at) in
        meeting_candidates
    {
        upsert_brain_inference(
            pool,
            BrainInferenceCandidate {
                source_kind: "meeting_action".to_string(),
                source_id: action_id,
                relation_kind: "GENERATED".to_string(),
                target_kind: "deliverable".to_string(),
                target_id: deliverable_id,
                confidence: if target_title.eq_ignore_ascii_case(&deliverable_title) {
                    meeting_exact
                } else {
                    meeting_fuzzy
                },
                rationale: format!(
                    "Meeting action text appears to belong to deliverable '{deliverable_title}'."
                ),
                evidence: json!({
                    "source": "meeting_actions.target_title/body + deliverables.title",
                    "target_title": target_title,
                    "body": truncate(&body, 300),
                    "created_at": created_at,
                }),
                template: Some(
                    if target_title.eq_ignore_ascii_case(&deliverable_title) {
                        "meeting_action_exact"
                    } else {
                        "meeting_action_fuzzy"
                    }
                    .to_string(),
                ),
            },
        )
        .await?;
    }

    let email_candidates = sqlx::query_as::<_, (String, String, String, String, String)>(
        r#"
        SELECT gt.thread_id, gt.subject, gt.snippet, d.id, d.title
        FROM gmail_threads gt
        JOIN deliverables d
          ON LENGTH(TRIM(d.title)) >= 8
         AND (
              LOWER(gt.subject) LIKE '%' || LOWER(d.title) || '%'
              OR LOWER(gt.snippet) LIKE '%' || LOWER(d.title) || '%'
              OR LOWER(COALESCE(gt.summary, '')) LIKE '%' || LOWER(d.title) || '%'
         )
        LEFT JOIN gmail_thread_deliverables existing
          ON existing.thread_id = gt.thread_id AND existing.deliverable_id = d.id
        WHERE existing.thread_id IS NULL
        LIMIT 120
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (thread_id, subject, snippet, deliverable_id, deliverable_title) in email_candidates {
        upsert_brain_inference(
            pool,
            BrainInferenceCandidate {
                source_kind: "email_thread".to_string(),
                source_id: thread_id,
                relation_kind: "RELATED_TO".to_string(),
                target_kind: "deliverable".to_string(),
                target_id: deliverable_id,
                confidence: email_mention,
                rationale: format!(
                    "Email subject or snippet mentions deliverable '{deliverable_title}'."
                ),
                evidence: json!({
                    "source": "gmail_threads subject/snippet/summary + deliverables.title",
                    "subject": subject,
                    "snippet": truncate(&snippet, 300),
                }),
                template: Some("email_thread_mention".to_string()),
            },
        )
        .await?;
    }

    let blocked = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT id, title, blocker_reason
        FROM deliverables
        WHERE blocker_reason IS NOT NULL AND TRIM(blocker_reason) != ''
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    let stakeholders = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, COALESCE(email, '') AS email FROM stakeholders",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (deliverable_id, title, blocker) in blocked {
        let haystack = blocker.to_ascii_lowercase();
        let blocker_id = format!("{}:{}", deliverable_id, stable_suffix(&blocker));
        for (stakeholder_id, name, email) in &stakeholders {
            let name_match = !name.trim().is_empty()
                && name.len() >= 3
                && haystack.contains(&name.to_ascii_lowercase());
            let email_match =
                !email.trim().is_empty() && haystack.contains(&email.to_ascii_lowercase());
            if !name_match && !email_match {
                continue;
            }
            upsert_brain_inference(
                pool,
                BrainInferenceCandidate {
                    source_kind: "blocker".to_string(),
                    source_id: blocker_id.clone(),
                    relation_kind: "WAITING_ON".to_string(),
                    target_kind: "stakeholder".to_string(),
                    target_id: stakeholder_id.clone(),
                    confidence: if email_match { blocker_email } else { blocker_fuzzy },
                    rationale: format!(
                        "Blocker on '{title}' appears to be waiting on stakeholder '{name}'."
                    ),
                    evidence: json!({
                        "source": "deliverables.blocker_reason + stakeholders name/email",
                        "blocker_reason": truncate(&blocker, 500),
                        "stakeholder_name": name,
                        "stakeholder_email": email,
                    }),
                    template: Some(
                        if email_match {
                            "blocker_email_match"
                        } else {
                            "blocker_fuzzy"
                        }
                        .to_string(),
                    ),
                },
            )
            .await?;
        }
    }

    Ok(())
}

async fn upsert_brain_inference(
    pool: &SqlitePool,
    candidate: BrainInferenceCandidate,
) -> Result<(), String> {
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT INTO brain_inferences (
          id, source_kind, source_id, relation_kind, target_kind, target_id,
          confidence, rationale, evidence_json, status, generated_by,
          template, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', 'system', ?, ?, ?)
        ON CONFLICT(source_kind, source_id, relation_kind, target_kind, target_id, generated_by)
        DO UPDATE SET
          confidence = CASE
            WHEN brain_inferences.status = 'pending' THEN excluded.confidence
            ELSE brain_inferences.confidence
          END,
          rationale = CASE
            WHEN brain_inferences.status = 'pending' THEN excluded.rationale
            ELSE brain_inferences.rationale
          END,
          evidence_json = CASE
            WHEN brain_inferences.status = 'pending' THEN excluded.evidence_json
            ELSE brain_inferences.evidence_json
          END,
          template = COALESCE(excluded.template, brain_inferences.template),
          updated_at = excluded.updated_at
        "#,
    )
    .bind(format!("inf_{}", Ulid::new()))
    .bind(candidate.source_kind)
    .bind(candidate.source_id)
    .bind(candidate.relation_kind)
    .bind(candidate.target_kind)
    .bind(candidate.target_id)
    .bind(candidate.confidence.clamp(0.0, 1.0))
    .bind(candidate.rationale)
    .bind(serde_json::to_string(&candidate.evidence).unwrap_or_else(|_| "{}".to_string()))
    .bind(candidate.template)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub(super) async fn add_inferences(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let rows = sqlx::query_as::<_, BrainInferenceRecord>(
        r#"
        SELECT id, source_kind, source_id, relation_kind, target_kind, target_id,
               confidence, rationale, evidence_json, status, generated_by,
               created_at, updated_at, reviewed_at,
               template, superseded_by, supersede_reason
        FROM brain_inferences
        WHERE status != 'rejected'
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in rows {
        let Some(source) = source_node_id(&row.source_kind, &row.source_id) else {
            continue;
        };
        let Some(target) = source_node_id(&row.target_kind, &row.target_id) else {
            continue;
        };
        let inference_node = graph_node_id("inference", &row.id);
        projection.push_node(entity(
            "inference",
            "brain_inferences",
            &row.id,
            &format!(
                "{} {} {}",
                row.source_kind, row.relation_kind, row.target_kind
            ),
            &row.rationale,
            &row.status,
            None,
            &row.created_at,
            &row.updated_at,
            row.confidence.clamp(0.2, 1.0),
            json!({
                "source_kind": &row.source_kind,
                "source_id": &row.source_id,
                "relation_kind": &row.relation_kind,
                "target_kind": &row.target_kind,
                "target_id": &row.target_id,
                "confidence": row.confidence,
                "evidence": json_from_string(&row.evidence_json),
                "generated_by": &row.generated_by,
                "reviewed_at": &row.reviewed_at,
            }),
        ));
        projection.push_edge(relation(
            &source,
            &inference_node,
            "SUGGESTS_RELATION",
            "suggests relation",
            row.confidence,
            json!({ "source": "brain_inferences" }),
            &row.created_at,
            &row.updated_at,
            json!({ "status": &row.status }),
        ));
        projection.push_edge(relation(
            &inference_node,
            &target,
            "TARGETS",
            "targets",
            row.confidence,
            json!({ "source": "brain_inferences" }),
            &row.created_at,
            &row.updated_at,
            json!({}),
        ));

        let trusted =
            row.status == "accepted" || (row.reviewed_at.is_some() && row.confidence >= 0.9);
        if trusted {
            projection.push_edge(relation(
                &source,
                &target,
                &row.relation_kind.to_ascii_uppercase(),
                "inferred",
                row.confidence,
                json!({
                    "source": "brain_inferences",
                    "inference_id": row.id,
                    "rationale": row.rationale,
                    "evidence": json_from_string(&row.evidence_json),
                }),
                &row.created_at,
                &row.updated_at,
                json!({ "inferred": true }),
            ));
        }
    }

    Ok(())
}
/// Predicate opposites — used by the auto-supersede logic in
/// `apply_feedback_to_inferences`. When an inference with one of these
/// relation_kinds is accepted, any accepted sibling with the opposite
/// relation_kind on the same `(source, target)` is auto-rejected and
/// linked via `superseded_by`.
///
/// Pairs are bidirectional: `opposite_predicate` checks both columns so
/// adding `("A", "B")` is sufficient — `B` resolves to `A` too.
static PREDICATE_OPPOSITES: &[(&str, &str)] = &[
    ("WAITING_ON", "NO_LONGER_WAITING_ON"),
    ("BLOCKED_BY", "UNBLOCKED_BY"),
    ("DEPENDS_ON", "INDEPENDENT_OF"),
];

fn opposite_predicate(relation_kind: &str) -> Option<&'static str> {
    for (left, right) in PREDICATE_OPPOSITES {
        if relation_kind == *left {
            return Some(*right);
        }
        if relation_kind == *right {
            return Some(*left);
        }
    }
    None
}

/// Auto-supersede pass triggered when an inference is accepted. Two rules:
/// 1. Reject accepted siblings with the explicit opposite predicate.
/// 2. Reject pending siblings with any other relation_kind (dominance —
///    user's pick now wins over speculative alternatives).
///
/// Each superseded inference gets `superseded_by = winner_id`,
/// `supersede_reason` set, and a `superseded` event in `brain_rl_events`
/// so the learning loop stays closed.
async fn auto_supersede_on_accept(
    pool: &SqlitePool,
    accepted_inference_id: &str,
    now: &str,
) -> Result<(), String> {
    let row: Option<(String, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT source_kind, source_id, relation_kind, target_kind, target_id
        FROM brain_inferences
        WHERE id = ?
        "#,
    )
    .bind(accepted_inference_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;
    let Some((source_kind, source_id, relation_kind, target_kind, target_id)) = row else {
        return Ok(());
    };

    let mut superseded_ids: Vec<String> = Vec::new();

    // Rule 1: explicit opposite predicate that's currently accepted.
    if let Some(opposite) = opposite_predicate(&relation_kind) {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT id FROM brain_inferences
            WHERE source_kind = ? AND source_id = ?
              AND target_kind = ? AND target_id = ?
              AND relation_kind = ?
              AND status = 'accepted'
              AND id != ?
            "#,
        )
        .bind(&source_kind)
        .bind(&source_id)
        .bind(&target_kind)
        .bind(&target_id)
        .bind(opposite)
        .bind(accepted_inference_id)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?;
        for (id,) in rows {
            sqlx::query(
                r#"
                UPDATE brain_inferences
                SET status = 'rejected',
                    superseded_by = ?,
                    supersede_reason = 'predicate_opposite',
                    reviewed_at = ?,
                    updated_at = ?
                WHERE id = ?
                "#,
            )
            .bind(accepted_inference_id)
            .bind(now)
            .bind(now)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
            superseded_ids.push(id);
        }
    }

    // Rule 2: dominance — any pending sibling with a different relation_kind.
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT id FROM brain_inferences
        WHERE source_kind = ? AND source_id = ?
          AND target_kind = ? AND target_id = ?
          AND relation_kind != ?
          AND status = 'pending'
          AND id != ?
        "#,
    )
    .bind(&source_kind)
    .bind(&source_id)
    .bind(&target_kind)
    .bind(&target_id)
    .bind(&relation_kind)
    .bind(accepted_inference_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (id,) in rows {
        sqlx::query(
            r#"
            UPDATE brain_inferences
            SET status = 'rejected',
                superseded_by = ?,
                supersede_reason = 'dominance',
                reviewed_at = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(accepted_inference_id)
        .bind(now)
        .bind(now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
        superseded_ids.push(id);
    }

    // Emit RL events for each superseded inference so any policy keyed
    // on those template/item pairs sees the negative signal.
    for inference_id in superseded_ids {
        let _ = sqlx::query(
            r#"
            INSERT INTO brain_rl_events
              (id, template, item_id, item_kind, event_type, reward,
               context_json, created_at)
            VALUES (?, 'inference_supersede', ?, 'inference', 'superseded',
                    -0.4, '{}', ?)
            "#,
        )
        .bind(format!("evt_{}", Ulid::new()))
        .bind(&inference_id)
        .bind(now)
        .execute(pool)
        .await;
    }

    Ok(())
}

/// Target precision for inference acceptance. The recompute task moves
/// each template's threshold toward the value that hits this rate.
const INFERENCE_TARGET_PRECISION: f64 = 0.80;
/// Minimum reviewed events before we touch a threshold. Below this we
/// don't have enough signal to recompute reliably.
const INFERENCE_THRESHOLD_MIN_SAMPLES: i64 = 20;
/// Cap per-recompute movement so a noisy day can't oscillate the policy.
const INFERENCE_THRESHOLD_STEP: f64 = 0.03;
/// Lookback window for recompute. 90 days is enough to capture seasonal
/// shifts in user behavior without weighting ancient feedback.
const INFERENCE_THRESHOLD_LOOKBACK_DAYS: i64 = 90;

/// One-shot snapshot of the learned-retrieval state. Used by the Section
/// 6.2 RL feedback surface (and any "is the bandit actually learning?"
/// diagnostic). Surfaces:
/// - Observation counts per policy.
/// - Current blend weights (read out of the retrieval_blend policy after
///   the same cold-start interpolation `retrieve_brain_context` uses).
/// - Per-template inference thresholds.

/// Walk every template in `inference_thresholds` and shift each threshold
/// toward the precision target based on accepted/rejected events in the
/// lookback window. Idempotent — safe to call on a 60-minute timer.
pub async fn recompute_inference_thresholds(pool: &SqlitePool) -> Result<(), String> {
    let templates: Vec<(String, f64)> =
        sqlx::query_as("SELECT template, threshold FROM inference_thresholds")
            .fetch_all(pool)
            .await
            .map_err(sql_error)?;

    for (template, current) in templates {
        recompute_inference_threshold_inner(pool, &template, current).await?;
    }

    Ok(())
}

/// Section 6.2 — synchronous per-template threshold recompute. Used inside
/// `review_inference` to give the user an accurate "threshold moved" toast
/// right after they accept/reject a row, without waiting for the hourly tick.
pub async fn recompute_inference_threshold_for(
    pool: &SqlitePool,
    template: &str,
) -> Result<(), String> {
    let row: Option<(f64,)> =
        sqlx::query_as("SELECT threshold FROM inference_thresholds WHERE template = ?")
            .bind(template)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?;
    if let Some((current,)) = row {
        recompute_inference_threshold_inner(pool, template, current).await?;
    }
    Ok(())
}

async fn recompute_inference_threshold_inner(
    pool: &SqlitePool,
    template: &str,
    current: f64,
) -> Result<(), String> {
    let cutoff = (Utc::now() - Duration::days(INFERENCE_THRESHOLD_LOOKBACK_DAYS))
        .to_rfc3339();
    let stats: Option<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT
            SUM(CASE WHEN event_type IN ('accepted_inference', 'useful') THEN 1 ELSE 0 END),
            SUM(CASE WHEN event_type IN ('rejected_inference', 'wrong', 'superseded') THEN 1 ELSE 0 END)
        FROM brain_rl_events
        WHERE template = ? AND created_at >= ?
        "#,
    )
    .bind(template)
    .bind(&cutoff)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let (accepted, rejected) = stats.unwrap_or((0, 0));
    let sample_count = accepted + rejected;
    if sample_count < INFERENCE_THRESHOLD_MIN_SAMPLES {
        return Ok(());
    }
    let precision = accepted as f64 / sample_count.max(1) as f64;

    // High precision → admit more (lower threshold). Low precision →
    // tighten (raise threshold). Step is bounded to avoid oscillation.
    let delta = if precision > INFERENCE_TARGET_PRECISION + 0.05 {
        -INFERENCE_THRESHOLD_STEP
    } else if precision < INFERENCE_TARGET_PRECISION - 0.05 {
        INFERENCE_THRESHOLD_STEP
    } else {
        0.0
    };
    if delta == 0.0 {
        return Ok(());
    }
    let next = (current + delta).clamp(0.4, 0.95);
    let now = now_utc();
    sqlx::query(
        r#"
        UPDATE inference_thresholds
        SET threshold = ?, sample_count = ?, last_recomputed = ?
        WHERE template = ?
        "#,
    )
    .bind(next)
    .bind(sample_count)
    .bind(&now)
    .bind(template)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Section 6.2 — RL feedback surface
// ═══════════════════════════════════════════════════════════════════════
//
// Paginated inference review queue, per-inference accept/reject, supersession
// log + revert, weekly digest, per-template detail, and template-scoped
// learning reset. All of this rides on top of pre-existing tables
// (`brain_inferences`, `brain_rl_events`, `brain_rl_policies`,
// `inference_thresholds`); we add nothing schema-side except the
// `brain_inferences.template` column added by migration 0052.

const REVIEW_REWARD_ACCEPT: f64 = 0.8;
const REVIEW_REWARD_REJECT: f64 = -0.8;
const SUPERSEDE_REVERT_REWARD: f64 = 0.4;

/// List pending (default) or filtered inferences for the review queue.
/// JOINs against `inference_thresholds` so each row carries the current
/// threshold for its template. Subject/target labels are resolved via
/// per-kind dispatch and best-effort (the queue is bounded to ~50 rows,
/// so N+1 is fine).
pub async fn list_brain_inferences(
    pool: &SqlitePool,
    filter: BrainInferenceFilter,
) -> Result<BrainInferenceListResult, String> {
    let status = filter
        .status
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "pending".to_string());
    let template_filter = filter
        .template
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let limit = filter
        .limit
        .unwrap_or(50)
        .clamp(1, 200);
    let before_cursor = filter
        .before_updated_at
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // Fetch limit+1 so we can tell the caller whether there's more.
    let fetch_limit = limit + 1;

    // Reuse the FromRow-derived `BrainInferenceRecord` so we stay under
    // sqlx's tuple FromRow size limit. Threshold lookup is a separate
    // batched query below.
    let mut query = String::from(
        r#"
        SELECT id, source_kind, source_id, relation_kind, target_kind, target_id,
               confidence, rationale, evidence_json, status, generated_by,
               created_at, updated_at, reviewed_at,
               template, superseded_by, supersede_reason
        FROM brain_inferences
        WHERE status = ?
        "#,
    );
    if template_filter.is_some() {
        query.push_str(" AND template = ?");
    }
    if before_cursor.is_some() {
        query.push_str(" AND updated_at < ?");
    }
    query.push_str(" ORDER BY updated_at DESC, id DESC LIMIT ?");

    let mut builder = sqlx::query_as::<_, BrainInferenceRecord>(&query).bind(&status);
    if let Some(ref template) = template_filter {
        builder = builder.bind(template);
    }
    if let Some(ref cursor) = before_cursor {
        builder = builder.bind(cursor);
    }
    let records = builder
        .bind(fetch_limit)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?;

    // Batch threshold lookup keyed by template — saves N+1 queries when the
    // page is monomorphic on a template, and is still cheap when mixed.
    let thresholds = template_threshold_map(pool).await;

    let mut items = Vec::with_capacity(records.len().min(limit as usize));
    let has_more = records.len() as i64 > limit;
    for (i, record) in records.into_iter().enumerate() {
        if i as i64 >= limit {
            break;
        }
        let subject_label = resolve_inference_label(pool, &record.source_kind, &record.source_id).await;
        let target_label = resolve_inference_label(pool, &record.target_kind, &record.target_id).await;
        let threshold = record
            .template
            .as_ref()
            .and_then(|t| thresholds.get(t.as_str()).copied());
        items.push(BrainInferenceRow {
            record,
            subject_label,
            target_label,
            threshold,
        });
    }

    let next_cursor = if has_more {
        items.last().map(|row| row.record.updated_at.clone())
    } else {
        None
    };

    // Counts for the queue's header tiles. Window = 7 days for accepted/rejected.
    let cutoff_7d = (Utc::now() - Duration::days(7)).to_rfc3339();
    let total_pending: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_inferences WHERE status = 'pending'",
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    let total_accepted_7d: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_inferences WHERE status = 'accepted' AND reviewed_at >= ?",
    )
    .bind(&cutoff_7d)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    let total_rejected_7d: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_inferences WHERE status = 'rejected' AND reviewed_at >= ?",
    )
    .bind(&cutoff_7d)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    Ok(BrainInferenceListResult {
        items,
        total_pending: total_pending.0,
        total_accepted_7d: total_accepted_7d.0,
        total_rejected_7d: total_rejected_7d.0,
        has_more,
        next_cursor,
    })
}

/// Per-kind best-effort label lookup. Returns None when the row's been
/// deleted or when the kind isn't one we know how to resolve.
async fn resolve_inference_label(
    pool: &SqlitePool,
    kind: &str,
    id: &str,
) -> Option<String> {
    let query = match kind {
        "deliverable" => Some("SELECT title FROM deliverables WHERE id = ?"),
        "initiative" => Some("SELECT title FROM initiatives WHERE id = ?"),
        "stakeholder" => Some("SELECT name FROM stakeholders WHERE id = ?"),
        "meeting" => Some("SELECT title FROM meetings WHERE id = ?"),
        "meeting_action" => Some("SELECT body FROM meeting_actions WHERE id = ?"),
        "capture" => Some("SELECT COALESCE(NULLIF(title, ''), body) FROM captures WHERE id = ?"),
        "email_thread" => Some("SELECT subject FROM gmail_threads WHERE thread_id = ?"),
        _ => None,
    }?;
    let row: Option<(Option<String>,)> = sqlx::query_as(query)
        .bind(id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    row.and_then(|(label,)| label)
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty())
}

/// Accept or reject a single inference. Wraps `apply_feedback_to_inferences`
/// so the existing auto-supersede + status-update logic is reused verbatim,
/// then records a per-template `brain_rl_events` row + recomputes that
/// template's threshold synchronously so the UI can show a "threshold
/// moved from X → Y" toast that actually reflects reality.
pub async fn review_inference(
    pool: &SqlitePool,
    inference_id: &str,
    decision: &str,
) -> Result<ReviewInferenceResult, String> {
    let decision = decision.trim().to_ascii_lowercase();
    if !matches!(decision.as_str(), "accepted" | "rejected") {
        return Err("decision must be 'accepted' or 'rejected'".to_string());
    }

    // Read the row first so we know its template + can guard against
    // already-reviewed rows.
    let row: Option<(String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, template, status FROM brain_inferences WHERE id = ?",
    )
    .bind(inference_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;
    let (resolved_id, template, _existing_status) =
        row.ok_or_else(|| format!("inference {inference_id} not found"))?;

    // Snapshot the pre-update threshold so the IPC return can show a delta.
    let threshold_before = template_threshold(pool, template.as_deref()).await;

    // Translate "accepted" / "rejected" to the legacy feedback shape that
    // `apply_feedback_to_inferences` already speaks. This reuses the
    // auto-supersede + corrected-row plumbing without duplicating it.
    let feedback = if decision == "accepted" { "useful" } else { "wrong" };
    let key = if decision == "accepted" {
        "accepted_inference_ids"
    } else {
        "rejected_inference_ids"
    };
    let corrected = json!({ key: [&resolved_id] });
    let now = now_utc();
    apply_feedback_to_inferences(pool, feedback, &corrected, &now).await?;

    // Capture any siblings that were auto-superseded by this accept so the UI
    // can show a follow-up "X superseded" indicator.
    let superseded_inference_ids: Vec<String> = if decision == "accepted" {
        sqlx::query_as::<_, (String,)>(
            "SELECT id FROM brain_inferences WHERE superseded_by = ? AND reviewed_at = ?",
        )
        .bind(&resolved_id)
        .bind(&now)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
        .into_iter()
        .map(|(id,)| id)
        .collect()
    } else {
        Vec::new()
    };

    // Per-template RL event so the threshold recompute has data to act on.
    if let Some(template_name) = template.clone() {
        let event_type = if decision == "accepted" {
            "accepted_inference"
        } else {
            "rejected_inference"
        };
        let reward = if decision == "accepted" {
            REVIEW_REWARD_ACCEPT
        } else {
            REVIEW_REWARD_REJECT
        };
        let _ = record_brain_learning_event(
            pool,
            BrainLearningEventInput {
                template: Some(template_name.clone()),
                item_id: resolved_id.clone(),
                item_kind: Some("brain_inference".to_string()),
                event_type: event_type.to_string(),
                reward: Some(reward),
                context: Some(json!({ "via": "review_inference" })),
            },
        )
        .await;
        // Synchronously recompute the threshold so the toast is honest.
        let _ = recompute_inference_threshold_for(pool, &template_name).await;
    }

    let threshold_after = template_threshold(pool, template.as_deref()).await;
    let sample_count = template_sample_count(pool, template.as_deref()).await;

    Ok(ReviewInferenceResult {
        inference_id: resolved_id,
        status: decision,
        template,
        threshold_before,
        threshold_after,
        sample_count,
        superseded_inference_ids,
    })
}

/// One-shot read of every template's current threshold. Bounded — we
/// seed only ~5 rows. Used by `list_brain_inferences` /
/// `list_inference_supersessions` to avoid N+1 lookups in the queue.
async fn template_threshold_map(pool: &SqlitePool) -> std::collections::HashMap<String, f64> {
    let rows: Vec<(String, f64)> =
        match sqlx::query_as("SELECT template, threshold FROM inference_thresholds")
            .fetch_all(pool)
            .await
        {
            Ok(rows) => rows,
            Err(_) => return std::collections::HashMap::new(),
        };
    rows.into_iter().collect()
}

async fn template_threshold(pool: &SqlitePool, template: Option<&str>) -> Option<f64> {
    let template = template?;
    let row: Option<(f64,)> =
        sqlx::query_as("SELECT threshold FROM inference_thresholds WHERE template = ?")
            .bind(template)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    row.map(|(t,)| t)
}

async fn template_sample_count(pool: &SqlitePool, template: Option<&str>) -> Option<i64> {
    let template = template?;
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT sample_count FROM inference_thresholds WHERE template = ?")
            .bind(template)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    row.map(|(c,)| c)
}

/// List the most recent (loser, winner) supersession pairs. We do the
/// JOIN logically in code (two fetches) so we stay under sqlx's tuple
/// FromRow size limit and keep each query mappable to
/// `BrainInferenceRecord`.
pub async fn list_inference_supersessions(
    pool: &SqlitePool,
    limit: Option<i64>,
) -> Result<Vec<SupersessionRecord>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let losers = sqlx::query_as::<_, BrainInferenceRecord>(
        r#"
        SELECT id, source_kind, source_id, relation_kind, target_kind, target_id,
               confidence, rationale, evidence_json, status, generated_by,
               created_at, updated_at, reviewed_at,
               template, superseded_by, supersede_reason
        FROM brain_inferences
        WHERE superseded_by IS NOT NULL
        ORDER BY updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let thresholds = template_threshold_map(pool).await;
    let mut out = Vec::with_capacity(losers.len());
    for loser in losers {
        let winner_id = match loser.superseded_by.as_deref() {
            Some(id) => id.to_string(),
            None => continue,
        };
        let winner = sqlx::query_as::<_, BrainInferenceRecord>(
            r#"
            SELECT id, source_kind, source_id, relation_kind, target_kind, target_id,
                   confidence, rationale, evidence_json, status, generated_by,
                   created_at, updated_at, reviewed_at,
                   template, superseded_by, supersede_reason
            FROM brain_inferences
            WHERE id = ?
            "#,
        )
        .bind(&winner_id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;
        let Some(winner) = winner else {
            // Winner row was deleted — skip rather than break the page.
            continue;
        };
        let supersede_reason = loser.supersede_reason.clone().unwrap_or_default();
        let superseded_at = loser.updated_at.clone();
        let loser_subject = resolve_inference_label(pool, &loser.source_kind, &loser.source_id).await;
        let loser_target = resolve_inference_label(pool, &loser.target_kind, &loser.target_id).await;
        let winner_subject = resolve_inference_label(pool, &winner.source_kind, &winner.source_id).await;
        let winner_target = resolve_inference_label(pool, &winner.target_kind, &winner.target_id).await;
        let loser_threshold = loser.template.as_deref().and_then(|t| thresholds.get(t).copied());
        let winner_threshold = winner.template.as_deref().and_then(|t| thresholds.get(t).copied());
        out.push(SupersessionRecord {
            loser: BrainInferenceRow {
                record: loser,
                subject_label: loser_subject,
                target_label: loser_target,
                threshold: loser_threshold,
            },
            winner: BrainInferenceRow {
                record: winner,
                subject_label: winner_subject,
                target_label: winner_target,
                threshold: winner_threshold,
            },
            supersede_reason,
            superseded_at,
        });
    }
    Ok(out)
}

/// Revert an auto-supersession. Clears `superseded_by` + `supersede_reason`
/// on the loser, but does NOT mutate the loser's `status` — re-pending a
/// row the bandit already rejected is destructive. Emits a compensating
/// `event_type='reverted'` row so the audit trail stays honest.
pub async fn revert_inference_supersession(
    pool: &SqlitePool,
    loser_inference_id: &str,
) -> Result<(), String> {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT superseded_by, template FROM brain_inferences WHERE id = ?",
    )
    .bind(loser_inference_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;
    let (superseded_by, template) = match row {
        Some(r) => r,
        None => return Err(format!("inference {loser_inference_id} not found")),
    };
    if superseded_by.is_none() {
        // Already reverted — no-op so the UI's optimistic remove is forgiving.
        return Ok(());
    }
    let now = now_utc();
    sqlx::query(
        r#"
        UPDATE brain_inferences
        SET superseded_by = NULL,
            supersede_reason = NULL,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&now)
    .bind(loser_inference_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    // Mirror the original -0.4 `superseded` event with a +0.4 revert so the
    // threshold recompute reweights toward the original bias.
    let _ = record_brain_learning_event(
        pool,
        BrainLearningEventInput {
            template: template.or_else(|| Some("inference_supersede".to_string())),
            item_id: loser_inference_id.to_string(),
            item_kind: Some("brain_inference".to_string()),
            event_type: "reverted".to_string(),
            reward: Some(SUPERSEDE_REVERT_REWARD),
            context: Some(json!({ "via": "revert_inference_supersession" })),
        },
    )
    .await;
    Ok(())
}
