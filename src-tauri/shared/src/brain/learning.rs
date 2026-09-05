//! Section 6.2 learning diagnostics surface.
//!
//! Four read APIs power the Brain UI's "is the bandit actually learning?"
//! panel:
//!
//! - `get_brain_learning_summary` — observation counts per policy, the live
//!   retrieval-blend weights, and every per-template inference threshold.
//! - `get_rl_digest` — windowed acceptance / supersession / Ask-feedback
//!   counts plus a threshold-drift summary.
//! - `get_template_detail` — single-template card with feature coefficients
//!   and the last 20 timeline events.
//! - `reset_brain_template_learning` — wipe a template's policy + per-item
//!   scores while preserving the audit trail; writes a `policy_reset` event.
//!
//! All four go through helpers that still live in `super::legacy`:
//! `sql_error`, `load_retrieval_blend_weights`, `record_brain_learning_event`,
//! `invalidate_rl_policy`, `invalidate_rl_item_scores`.

use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::models::{
    BrainLearningEvent, BrainLearningEventInput, BrainRlPolicyRecord, RLDigest,
    TemplateCoefficient, TemplateDetail, TopTemplateSummary,
};

use super::legacy::sql_error;
use super::retrieval::load_retrieval_blend_weights;
use super::rl::{invalidate_rl_item_scores, invalidate_rl_policy, record_brain_learning_event};

pub async fn get_brain_learning_summary(
    pool: &SqlitePool,
) -> Result<crate::models::BrainLearningSummary, String> {
    let policy_rows: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT template, observations, updated_at FROM brain_rl_policies",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    let policies = policy_rows
        .into_iter()
        .map(|(template, observations, updated_at)| {
            crate::models::BrainPolicySummary {
                template,
                observations,
                updated_at,
            }
        })
        .collect::<Vec<_>>();

    let blend = load_retrieval_blend_weights(pool).await;
    let blend_summary = crate::models::BrainRetrievalBlend {
        bm25: blend.bm25,
        cosine: blend.cosine,
        node_weight: blend.node_weight,
        focus_proximity: blend.focus_proximity,
    };

    let threshold_rows: Vec<(String, f64, i64, String)> = sqlx::query_as(
        "SELECT template, threshold, sample_count, last_recomputed FROM inference_thresholds",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    let thresholds = threshold_rows
        .into_iter()
        .map(
            |(template, threshold, sample_count, last_recomputed)| {
                crate::models::InferenceThresholdSummary {
                    template,
                    threshold,
                    sample_count,
                    last_recomputed,
                }
            },
        )
        .collect::<Vec<_>>();

    Ok(crate::models::BrainLearningSummary {
        policies,
        blend: blend_summary,
        inference_thresholds: thresholds,
    })
}
pub async fn get_rl_digest(pool: &SqlitePool, days: Option<i64>) -> Result<RLDigest, String> {
    let window_days = days.unwrap_or(7).clamp(1, 90);
    let cutoff = (Utc::now() - Duration::days(window_days)).to_rfc3339();

    let inferences_generated: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_inferences WHERE created_at >= ?",
    )
    .bind(&cutoff)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    let inferences_accepted: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_inferences WHERE status = 'accepted' AND reviewed_at >= ?",
    )
    .bind(&cutoff)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    let inferences_rejected: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_inferences WHERE status = 'rejected' AND reviewed_at >= ?",
    )
    .bind(&cutoff)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    let supersessions: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_inferences WHERE superseded_by IS NOT NULL AND updated_at >= ?",
    )
    .bind(&cutoff)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    let ask_useful: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_rl_events WHERE event_type = 'useful' AND created_at >= ?",
    )
    .bind(&cutoff)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    let ask_wrong: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM brain_rl_events WHERE event_type = 'wrong' AND created_at >= ?",
    )
    .bind(&cutoff)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    let top_template_row: Option<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT template,
               SUM(CASE WHEN event_type = 'accepted_inference' THEN 1 ELSE 0 END) AS accepted,
               SUM(CASE WHEN event_type = 'rejected_inference' THEN 1 ELSE 0 END) AS rejected
        FROM brain_rl_events
        WHERE created_at >= ?
          AND event_type IN ('accepted_inference', 'rejected_inference')
          AND template IS NOT NULL
        GROUP BY template
        ORDER BY accepted DESC, rejected ASC
        LIMIT 1
        "#,
    )
    .bind(&cutoff)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    // `entity_embeddings.embedded_at` is a Unix epoch (INTEGER, not RFC-3339),
    // so the cutoff for this single query is computed separately.
    let cutoff_epoch = (Utc::now() - Duration::days(window_days)).timestamp();
    let embeddings_added: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entity_embeddings WHERE embedded_at >= ?",
    )
    .bind(cutoff_epoch)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    // threshold_drift: count thresholds touched in the window and average
    // the absolute distance from each one's seeded baseline. Cheap proxy
    // for "is the bandit churning thresholds or stable?"
    let drift_rows: Vec<(String, f64, String)> = sqlx::query_as(
        r#"
        SELECT template, threshold, last_recomputed
        FROM inference_thresholds
        WHERE last_recomputed >= ?
        "#,
    )
    .bind(&cutoff)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    let threshold_drift = if drift_rows.is_empty() {
        0.0
    } else {
        let sum: f64 = drift_rows
            .iter()
            .map(|(template, threshold, _)| (threshold - seeded_threshold(template)).abs())
            .sum();
        sum / drift_rows.len() as f64
    };

    let reviewed = inferences_accepted.0 + inferences_rejected.0;
    let acceptance_rate = if reviewed > 0 {
        inferences_accepted.0 as f64 / reviewed as f64
    } else {
        0.0
    };
    let ask_total = ask_useful.0 + ask_wrong.0;
    let ask_feedback_rate = if ask_total > 0 {
        ask_useful.0 as f64 / ask_total as f64
    } else {
        0.0
    };
    let top_template = top_template_row.map(|(name, accepted, rejected)| {
        let total = accepted + rejected;
        let rate = if total > 0 {
            accepted as f64 / total as f64
        } else {
            0.0
        };
        TopTemplateSummary {
            name,
            accepted,
            rejected,
            acceptance_rate: rate,
        }
    });

    Ok(RLDigest {
        window_days,
        inferences_generated: inferences_generated.0,
        inferences_accepted: inferences_accepted.0,
        inferences_rejected: inferences_rejected.0,
        acceptance_rate,
        supersessions: supersessions.0,
        top_template,
        ask_useful: ask_useful.0,
        ask_wrong: ask_wrong.0,
        ask_feedback_rate,
        embeddings_added: embeddings_added.0,
        threshold_drift,
    })
}

fn seeded_threshold(template: &str) -> f64 {
    match template {
        "meeting_action_exact" => 0.86,
        "meeting_action_fuzzy" => 0.72,
        "email_thread_mention" => 0.64,
        "blocker_email_match" => 0.88,
        "blocker_fuzzy" => 0.74,
        _ => 0.5,
    }
}
/// Per-template detail card. Loads the policy row + threshold + parses the
/// stored A-matrix / b-vector into a feature coefficient summary, plus the
/// last 20 events for the timeline.
pub async fn get_template_detail(
    pool: &SqlitePool,
    template: &str,
) -> Result<TemplateDetail, String> {
    let policy: Option<BrainRlPolicyRecord> = sqlx::query_as::<_, BrainRlPolicyRecord>(
        r#"
        SELECT template, feature_names_json, a_matrix_json, b_vector_json,
               observations, alpha, updated_at
        FROM brain_rl_policies
        WHERE template = ?
        "#,
    )
    .bind(template)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let threshold_row: Option<(f64, i64)> = sqlx::query_as(
        "SELECT threshold, sample_count FROM inference_thresholds WHERE template = ?",
    )
    .bind(template)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let (feature_names, coefficient_summary, observations) = match policy.as_ref() {
        Some(policy_row) => {
            let feature_names: Vec<String> =
                serde_json::from_str(&policy_row.feature_names_json).unwrap_or_default();
            let a_matrix: Vec<Vec<f64>> =
                serde_json::from_str(&policy_row.a_matrix_json).unwrap_or_default();
            let mut summary = Vec::with_capacity(feature_names.len());
            for (col, name) in feature_names.iter().enumerate() {
                if let Some(row) = a_matrix.get(col) {
                    let n = row.len() as f64;
                    let sum: f64 = row.iter().copied().sum();
                    let mean = if n > 0.0 { sum / n } else { 0.0 };
                    let abs_max = row.iter().copied().fold(0.0_f64, |acc, v| acc.max(v.abs()));
                    summary.push(TemplateCoefficient {
                        feature: name.clone(),
                        mean,
                        abs_max,
                    });
                }
            }
            (feature_names, summary, policy_row.observations)
        }
        None => (Vec::new(), Vec::new(), 0),
    };

    let recent_events = sqlx::query_as::<_, BrainLearningEvent>(
        r#"
        SELECT id, template, item_id, item_kind, event_type, reward,
               context_json, created_at
        FROM brain_rl_events
        WHERE template = ?
        ORDER BY created_at DESC
        LIMIT 20
        "#,
    )
    .bind(template)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    Ok(TemplateDetail {
        template: template.to_string(),
        observations,
        threshold: threshold_row.as_ref().map(|(t, _)| *t),
        sample_count: threshold_row.as_ref().map(|(_, c)| *c),
        feature_names,
        coefficient_summary,
        recent_events,
    })
}

/// Reset a template's learned policy. Wipes the A/b matrices and per-item
/// scores so the next observation starts from baseline. Audit trail (the
/// events table) is preserved — only the live policy is gone. A
/// `event_type='policy_reset'` row is added so the timeline shows what
/// happened.
pub async fn reset_brain_template_learning(
    pool: &SqlitePool,
    template: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM brain_rl_policies WHERE template = ?")
        .bind(template)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    sqlx::query("DELETE FROM brain_rl_item_scores WHERE template = ?")
        .bind(template)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    let _ = record_brain_learning_event(
        pool,
        BrainLearningEventInput {
            template: Some(template.to_string()),
            item_id: format!("reset_{}", Ulid::new()),
            item_kind: Some("policy".to_string()),
            event_type: "policy_reset".to_string(),
            reward: Some(0.0),
            context: Some(json!({ "via": "user_reset" })),
        },
    )
    .await;
    invalidate_rl_policy(template);
    invalidate_rl_item_scores(template);
    Ok(())
}
