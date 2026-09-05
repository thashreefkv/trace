//! Brain retrieval: hybrid BM25 + cosine + learned-blend scoring over the
//! projection, plus MMR diversification and per-node importance learning.
//!
//! `retrieve_brain_context` is the public entry — it pulls the brain graph
//! via `super::legacy::get_brain_graph`, scores every node with
//! `hybrid_score_node`, applies the learned `retrieval_blend` policy weights
//! (`load_retrieval_blend_weights`) and the learned per-node importance
//! multiplier (`load_node_importance_scores`), MMR-reranks the head, expands
//! a small neighborhood via `super::legacy::expand_neighborhood`, and
//! returns a `BrainContextResult`.
//!
//! All threshold consts (`RETRIEVAL_BLEND_*`, `NODE_IMPORTANCE_*`,
//! `MMR_DIVERSITY_LAMBDA`) live here. `BlendWeights` is the live struct the
//! `retrieval_blend` bandit writes to. Embedding plumbing
//! (`compute_query_embedding`, `load_node_embeddings`) lives here too —
//! it's the only retrieval-side consumer of `crate::gemini::embed_*`.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

use serde_json::json;
use sqlx::SqlitePool;

use crate::models::{
    BrainContextResult, BrainGraphFilters, BrainRetrieveInput, ScoredBrainNode, WorkGraph,
    WorkGraphEdge, WorkGraphNode,
};

use super::legacy::{expand_neighborhood, get_brain_graph, now_utc, truncate};
use super::rl::{invert_matrix, load_rl_policy_with_features, mat_vec_mul, rl_cache, RL_CACHE_TTL};

pub(super) const DEFAULT_RETRIEVAL_LIMIT: usize = 24;

pub async fn retrieve_brain_context(
    pool: &SqlitePool,
    path: &Path,
    input: BrainRetrieveInput,
) -> Result<BrainContextResult, String> {
    let limit = input.limit.unwrap_or(DEFAULT_RETRIEVAL_LIMIT).clamp(4, 80);
    let max_hops = input.max_hops.unwrap_or(2).clamp(1, 4);
    let graph = get_brain_graph(pool, path, BrainGraphFilters::default()).await?;
    let query_terms = query_terms(&input.query);
    let focus = input.focus_entity_id.as_deref();

    // Hybrid retrieval: try to compute a query embedding (best-effort; falls
    // back to BM25-only if no key or the call fails). Then bulk-load stored
    // entity embeddings for all nodes that have one.
    let query_embedding = compute_query_embedding(pool, &input.query).await;
    let node_embeddings = if query_embedding.is_some() {
        load_node_embeddings(pool, &graph.nodes).await
    } else {
        std::collections::HashMap::new()
    };

    // Look up the learned blend policy + per-entity importance scores. Both
    // fall back to deterministic defaults when no observations exist.
    let blend = load_retrieval_blend_weights(pool).await;
    let learned_importance =
        load_node_importance_scores(pool, &graph.nodes).await;

    let mut scored = graph
        .nodes
        .iter()
        .map(|node| {
            let mut breakdown = hybrid_score_node(
                node,
                &query_terms,
                focus,
                query_embedding.as_ref(),
                &node_embeddings,
                &blend,
            );
            if let Some(factor) = learned_importance.get(&node.entity_id).copied() {
                breakdown.learned_factor = factor;
                breakdown.blended_score *= factor;
            }
            (breakdown, node.clone())
        })
        .filter(|(breakdown, _)| breakdown.blended_score > 0.0 || focus.is_some())
        .collect::<Vec<_>>();
    scored.sort_by(|(left, left_node), (right, right_node)| {
        right
            .blended_score
            .partial_cmp(&left.blended_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right_node.weight.cmp(&left_node.weight))
            .then_with(|| left_node.label.cmp(&right_node.label))
    });

    // MMR diversity pass: re-rank the top 2*limit candidates so the final
    // selection isn't all from one cluster. Skip when there's nothing to
    // diversify or when the user has anchored on a focus entity (we don't
    // want to push the focus itself out of the top results).
    let mmr_limit = limit.max(1);
    let mut diversified: Vec<(ScoredBrainNode, WorkGraphNode)> = if scored.len() > mmr_limit {
        let candidate_pool = scored
            .iter()
            .take(mmr_limit.saturating_mul(2))
            .cloned()
            .collect::<Vec<_>>();
        mmr_rerank(candidate_pool, &node_embeddings, focus, mmr_limit)
    } else {
        scored.clone()
    };
    // Anything past the diversified head keeps its score order as a tail.
    if scored.len() > diversified.len() {
        let diversified_ids: HashSet<String> = diversified
            .iter()
            .map(|(_, node)| node.id.clone())
            .collect();
        for entry in scored.iter() {
            if !diversified_ids.contains(&entry.1.id) {
                diversified.push(entry.clone());
            }
        }
    }

    let mut selected_ids = diversified
        .iter()
        .take(limit.min(12))
        .map(|(_, node)| node.id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(focus) = focus {
        for node in &graph.nodes {
            if node.id == focus || node.entity_id == focus {
                selected_ids.insert(node.id.clone());
            }
        }
    }

    selected_ids = expand_neighborhood(&selected_ids, &graph.edges, max_hops, limit);

    let nodes = graph
        .nodes
        .iter()
        .filter(|node| selected_ids.contains(&node.id))
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let node_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let edges = graph
        .edges
        .iter()
        .filter(|edge| node_ids.contains(&edge.source) && node_ids.contains(&edge.target))
        .cloned()
        .collect::<Vec<_>>();

    let ranked: Vec<(ScoredBrainNode, WorkGraphNode)> = diversified
        .into_iter()
        .filter(|(_, node)| node_ids.contains(&node.id))
        .take(limit)
        .collect();
    let ranked_nodes = ranked.iter().map(|(_, node)| node.clone()).collect::<Vec<_>>();
    let scored_nodes = ranked.into_iter().map(|(breakdown, _)| breakdown).collect();
    let summary = brain_context_summary(&input.query, &ranked_nodes, &edges);

    Ok(BrainContextResult {
        query: input.query,
        summary: summary.clone(),
        graph: WorkGraph {
            generated_at: now_utc(),
            ai_context: summary,
            nodes,
            edges,
        },
        ranked_nodes,
        scored_nodes,
    })
}

pub async fn tool_retrieve_brain_context(
    pool: &SqlitePool,
    path: &Path,
    query: &str,
    focus_entity_id: Option<String>,
    max_hops: Option<usize>,
    limit: Option<usize>,
) -> serde_json::Value {
    match retrieve_brain_context(
        pool,
        path,
        BrainRetrieveInput {
            query: query.to_string(),
            focus_entity_id,
            max_hops,
            limit,
        },
    )
    .await
    {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}
fn brain_context_summary(query: &str, nodes: &[WorkGraphNode], edges: &[WorkGraphEdge]) -> String {
    let mut lines = vec![format!(
        "Brain context for '{}': {} relevant node(s), {} relation(s).",
        query,
        nodes.len(),
        edges.len()
    )];
    for node in nodes.iter().take(14) {
        lines.push(format!(
            "- [{} {}] {}{}",
            node.kind,
            node.status.as_deref().unwrap_or(""),
            node.label,
            node.subtitle
                .as_ref()
                .map(|summary| format!(" | {}", truncate(summary, 220)))
                .unwrap_or_default()
        ));
    }
    for edge in edges.iter().take(14) {
        lines.push(format!(
            "- relation {}: {} -> {} ({})",
            edge.kind, edge.source, edge.target, edge.label
        ));
    }
    lines.join("\n")
}

pub(super) fn graph_ai_context(nodes: &[WorkGraphNode], edges: &[WorkGraphEdge]) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for node in nodes {
        *counts.entry(node.kind.clone()).or_default() += 1;
    }
    let mut lines = vec![format!(
        "Trace brain graph: {} node(s), {} relation(s).",
        nodes.len(),
        edges.len()
    )];
    lines.push(
        counts
            .iter()
            .map(|(kind, count)| format!("{kind}: {count}"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    for node in nodes.iter().take(18) {
        lines.push(format!(
            "- [{}] {}{}{}",
            node.kind,
            node.label,
            node.status
                .as_ref()
                .map(|status| format!(" ({status})"))
                .unwrap_or_default(),
            node.subtitle
                .as_ref()
                .map(|summary| format!(" | {}", truncate(summary, 180)))
                .unwrap_or_default()
        ));
    }
    lines.join("\n")
}

fn score_node(node: &WorkGraphNode, terms: &[String], focus_entity_id: Option<&str>) -> f64 {
    let mut score = 0.0;
    if let Some(focus) = focus_entity_id {
        if node.id == focus || node.entity_id == focus {
            score += 25.0;
        }
    }
    let haystack = [
        node.id.as_str(),
        node.kind.as_str(),
        node.label.as_str(),
        node.subtitle.as_deref().unwrap_or(""),
        node.status.as_deref().unwrap_or(""),
        node.context.as_str(),
    ]
    .join(" ")
    .to_ascii_lowercase();
    for term in terms {
        if node.label.to_ascii_lowercase().contains(term) {
            score += 6.0;
        }
        if haystack.contains(term) {
            score += 2.0;
        }
    }
    if terms
        .iter()
        .any(|term| term == "blocked" || term == "blocker")
        && (node.kind == "blocker" || haystack.contains("blocked by"))
    {
        score += 8.0;
    }
    score + (node.weight as f64 * 0.15)
}

/// Hybrid relevance: BM25-style keyword score + semantic cosine + recency.
/// Each signal is normalized to [0, 1] then weighted, so the function returns
/// roughly [0, ~2]. Falls back gracefully to BM25-only when no query embedding
/// is available, which is the common case before the embedding worker has
/// caught up.
///
/// Returns a per-node breakdown so callers (retrieve_brain_context, the
/// `retrieval_blend` learning loop, the "Why this answer?" UI) can see the
/// individual signal contributions, not just the final blended score.
fn hybrid_score_node(
    node: &WorkGraphNode,
    terms: &[String],
    focus_entity_id: Option<&str>,
    query_embedding: Option<&(Vec<f32>, f32)>,
    node_embeddings: &std::collections::HashMap<(String, String), (Vec<f32>, f32)>,
    blend: &BlendWeights,
) -> ScoredBrainNode {
    let bm25 = score_node(node, terms, focus_entity_id);
    // Map BM25 score (unbounded above) to [0, 1) so it can blend with cosine.
    let bm25_norm = bm25 / (bm25 + 10.0);

    let cosine = query_embedding
        .and_then(|(q_vec, q_norm)| {
            let key = (node.kind.clone(), node.entity_id.clone());
            node_embeddings.get(&key).map(|(v, n)| {
                crate::entity_embeddings::cosine(q_vec, *q_norm, v, *n) as f64
            })
        })
        .map(|c| c.clamp(0.0, 1.0))
        .unwrap_or(0.0);

    let recency = recency_multiplier(node.updated_at.as_deref());
    let node_weight_norm = (node.weight as f64 / 10.0).clamp(0.0, 1.0);
    let focus_proximity = match focus_entity_id {
        Some(focus) if node.entity_id == focus || node.id == focus => 1.0,
        _ => 0.0,
    };

    // Linear blend across signals. Default weights bias toward BM25 + cosine
    // (the established `0.6 / 0.4` baseline) with small contributions from
    // node weight and focus proximity. The retrieval_blend bandit can shift
    // these once it has observations.
    let blended_raw = blend.bm25 * bm25_norm
        + blend.cosine * cosine
        + blend.node_weight * node_weight_norm
        + blend.focus_proximity * focus_proximity;
    // Scale by recency so freshness multiplies the blend (preserves the
    // pre-learning behavior). recency ∈ [1.0, 1.5].
    let blended_score = blended_raw * recency * 10.0;

    ScoredBrainNode {
        node_id: node.id.clone(),
        entity_id: node.entity_id.clone(),
        kind: node.kind.clone(),
        bm25_norm,
        cosine,
        recency_multiplier: recency,
        node_weight_norm,
        focus_proximity,
        learned_factor: 1.0,
        blended_score,
    }
}

/// Linear blend weights across retrieval signals. Defaults mirror the
/// pre-learning hardcoded constants; the retrieval_blend bandit replaces
/// these once warmed up.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlendWeights {
    pub bm25: f64,
    pub cosine: f64,
    pub node_weight: f64,
    pub focus_proximity: f64,
}

impl BlendWeights {
    pub fn baseline() -> Self {
        Self {
            bm25: 0.6,
            cosine: 0.4,
            node_weight: 0.0,
            focus_proximity: 0.0,
        }
    }
}


/// Feature schema for the `retrieval_blend` policy. Order matters — the
/// stored A/b matrices index features positionally. Never reorder; only
/// append.
pub(super) const RETRIEVAL_BLEND_FEATURES: [&str; 5] = [
    "bm25_norm",
    "cosine",
    "recency_factor",
    "node_weight_norm",
    "focus_proximity",
];
pub(super) const RETRIEVAL_BLEND_TEMPLATE: &str = "retrieval_blend";
pub(super) const NODE_IMPORTANCE_TEMPLATE: &str = "node_importance";

/// Cold-start guard. Blend weights stay at baseline until enough events
/// have been observed. Past `RETRIEVAL_BLEND_WARMUP` we linearly mix in
/// the learned weights, fully replacing baseline by
/// `RETRIEVAL_BLEND_FULLY_LEARNED` events.
const RETRIEVAL_BLEND_WARMUP: i64 = 20;
const RETRIEVAL_BLEND_FULLY_LEARNED: i64 = 50;

/// Influence cap for learned per-entity importance. The bandit can at most
/// halve or double a node's effective weight so a few rogue clicks can't
/// dominate retrieval.
const NODE_IMPORTANCE_FLOOR: f64 = 0.5;
const NODE_IMPORTANCE_CEIL: f64 = 2.0;

/// MMR diversity weight. Higher = more aggressive diversification. The
/// term `score(c) - MMR_DIVERSITY_LAMBDA * max_cos(c, selected)` is what
/// drives the iterative pick.
const MMR_DIVERSITY_LAMBDA: f64 = 0.3;

pub(super) async fn load_retrieval_blend_weights(pool: &SqlitePool) -> BlendWeights {
    let policy = match load_rl_policy_with_features(
        pool,
        RETRIEVAL_BLEND_TEMPLATE,
        &RETRIEVAL_BLEND_FEATURES,
    )
    .await
    {
        Ok(p) => p,
        Err(_) => return BlendWeights::baseline(),
    };

    let baseline = BlendWeights::baseline();
    if policy.observations < RETRIEVAL_BLEND_WARMUP {
        return baseline;
    }

    let inverse = match invert_matrix(&policy.a_matrix) {
        Some(matrix) => matrix,
        None => return baseline,
    };
    let theta = mat_vec_mul(&inverse, &policy.b_vector);

    // Clamp each learned weight to a sensible range so a runaway update
    // can't flip the sign of a signal. Then mix with baseline based on
    // how much data we've seen.
    let learned = BlendWeights {
        bm25: theta.get(0).copied().unwrap_or(0.6).clamp(0.0, 1.0),
        cosine: theta.get(1).copied().unwrap_or(0.4).clamp(0.0, 1.0),
        node_weight: theta.get(3).copied().unwrap_or(0.0).clamp(0.0, 0.6),
        focus_proximity: theta.get(4).copied().unwrap_or(0.0).clamp(0.0, 0.6),
    };

    let progress = ((policy.observations - RETRIEVAL_BLEND_WARMUP) as f64
        / (RETRIEVAL_BLEND_FULLY_LEARNED - RETRIEVAL_BLEND_WARMUP) as f64)
        .clamp(0.0, 1.0);

    BlendWeights {
        bm25: baseline.bm25 * (1.0 - progress) + learned.bm25 * progress,
        cosine: baseline.cosine * (1.0 - progress) + learned.cosine * progress,
        node_weight: baseline.node_weight * (1.0 - progress) + learned.node_weight * progress,
        focus_proximity: baseline.focus_proximity * (1.0 - progress)
            + learned.focus_proximity * progress,
    }
}

/// Bulk-fetch per-entity learned importance multipliers for every node in
/// the graph. Returns a map keyed by `entity_id` (matching how the rest
/// of the retrieval pipeline keys nodes). Entities without a row get the
/// neutral `1.0` factor at lookup time.
async fn load_node_importance_scores(
    pool: &SqlitePool,
    nodes: &[WorkGraphNode],
) -> std::collections::HashMap<String, f64> {
    let mut out = std::collections::HashMap::new();
    if nodes.is_empty() {
        return out;
    }

    // TTL cache snapshots the entire `template='node_importance'` row set, so
    // the per-call cost drops to an in-memory hash filter. Invalidated by
    // `update_rl_item_score`.
    let snapshot = node_importance_snapshot(pool).await;

    for node in nodes {
        if let Some(raw) = snapshot.get(node.id.as_str()) {
            let clamped = raw.clamp(NODE_IMPORTANCE_FLOOR, NODE_IMPORTANCE_CEIL);
            out.insert(node.entity_id.clone(), clamped);
        }
    }
    out
}

async fn node_importance_snapshot(
    pool: &SqlitePool,
) -> std::collections::HashMap<String, f64> {
    let now = std::time::Instant::now();
    {
        if let Ok(guard) = rl_cache().lock() {
            if let Some((cached_at, scores)) = guard.item_scores.get(NODE_IMPORTANCE_TEMPLATE) {
                if now.duration_since(*cached_at) < RL_CACHE_TTL {
                    return scores.clone();
                }
            }
        }
    }
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT item_id, score FROM brain_rl_item_scores WHERE template = ?",
    )
    .bind(NODE_IMPORTANCE_TEMPLATE)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let snapshot: std::collections::HashMap<String, f64> = rows.into_iter().collect();
    if let Ok(mut guard) = rl_cache().lock() {
        guard.item_scores.insert(
            NODE_IMPORTANCE_TEMPLATE.to_string(),
            (std::time::Instant::now(), snapshot.clone()),
        );
    }
    snapshot
}

/// MMR re-rank: iteratively pick the candidate that maximizes
/// `score - λ * max_cosine(candidate, already_selected)`. Falls back to
/// score-only ordering when embeddings are missing for one side of the
/// comparison.
fn mmr_rerank(
    mut candidates: Vec<(ScoredBrainNode, WorkGraphNode)>,
    node_embeddings: &std::collections::HashMap<(String, String), (Vec<f32>, f32)>,
    focus_entity_id: Option<&str>,
    limit: usize,
) -> Vec<(ScoredBrainNode, WorkGraphNode)> {
    let mut selected: Vec<(ScoredBrainNode, WorkGraphNode)> = Vec::with_capacity(limit);
    while !candidates.is_empty() && selected.len() < limit {
        let mut best_idx = 0usize;
        let mut best_score = f64::MIN;
        for (idx, (breakdown, node)) in candidates.iter().enumerate() {
            // Anchor entity is never diversity-penalized — it's the pin.
            let is_anchor = focus_entity_id
                .map(|focus| node.entity_id == focus || node.id == focus)
                .unwrap_or(false);
            let mut max_sim: f64 = 0.0;
            if !is_anchor {
                if let Some((vec_c, norm_c)) =
                    node_embeddings.get(&(node.kind.clone(), node.entity_id.clone()))
                {
                    for (_, sel_node) in &selected {
                        if let Some((vec_s, norm_s)) = node_embeddings
                            .get(&(sel_node.kind.clone(), sel_node.entity_id.clone()))
                        {
                            let sim = crate::entity_embeddings::cosine(
                                vec_c, *norm_c, vec_s, *norm_s,
                            ) as f64;
                            if sim > max_sim {
                                max_sim = sim;
                            }
                        }
                    }
                }
            }
            let mmr_score = breakdown.blended_score - MMR_DIVERSITY_LAMBDA * max_sim;
            if mmr_score > best_score {
                best_score = mmr_score;
                best_idx = idx;
            }
        }
        selected.push(candidates.remove(best_idx));
    }
    selected
}

/// Recency multiplier ∈ [1.0, 1.5]. Items touched in the last 24h get the
/// full boost; older items decay toward 1.0 with a 60-day half-life.
fn recency_multiplier(updated_at: Option<&str>) -> f64 {
    let Some(updated_at) = updated_at else {
        return 1.0;
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(updated_at) else {
        return 1.0;
    };
    let now = chrono::Utc::now();
    let age_days = (now - parsed.with_timezone(&chrono::Utc))
        .num_seconds()
        .max(0) as f64
        / 86_400.0;
    if age_days < 1.0 {
        return 1.5;
    }
    // 0.5 * 0.5^(days/60); maxes at 0.5 just past 24h, asymptotic to 0.
    let decay = 0.5 * 0.5_f64.powf((age_days - 1.0) / 60.0);
    1.0 + decay
}

async fn compute_query_embedding(
    pool: &sqlx::SqlitePool,
    query: &str,
) -> Option<(Vec<f32>, f32)> {
    if query.trim().is_empty() {
        return None;
    }
    let api_key = crate::runtime::gemini_api_key()?;
    match crate::gemini::embed_retrieval_query(Some(pool), &api_key, query).await {
        Ok(vec) => {
            let norm = crate::entity_embeddings::vector_norm(&vec.values);
            if norm == 0.0 {
                None
            } else {
                Some((vec.values, norm))
            }
        }
        Err(error) => {
            eprintln!("[brain] query embedding failed: {error}");
            None
        }
    }
}

async fn load_node_embeddings(
    pool: &SqlitePool,
    nodes: &[WorkGraphNode],
) -> std::collections::HashMap<(String, String), (Vec<f32>, f32)> {
    let mut keys: Vec<(String, String)> = nodes
        .iter()
        .filter(|n| !n.entity_id.is_empty())
        .map(|n| (n.kind.clone(), n.entity_id.clone()))
        .collect();
    keys.sort();
    keys.dedup();
    crate::entity_embeddings::load_embeddings_for(pool, &keys)
        .await
        .unwrap_or_default()
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_alphanumeric())
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| term.len() > 2)
        .collect()
}
