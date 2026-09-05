//! Brain reinforcement-learning surface (Section 6.2 bandit).
//!
//! This module owns the bandit policy machinery, the per-template item-score
//! cache, the feedback-event pipeline, and the public Ask-feedback /
//! learning-snapshot APIs:
//!
//! - `BrainRlPolicy` / `BrainRlScore` + `apply_learned_ranking` apply the
//!   learned A-matrix / b-vector ranking on top of a heuristic
//!   `BrainTemplateResult`.
//! - `load_rl_policy*`, `update_rl_policy*`, `upsert_rl_item_score`,
//!   `attach_rl_score` are the SQL chokepoints. The `RL_READ_CACHE` mirrors
//!   `inference_thresholds`-style memoization for the hot read path.
//! - `record_brain_feedback`, `record_brain_learning_event`,
//!   `get_brain_learning_snapshot`, `fan_out_retrieval_events`,
//!   `record_feedback_learning_events` form the Section 6.2 feedback surface.
//! - `brain_rl_features` builds the 27-dim feature vector consumed by every
//!   policy in the RL bandit.
//!
//! Heavy linear-algebra primitives (`invert_matrix`, `mat_vec_mul`, `dot`,
//! `parse_matrix`, `parse_vector`, `identity_matrix`) live here too rather
//! than in a separate math module — they're only ever exercised through the
//! LinUCB update path in `update_rl_policy_with_features`.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use serde_json::{json, Map};
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::models::{
    BrainFeedbackInput, BrainLearningEvent, BrainLearningEventInput, BrainLearningSnapshot,
    BrainRlItemScore, BrainRlPolicyRecord, BrainTemplateResult, WorkGraph, WorkGraphNode,
};

use super::inferences::apply_feedback_to_inferences;
use super::legacy::{
    date_is_before_today, date_is_within_days, now_utc, parse_date_prefix, sql_error,
};
use super::retrieval::{NODE_IMPORTANCE_TEMPLATE, RETRIEVAL_BLEND_FEATURES, RETRIEVAL_BLEND_TEMPLATE};
use super::templates::{payload_bool, payload_string};

const BRAIN_RL_ALPHA: f64 = 0.75;
const BRAIN_RL_FEATURES: [&str; 27] = [
    "bias",
    "node_weight",
    "is_deliverable",
    "is_task",
    "is_email_followup",
    "is_blocker",
    "is_open_loop",
    "is_attention_signal",
    "is_stakeholder",
    "is_calendar_event",
    "is_file",
    "status_blocked",
    "status_overdue",
    "status_open",
    "status_current_focus",
    "priority_high",
    "has_due_soon",
    "is_stale",
    "updated_recent",
    "graph_degree",
    "open_loop_neighbors",
    "attention_neighbors",
    "stakeholder_neighbors",
    "email_neighbors",
    "meeting_neighbors",
    "file_neighbors",
    "calendar_neighbors",
];
pub async fn record_brain_feedback(
    pool: &SqlitePool,
    input: BrainFeedbackInput,
) -> Result<(), String> {
    let feedback = input.feedback.trim().to_ascii_lowercase();
    if !matches!(feedback.as_str(), "useful" | "wrong" | "ignored") {
        return Err("feedback must be useful, wrong, or ignored".to_string());
    }
    let corrected = input.corrected.unwrap_or_else(|| json!({}));
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT INTO brain_answer_feedback
          (id, question, template, feedback, corrected_json, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(format!("fb_{}", Ulid::new()))
    .bind(input.question)
    .bind(input.template.clone())
    .bind(&feedback)
    .bind(serde_json::to_string(&corrected).unwrap_or_else(|_| "{}".to_string()))
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    apply_feedback_to_inferences(pool, &feedback, &corrected, &now).await?;
    record_feedback_learning_events(pool, &input.template, &feedback, &corrected).await?;
    Ok(())
}

pub async fn record_brain_learning_event(
    pool: &SqlitePool,
    input: BrainLearningEventInput,
) -> Result<BrainLearningEvent, String> {
    let template = normalize_template_name(input.template.as_deref());
    let item_id = input.item_id.trim().to_string();
    if item_id.is_empty() {
        return Err("brain learning event item_id must not be empty".to_string());
    }
    let item_kind = input
        .item_kind
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let event_type = input.event_type.trim().to_ascii_lowercase();
    if event_type.is_empty() {
        return Err("brain learning event event_type must not be empty".to_string());
    }
    let reward = input
        .reward
        .unwrap_or_else(|| default_reward_for_event(&event_type))
        .clamp(-1.0, 1.0);
    let context = input.context.unwrap_or_else(|| json!({}));
    let context_json = serde_json::to_string(&context).unwrap_or_else(|_| "{}".to_string());
    let created_at = now_utc();
    let event = BrainLearningEvent {
        id: format!("ble_{}", Ulid::new()),
        template,
        item_id,
        item_kind,
        event_type,
        reward,
        context_json,
        created_at,
    };

    sqlx::query(
        r#"
        INSERT INTO brain_rl_events
          (id, template, item_id, item_kind, event_type, reward, context_json, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&event.id)
    .bind(&event.template)
    .bind(&event.item_id)
    .bind(&event.item_kind)
    .bind(&event.event_type)
    .bind(event.reward)
    .bind(&event.context_json)
    .bind(&event.created_at)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if event.reward.abs() > f64::EPSILON {
        match event.template.as_str() {
            RETRIEVAL_BLEND_TEMPLATE => {
                let features = context
                    .get("features")
                    .and_then(raw_feature_vector_from_json)
                    .unwrap_or_else(|| vec![0.0; RETRIEVAL_BLEND_FEATURES.len()]);
                update_rl_policy_with_features(
                    pool,
                    &event.template,
                    &RETRIEVAL_BLEND_FEATURES,
                    &features,
                    event.reward,
                )
                .await?;
            }
            _ => {
                let features = if let Some(features) = context
                    .get("features")
                    .and_then(|value| feature_vector_from_json(value))
                {
                    features
                } else {
                    load_item_features(pool, &event.template, &event.item_id)
                        .await?
                        .unwrap_or_else(default_feature_vector)
                };
                update_rl_policy(pool, &event.template, &features, event.reward).await?;
            }
        }
    }

    Ok(event)
}

/// Parse a context.features payload as a raw f64 vector, preserving the
/// caller's dimensionality. Use for non-27-dim templates (retrieval_blend,
/// node_importance) where the 27-dim normalizer would corrupt the data.
fn raw_feature_vector_from_json(value: &serde_json::Value) -> Option<Vec<f64>> {
    if let Some(array) = value.as_array() {
        return Some(
            array
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0))
                .collect::<Vec<_>>(),
        );
    }
    None
}

pub async fn get_brain_learning_snapshot(
    pool: &SqlitePool,
    template: Option<String>,
    limit: Option<usize>,
) -> Result<BrainLearningSnapshot, String> {
    let limit = limit.unwrap_or(25).clamp(1, 100) as i64;
    let template = template.map(|value| normalize_template_name(Some(&value)));

    let policies = if let Some(template) = &template {
        sqlx::query_as::<_, BrainRlPolicyRecord>(
            r#"
            SELECT template, feature_names_json, a_matrix_json, b_vector_json,
                   observations, alpha, updated_at
            FROM brain_rl_policies
            WHERE template = ?
            "#,
        )
        .bind(template)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    } else {
        sqlx::query_as::<_, BrainRlPolicyRecord>(
            r#"
            SELECT template, feature_names_json, a_matrix_json, b_vector_json,
                   observations, alpha, updated_at
            FROM brain_rl_policies
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    };

    let recent_events = if let Some(template) = &template {
        sqlx::query_as::<_, BrainLearningEvent>(
            r#"
            SELECT id, template, item_id, item_kind, event_type, reward, context_json, created_at
            FROM brain_rl_events
            WHERE template = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(template)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    } else {
        sqlx::query_as::<_, BrainLearningEvent>(
            r#"
            SELECT id, template, item_id, item_kind, event_type, reward, context_json, created_at
            FROM brain_rl_events
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    };

    let top_scores = if let Some(template) = &template {
        sqlx::query_as::<_, BrainRlItemScore>(
            r#"
            SELECT template, item_id, item_kind, score, exploitation, exploration,
                   features_json, updated_at
            FROM brain_rl_item_scores
            WHERE template = ?
            ORDER BY score DESC
            LIMIT ?
            "#,
        )
        .bind(template)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    } else {
        sqlx::query_as::<_, BrainRlItemScore>(
            r#"
            SELECT template, item_id, item_kind, score, exploitation, exploration,
                   features_json, updated_at
            FROM brain_rl_item_scores
            ORDER BY updated_at DESC, score DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    };

    Ok(BrainLearningSnapshot {
        policies,
        recent_events,
        top_scores,
    })
}

pub async fn tool_record_brain_learning_event(
    pool: &SqlitePool,
    input: BrainLearningEventInput,
) -> serde_json::Value {
    match record_brain_learning_event(pool, input).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

pub async fn tool_get_brain_learning_snapshot(
    pool: &SqlitePool,
    template: Option<String>,
    limit: Option<usize>,
) -> serde_json::Value {
    match get_brain_learning_snapshot(pool, template, limit).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

async fn record_feedback_learning_events(
    pool: &SqlitePool,
    template: &Option<String>,
    feedback: &str,
    corrected: &serde_json::Value,
) -> Result<(), String> {
    let reward = default_reward_for_event(feedback);
    if let Some(item_id) = corrected.get("item_id").and_then(|value| value.as_str()) {
        record_brain_learning_event(
            pool,
            BrainLearningEventInput {
                template: template.clone(),
                item_id: item_id.to_string(),
                item_kind: corrected
                    .get("item_kind")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                event_type: feedback.to_string(),
                reward: Some(reward),
                context: corrected.get("context").cloned(),
            },
        )
        .await?;
        fan_out_retrieval_events(pool, item_id, corrected, feedback, reward).await;
    }
    if let Some(items) = corrected.get("item_ids").and_then(|value| value.as_array()) {
        for item_id in items.iter().filter_map(|value| value.as_str()) {
            record_brain_learning_event(
                pool,
                BrainLearningEventInput {
                    template: template.clone(),
                    item_id: item_id.to_string(),
                    item_kind: None,
                    event_type: feedback.to_string(),
                    reward: Some(reward),
                    context: None,
                },
            )
            .await?;
            fan_out_retrieval_events(pool, item_id, corrected, feedback, reward).await;
        }
    }

    Ok(())
}

/// When Ask feedback fires on a cited entity, also feed the
/// retrieval_blend (for blend weights) and node_importance (for per-entity
/// weight) policies. Failures are swallowed — these are best-effort
/// learning signals layered on top of the primary feedback event.
async fn fan_out_retrieval_events(
    pool: &SqlitePool,
    item_id: &str,
    corrected: &serde_json::Value,
    feedback: &str,
    reward: f64,
) {
    // Retrieval-blend event: requires the 5-element feature vector for
    // this citation, sourced from `corrected.retrieval_features[item_id]`
    // (the model copies it over from `BrainContextResult.scored_nodes`).
    let blend_features = corrected
        .get("retrieval_features")
        .and_then(|v| v.get(item_id))
        .cloned();
    if let Some(features) = blend_features {
        let _ = record_brain_learning_event(
            pool,
            BrainLearningEventInput {
                template: Some(RETRIEVAL_BLEND_TEMPLATE.to_string()),
                item_id: item_id.to_string(),
                item_kind: corrected
                    .get("item_kind")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                event_type: feedback.to_string(),
                reward: Some(reward),
                context: Some(json!({ "features": features })),
            },
        )
        .await;
    }

    // Per-entity importance event: no features needed — the bandit keys
    // by item_id alone.
    let _ = record_brain_learning_event(
        pool,
        BrainLearningEventInput {
            template: Some(NODE_IMPORTANCE_TEMPLATE.to_string()),
            item_id: item_id.to_string(),
            item_kind: corrected
                .get("item_kind")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            event_type: feedback.to_string(),
            reward: Some(reward),
            context: None,
        },
    )
    .await;
}

#[derive(Debug, Clone)]
pub(super) struct BrainRlPolicy {
    pub(super) template: String,
    pub(super) feature_names: Vec<String>,
    pub(super) a_matrix: Vec<Vec<f64>>,
    pub(super) b_vector: Vec<f64>,
    pub(super) observations: i64,
    pub(super) alpha: f64,
}

impl BrainRlPolicy {
    fn dimension(&self) -> usize {
        self.feature_names.len().max(1)
    }
}

#[derive(Debug, Clone)]
struct BrainRlScore {
    score: f64,
    exploitation: f64,
    exploration: f64,
    features: Vec<f64>,
}

pub(super) async fn apply_learned_ranking(
    pool: &SqlitePool,
    result: &mut BrainTemplateResult,
) -> Result<(), String> {
    let template = normalize_template_name(Some(&result.template));
    let policy = load_rl_policy(pool, &template).await?;
    let mut scores = BTreeMap::<String, BrainRlScore>::new();

    for node in &result.graph.nodes {
        let features = brain_rl_features(node, &result.graph);
        let score = score_with_policy(node, &features, &policy);
        upsert_rl_item_score(pool, &template, node, &score).await?;
        scores.insert(node.id.clone(), score);
    }

    for node in &mut result.graph.nodes {
        if let Some(score) = scores.get(&node.id) {
            attach_rl_score(node, &template, score);
        }
    }
    result.graph.nodes.sort_by(|left, right| {
        let left_score = scores
            .get(&left.id)
            .map(|score| score.score)
            .unwrap_or(left.weight as f64 / 10.0);
        let right_score = scores
            .get(&right.id)
            .map(|score| score.score)
            .unwrap_or(right.weight as f64 / 10.0);
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.weight.cmp(&left.weight))
            .then_with(|| left.label.cmp(&right.label))
    });

    for row in &mut result.rows {
        if let Some(id) = row.get("id").and_then(|value| value.as_str()) {
            if let Some(score) = scores.get(id) {
                if let Some(object) = row.as_object_mut() {
                    object.insert("brain_rl_score".to_string(), json!(score.score));
                    object.insert(
                        "brain_rl_exploitation".to_string(),
                        json!(score.exploitation),
                    );
                    object.insert("brain_rl_exploration".to_string(), json!(score.exploration));
                }
            }
        }
    }
    result.rows.sort_by(|left, right| {
        let left_score = left
            .get("brain_rl_score")
            .and_then(|value| value.as_f64())
            .unwrap_or_else(|| {
                left.get("weight")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0)
            });
        let right_score = right
            .get("brain_rl_score")
            .and_then(|value| value.as_f64())
            .unwrap_or_else(|| {
                right
                    .get("weight")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0)
            });
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(())
}

// ───────────────────────── RL read cache ─────────────────────────────────
//
// `load_rl_policy_with_features` and `load_node_importance_scores` run on the
// `retrieve_brain_context` hot path — once or twice per Ask turn. Hitting
// SQLite every time is sub-millisecond but compounds, and the data only
// changes when `update_rl_*` writes. A small in-process TTL cache eliminates
// the round-trip for the common read-heavy case.

pub(super) const RL_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(5 * 60);

#[derive(Default)]
pub(super) struct RlReadCache {
    pub(super) policies: std::collections::HashMap<String, (std::time::Instant, BrainRlPolicy)>,
    /// Snapshot of `brain_rl_item_scores` per template. Key = template name;
    /// value = (cached_at, item_id → score).
    pub(super) item_scores: std::collections::HashMap<
        String,
        (std::time::Instant, std::collections::HashMap<String, f64>),
    >,
}

static RL_READ_CACHE: std::sync::OnceLock<std::sync::Mutex<RlReadCache>> =
    std::sync::OnceLock::new();

pub(super) fn rl_cache() -> &'static std::sync::Mutex<RlReadCache> {
    RL_READ_CACHE.get_or_init(|| std::sync::Mutex::new(RlReadCache::default()))
}

pub(super) fn invalidate_rl_policy(template: &str) {
    if let Ok(mut guard) = rl_cache().lock() {
        guard.policies.remove(template);
    }
}

pub(super) fn invalidate_rl_item_scores(template: &str) {
    if let Ok(mut guard) = rl_cache().lock() {
        guard.item_scores.remove(template);
    }
}

async fn load_rl_policy(pool: &SqlitePool, template: &str) -> Result<BrainRlPolicy, String> {
    load_rl_policy_with_features(pool, template, &BRAIN_RL_FEATURES).await
}

/// Load a learned policy, declaring the default feature schema for cold
/// starts. The dimension is determined by the stored `feature_names_json`
/// when present, falling back to `default_features` length when missing.
pub(super) async fn load_rl_policy_with_features(
    pool: &SqlitePool,
    template: &str,
    default_features: &[&str],
) -> Result<BrainRlPolicy, String> {
    // Cache hit fast-path — invalidated by every `update_rl_policy_with_features` write
    // so staleness is bounded by writes, not by the TTL.
    {
        let now = std::time::Instant::now();
        if let Ok(guard) = rl_cache().lock() {
            if let Some((cached_at, policy)) = guard.policies.get(template) {
                if now.duration_since(*cached_at) < RL_CACHE_TTL {
                    return Ok(policy.clone());
                }
            }
        }
    }
    let row = sqlx::query_as::<_, BrainRlPolicyRecord>(
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

    let default_names: Vec<String> = default_features
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    let policy = if let Some(row) = row {
        let stored_names: Vec<String> =
            serde_json::from_str(&row.feature_names_json).unwrap_or_default();
        let feature_names = if stored_names.is_empty() {
            default_names
        } else {
            stored_names
        };
        let dimension = feature_names.len().max(1);
        let a_matrix = parse_matrix(&row.a_matrix_json, dimension);
        let b_vector = parse_vector(&row.b_vector_json, dimension);
        BrainRlPolicy {
            template: row.template,
            feature_names,
            a_matrix,
            b_vector,
            observations: row.observations,
            alpha: row.alpha,
        }
    } else {
        let dimension = default_names.len().max(1);
        BrainRlPolicy {
            template: template.to_string(),
            feature_names: default_names,
            a_matrix: identity_matrix(dimension),
            b_vector: vec![0.0; dimension],
            observations: 0,
            alpha: BRAIN_RL_ALPHA,
        }
    };

    if let Ok(mut guard) = rl_cache().lock() {
        guard
            .policies
            .insert(template.to_string(), (std::time::Instant::now(), policy.clone()));
    }
    Ok(policy)
}

async fn update_rl_policy(
    pool: &SqlitePool,
    template: &str,
    features: &[f64],
    reward: f64,
) -> Result<(), String> {
    update_rl_policy_with_features(pool, template, &BRAIN_RL_FEATURES, features, reward).await
}

async fn update_rl_policy_with_features(
    pool: &SqlitePool,
    template: &str,
    default_features: &[&str],
    features: &[f64],
    reward: f64,
) -> Result<(), String> {
    let mut policy = load_rl_policy_with_features(pool, template, default_features).await?;
    let dimension = policy.dimension();
    let features = align_feature_vector(features, dimension);
    for row in 0..dimension {
        for col in 0..dimension {
            policy.a_matrix[row][col] += features[row] * features[col];
        }
        policy.b_vector[row] += reward * features[row];
    }
    policy.observations += 1;
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT INTO brain_rl_policies
          (template, feature_names_json, a_matrix_json, b_vector_json, observations, alpha, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(template) DO UPDATE SET
          feature_names_json = excluded.feature_names_json,
          a_matrix_json = excluded.a_matrix_json,
          b_vector_json = excluded.b_vector_json,
          observations = excluded.observations,
          alpha = excluded.alpha,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(&policy.template)
    .bind(serde_json::to_string(&policy.feature_names).unwrap_or_else(|_| "[]".to_string()))
    .bind(serde_json::to_string(&policy.a_matrix).unwrap_or_else(|_| "[]".to_string()))
    .bind(serde_json::to_string(&policy.b_vector).unwrap_or_else(|_| "[]".to_string()))
    .bind(policy.observations)
    .bind(policy.alpha)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    invalidate_rl_policy(&policy.template);
    Ok(())
}

/// Adjust a feature vector to a target dimensionality without applying the
/// 27-dim BRAIN_RL_FEATURES normalization (which would corrupt smaller
/// policies). Truncates or zero-extends and clamps each component.
fn align_feature_vector(features: &[f64], dimension: usize) -> Vec<f64> {
    let mut values = vec![0.0; dimension];
    for (index, value) in features.iter().take(dimension).enumerate() {
        values[index] = if value.is_finite() {
            value.clamp(-5.0, 5.0)
        } else {
            0.0
        };
    }
    values
}

fn score_with_policy(
    node: &WorkGraphNode,
    features: &[f64],
    policy: &BrainRlPolicy,
) -> BrainRlScore {
    let features = normalized_feature_vector(features);
    let base_score = (node.weight as f64 / 10.0).clamp(0.0, 1.0);
    if policy.observations == 0 {
        return BrainRlScore {
            score: base_score,
            exploitation: base_score,
            exploration: 0.0,
            features,
        };
    }

    let inverse =
        invert_matrix(&policy.a_matrix).unwrap_or_else(|| identity_matrix(features.len()));
    let theta = mat_vec_mul(&inverse, &policy.b_vector);
    let exploitation = dot(&theta, &features);
    let variance = dot(&features, &mat_vec_mul(&inverse, &features))
        .max(0.0)
        .sqrt();
    let exploration = policy.alpha * variance;
    BrainRlScore {
        score: 0.25 * base_score + exploitation + exploration,
        exploitation,
        exploration,
        features,
    }
}

async fn upsert_rl_item_score(
    pool: &SqlitePool,
    template: &str,
    node: &WorkGraphNode,
    score: &BrainRlScore,
) -> Result<(), String> {
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT INTO brain_rl_item_scores
          (template, item_id, item_kind, score, exploitation, exploration, features_json, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(template, item_id) DO UPDATE SET
          item_kind = excluded.item_kind,
          score = excluded.score,
          exploitation = excluded.exploitation,
          exploration = excluded.exploration,
          features_json = excluded.features_json,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(template)
    .bind(&node.id)
    .bind(&node.kind)
    .bind(score.score)
    .bind(score.exploitation)
    .bind(score.exploration)
    .bind(feature_vector_to_json(&score.features).to_string())
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    invalidate_rl_item_scores(template);
    Ok(())
}

fn attach_rl_score(node: &mut WorkGraphNode, template: &str, score: &BrainRlScore) {
    let Some(object) = node.properties.as_object_mut() else {
        return;
    };
    object.insert(
        "brain_rl".to_string(),
        json!({
            "template": template,
            "score": score.score,
            "exploitation": score.exploitation,
            "exploration": score.exploration,
            "features": feature_vector_to_json(&score.features),
        }),
    );
}

fn brain_rl_features(node: &WorkGraphNode, graph: &WorkGraph) -> Vec<f64> {
    let degree = graph
        .edges
        .iter()
        .filter(|edge| edge.source == node.id || edge.target == node.id)
        .count();
    let neighbor_count = |kind: &str| -> f64 {
        graph
            .edges
            .iter()
            .filter_map(|edge| {
                if edge.source == node.id {
                    Some(edge.target.as_str())
                } else if edge.target == node.id {
                    Some(edge.source.as_str())
                } else {
                    None
                }
            })
            .filter(|id| {
                graph
                    .nodes
                    .iter()
                    .any(|candidate| candidate.id == *id && candidate.kind == kind)
            })
            .count()
            .min(6) as f64
            / 6.0
    };

    let status = node.status.as_deref().unwrap_or("");
    let priority = payload_string(node, "priority").unwrap_or_default();
    let due = payload_string(node, "due_date")
        .or_else(|| payload_string(node, "due_at"))
        .or_else(|| payload_string(node, "deadline"));
    let stale = node.kind == "attention_signal" && status == "stale_work";
    let updated_recent = node
        .updated_at
        .as_deref()
        .and_then(parse_date_prefix)
        .map(|date| date >= Utc::now().date_naive() - Duration::days(7))
        .unwrap_or(false);

    vec![
        1.0,
        (node.weight as f64 / 10.0).clamp(0.0, 1.0),
        bool_feature(node.kind == "deliverable"),
        bool_feature(node.kind == "task"),
        bool_feature(node.kind == "email_followup"),
        bool_feature(node.kind == "blocker"),
        bool_feature(node.kind == "open_loop"),
        bool_feature(node.kind == "attention_signal"),
        bool_feature(node.kind == "stakeholder" || node.kind == "email_participant"),
        bool_feature(node.kind == "calendar_event"),
        bool_feature(node.kind == "file" || node.kind == "trace_folder"),
        bool_feature(status == "blocked"),
        bool_feature(status == "overdue"),
        bool_feature(status == "open" || status == "todo" || status == "doing"),
        bool_feature(status == "current_focus" || payload_bool(node, "is_focused")),
        bool_feature(matches!(priority.as_str(), "high" | "urgent")),
        bool_feature(
            due.as_deref()
                .map(|date| date_is_within_days(date, 2) || date_is_before_today(date))
                .unwrap_or(false),
        ),
        bool_feature(stale),
        bool_feature(updated_recent),
        (degree.min(12) as f64) / 12.0,
        neighbor_count("open_loop"),
        neighbor_count("attention_signal"),
        neighbor_count("stakeholder") + neighbor_count("email_participant"),
        neighbor_count("email_thread") + neighbor_count("email_followup"),
        neighbor_count("meeting") + neighbor_count("meeting_action"),
        neighbor_count("file") + neighbor_count("trace_folder"),
        neighbor_count("calendar_event"),
    ]
}

fn default_reward_for_event(event_type: &str) -> f64 {
    match event_type {
        "useful" => 1.0,
        "wrong" => -1.0,
        "ignored" => -0.2,
        "shown" => 0.0,
        "clicked" | "opened" => 0.35,
        "completed_after_seen" => 1.0,
        "accepted_inference" | "accepted" => 0.8,
        "rejected_inference" | "rejected" => -0.8,
        "manual_link_created" => 0.7,
        "dismissed" => -0.6,
        "snoozed" => -0.3,
        _ => 0.0,
    }
}

fn normalize_template_name(template: Option<&str>) -> String {
    let normalized = template
        .unwrap_or("global")
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if normalized.is_empty() {
        "global".to_string()
    } else {
        normalized
    }
}

async fn load_item_features(
    pool: &SqlitePool,
    template: &str,
    item_id: &str,
) -> Result<Option<Vec<f64>>, String> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT features_json FROM brain_rl_item_scores WHERE template = ? AND item_id = ?",
    )
    .bind(template)
    .bind(item_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;
    Ok(raw
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|value| feature_vector_from_json(&value)))
}

fn bool_feature(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

fn default_feature_vector() -> Vec<f64> {
    let mut values = vec![0.0; BRAIN_RL_FEATURES.len()];
    values[0] = 1.0;
    values
}

fn normalized_feature_vector(features: &[f64]) -> Vec<f64> {
    let mut values = default_feature_vector();
    for (index, value) in features.iter().take(BRAIN_RL_FEATURES.len()).enumerate() {
        values[index] = if value.is_finite() {
            value.clamp(-5.0, 5.0)
        } else {
            0.0
        };
    }
    values
}

fn feature_vector_to_json(features: &[f64]) -> serde_json::Value {
    let features = normalized_feature_vector(features);
    let object = BRAIN_RL_FEATURES
        .iter()
        .zip(features)
        .map(|(name, value)| ((*name).to_string(), json!(value)))
        .collect::<Map<_, _>>();
    serde_json::Value::Object(object)
}

fn feature_vector_from_json(value: &serde_json::Value) -> Option<Vec<f64>> {
    if let Some(array) = value.as_array() {
        let values = array
            .iter()
            .map(|value| value.as_f64().unwrap_or(0.0))
            .collect::<Vec<_>>();
        return Some(normalized_feature_vector(&values));
    }
    let object = value.as_object()?;
    let values = BRAIN_RL_FEATURES
        .iter()
        .map(|name| {
            object
                .get(*name)
                .and_then(|value| value.as_f64())
                .unwrap_or(0.0)
        })
        .collect::<Vec<_>>();
    Some(normalized_feature_vector(&values))
}

fn parse_matrix(raw: &str, dimension: usize) -> Vec<Vec<f64>> {
    let Ok(matrix) = serde_json::from_str::<Vec<Vec<f64>>>(raw) else {
        return identity_matrix(dimension);
    };
    if matrix.len() != dimension || matrix.iter().any(|row| row.len() != dimension) {
        return identity_matrix(dimension);
    }
    matrix
}

fn parse_vector(raw: &str, dimension: usize) -> Vec<f64> {
    let Ok(vector) = serde_json::from_str::<Vec<f64>>(raw) else {
        return vec![0.0; dimension];
    };
    if vector.len() == dimension {
        vector
    } else {
        vec![0.0; dimension]
    }
}

fn identity_matrix(dimension: usize) -> Vec<Vec<f64>> {
    let mut matrix = vec![vec![0.0; dimension]; dimension];
    for index in 0..dimension {
        matrix[index][index] = 1.0;
    }
    matrix
}

pub(super) fn invert_matrix(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let dimension = matrix.len();
    if dimension == 0 || matrix.iter().any(|row| row.len() != dimension) {
        return None;
    }
    let mut augmented = vec![vec![0.0; dimension * 2]; dimension];
    for row in 0..dimension {
        for col in 0..dimension {
            augmented[row][col] = matrix[row][col];
        }
        augmented[row][dimension + row] = 1.0;
    }

    for pivot in 0..dimension {
        let mut pivot_row = pivot;
        for row in (pivot + 1)..dimension {
            if augmented[row][pivot].abs() > augmented[pivot_row][pivot].abs() {
                pivot_row = row;
            }
        }
        if augmented[pivot_row][pivot].abs() < 1e-9 {
            return None;
        }
        if pivot_row != pivot {
            augmented.swap(pivot, pivot_row);
        }
        let pivot_value = augmented[pivot][pivot];
        for col in 0..(dimension * 2) {
            augmented[pivot][col] /= pivot_value;
        }
        for row in 0..dimension {
            if row == pivot {
                continue;
            }
            let factor = augmented[row][pivot];
            for col in 0..(dimension * 2) {
                augmented[row][col] -= factor * augmented[pivot][col];
            }
        }
    }

    let mut inverse = vec![vec![0.0; dimension]; dimension];
    for row in 0..dimension {
        for col in 0..dimension {
            inverse[row][col] = augmented[row][dimension + col];
        }
    }
    Some(inverse)
}

pub(super) fn mat_vec_mul(matrix: &[Vec<f64>], vector: &[f64]) -> Vec<f64> {
    matrix
        .iter()
        .map(|row| {
            row.iter()
                .zip(vector)
                .map(|(left, right)| left * right)
                .sum()
        })
        .collect()
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

