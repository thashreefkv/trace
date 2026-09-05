use std::collections::{BTreeMap, BTreeSet};

use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{
        ListMemoryFilters, MemoryRecord, MemoryRetrievalDiagnostics, MemoryRetrievalResult,
        MemoryStatus, RetrieveMemoryInput, ScoredMemory,
    },
};

use super::super::{json_string, now_utc};
use super::{get_memory_settings, list_memories};

pub async fn retrieve_memories(
    pool: &SqlitePool,
    input: RetrieveMemoryInput,
) -> Result<MemoryRetrievalResult, String> {
    retrieve_memories_with_key(pool, input, None).await
}

pub async fn retrieve_memories_with_key(
    pool: &SqlitePool,
    input: RetrieveMemoryInput,
    api_key: Option<&str>,
) -> Result<MemoryRetrievalResult, String> {
    let settings = get_memory_settings(pool).await?;
    if !settings.enabled {
        return Ok(MemoryRetrievalResult {
            context: "Memory is disabled.".to_string(),
            memories: Vec::new(),
            scored: Vec::new(),
            diagnostics: MemoryRetrievalDiagnostics::default(),
            retrieval_id: None,
        });
    }

    let limit = input.limit.unwrap_or(settings.retrieval_limit).clamp(1, 40) as usize;
    let query = input.query.trim().to_string();
    let now = now_utc();
    let mut diagnostics = MemoryRetrievalDiagnostics::default();

    // 1. Lexical / FTS candidates
    let lexical_filters = ListMemoryFilters {
        kind: None,
        status: Some(MemoryStatus::Active),
        query: if query.is_empty() {
            None
        } else {
            Some(query.clone())
        },
        include_archived: false,
    };
    let mut candidates = list_memories(pool, lexical_filters).await?;
    diagnostics.lexical_used = !query.is_empty();

    // 2. Procedural pin set — always-relevant procedures + pinned semantic facts.
    let include_pins = input.include_pinned.unwrap_or(true);
    if include_pins {
        let pinned = list_memories(
            pool,
            ListMemoryFilters {
                status: Some(MemoryStatus::Active),
                ..Default::default()
            },
        )
        .await?
        .into_iter()
        .filter(|memory| memory.pinned || memory.source == "system" || memory.kind == "procedural")
        .collect::<Vec<_>>();
        diagnostics.procedural_pin_count = pinned.len() as i64;
        for memory in pinned {
            if !candidates.iter().any(|existing| existing.id == memory.id) {
                candidates.push(memory);
            }
        }
    }

    // 3. Drop expired and deleted memories before scoring.
    candidates.retain(|memory| !memory_expired(memory, &now));

    // 4. Optional semantic retrieval via Gemini embeddings.
    let mut semantic_scores: BTreeMap<String, f64> = BTreeMap::new();
    if let Some(key) = api_key {
        if !query.is_empty() {
            match crate::gemini::embed_retrieval_query(Some(pool), key, &query).await {
                Ok(query_vector) => {
                    diagnostics.semantic_used = true;
                    diagnostics.embedding_model = Some(query_vector.model.clone());
                    let stored = list_memory_embeddings(pool).await?;
                    let normalized_query = normalize_vector(&query_vector.values);
                    for embedding in stored {
                        if let Some(score) = cosine_score(&embedding.normalized, &normalized_query)
                        {
                            semantic_scores.insert(embedding.memory_id, score);
                        }
                    }
                }
                Err(error) => {
                    diagnostics.embedding_error = Some(error);
                }
            }
        }
    }

    // 5. Kind filter (if requested).
    if !input.kinds.is_empty() {
        let allowed = input
            .kinds
            .iter()
            .map(|kind| kind.as_str())
            .collect::<BTreeSet<_>>();
        candidates.retain(|memory| allowed.contains(memory.kind.as_str()));
    }

    // 6. Score & rank.
    let query_tokens = tokenize_for_memory(&query);
    let task_type = input.task_type.as_deref();
    let mut scored: Vec<ScoredMemory> = candidates
        .into_iter()
        .map(|memory| {
            let semantic_score = semantic_scores.get(&memory.id).copied().unwrap_or(0.0);
            let lexical_score = lexical_score(&memory, &query_tokens);
            let recency_score = recency_score(&memory.updated_at, &now);
            let procedural_pin =
                memory.pinned || memory.kind == "procedural" || memory.source == "system";
            let score = composite_score(
                &memory,
                semantic_score,
                lexical_score,
                recency_score,
                procedural_pin,
                task_type,
            );
            ScoredMemory {
                memory,
                score,
                semantic_score,
                lexical_score,
                recency_score,
                procedural_pin,
            }
        })
        .collect();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.memory.updated_at.cmp(&left.memory.updated_at))
    });

    if scored.len() > limit {
        scored.truncate(limit);
    }

    let memories: Vec<MemoryRecord> = scored.iter().map(|item| item.memory.clone()).collect();

    // 7. Bump retrieval counts so usage feeds future ranking.
    for memory in &memories {
        sqlx::query(
            "UPDATE memories SET retrieval_count = retrieval_count + 1, last_retrieved_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(&memory.id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    let context = format_memories_for_prompt(&memories);
    let retrieval_id = log_retrieval(
        pool,
        &query,
        &scored,
        input.source_kind.as_deref(),
        input.source_id.as_deref(),
    )
    .await?;

    Ok(MemoryRetrievalResult {
        context,
        memories,
        scored,
        diagnostics,
        retrieval_id: Some(retrieval_id),
    })
}
fn tokenize_for_memory(query: &str) -> BTreeSet<String> {
    query
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|token| token.len() > 2)
        .map(str::to_string)
        .collect()
}

fn lexical_score(memory: &MemoryRecord, tokens: &BTreeSet<String>) -> f64 {
    if tokens.is_empty() {
        return 0.0;
    }
    let haystack =
        format!("{} {} {}", memory.title, memory.body, memory.tags.join(" ")).to_lowercase();
    let hits = tokens
        .iter()
        .filter(|token| haystack.contains(token.as_str()))
        .count() as f64;
    (hits / tokens.len() as f64).clamp(0.0, 1.0)
}

fn recency_score(updated_at: &str, now: &str) -> f64 {
    use chrono::DateTime;
    let updated = DateTime::parse_from_rfc3339(updated_at);
    let current = DateTime::parse_from_rfc3339(now);
    match (updated, current) {
        (Ok(updated), Ok(now)) => {
            let age_days = (now - updated).num_days().max(0) as f64;
            // Half-life ~30 days: 1.0 fresh, 0.5 at 30d, 0.25 at 60d.
            let value = 0.5_f64.powf(age_days / 30.0);
            value.clamp(0.0, 1.0)
        }
        _ => 0.5,
    }
}

fn composite_score(
    memory: &MemoryRecord,
    semantic: f64,
    lexical: f64,
    recency: f64,
    procedural_pin: bool,
    task_type: Option<&str>,
) -> f64 {
    let confidence = memory.confidence.clamp(0.0, 1.0);
    let explicitness = match memory.source.as_str() {
        "manual" => 1.0,
        "system" => 0.85,
        "consolidated" => 0.6,
        _ => 0.5,
    };
    let usage = (memory.retrieval_count as f64 / 25.0).clamp(0.0, 1.0)
        + (memory.success_count as f64 / 10.0).clamp(0.0, 1.0);
    let usage = (usage / 2.0).clamp(0.0, 1.0);

    let staleness = if memory.contradiction_count > 0 {
        (memory.contradiction_count as f64 / 5.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sensitivity_penalty = match memory.sensitivity.as_str() {
        "sensitive" => 0.30,
        "pii" => 0.18,
        _ => 0.0,
    };
    let task_boost = match (task_type, memory.kind.as_str()) {
        (Some("instruction"), "procedural") => 0.06,
        (Some("planning"), "semantic") => 0.04,
        (Some("recall"), "episodic") => 0.04,
        _ => 0.0,
    };
    let pin_boost = if procedural_pin { 0.05 } else { 0.0 };
    let importance = memory.importance.clamp(0.0, 1.0);

    let base = 0.42 * semantic
        + 0.18 * lexical
        + 0.14 * confidence
        + 0.10 * recency
        + 0.08 * explicitness
        + 0.05 * usage
        + 0.05 * importance;
    (base + task_boost + pin_boost - 0.20 * staleness - sensitivity_penalty).clamp(-0.5, 1.5)
}

fn format_memories_for_prompt(memories: &[MemoryRecord]) -> String {
    if memories.is_empty() {
        return "No relevant durable memory was retrieved.".to_string();
    }

    let mut lines = vec![
        "Relevant memory, do not treat as absolute truth. Prefer manual/system memories over consolidated ones when records conflict.".to_string(),
    ];
    for memory in memories {
        let tags = if memory.tags.is_empty() {
            String::new()
        } else {
            format!(" tags={}", memory.tags.join(","))
        };
        let source_id = memory
            .source_id
            .as_deref()
            .map(|id| format!(" source_id={id}"))
            .unwrap_or_default();
        let sensitivity = if memory.sensitivity != "normal" {
            format!(" sensitivity={}", memory.sensitivity)
        } else {
            String::new()
        };
        let pinned = if memory.pinned { " pinned=true" } else { "" };
        lines.push(format!(
            "- [{kind}:{id} confidence={confidence:.2} source={source}{source_id}{tags}{sensitivity}{pinned}] {title}: {body}",
            kind = memory.kind,
            id = memory.id,
            confidence = memory.confidence,
            source = memory.source,
            source_id = source_id,
            tags = tags,
            sensitivity = sensitivity,
            pinned = pinned,
            title = memory.title,
            body = memory.body
        ));
    }
    lines.join("\n")
}

fn memory_expired(memory: &MemoryRecord, now: &str) -> bool {
    use chrono::DateTime;
    let Some(expires_at) = memory.expires_at.as_deref() else {
        return false;
    };
    match (
        DateTime::parse_from_rfc3339(expires_at),
        DateTime::parse_from_rfc3339(now),
    ) {
        (Ok(expires), Ok(current)) => expires <= current,
        _ => false,
    }
}


#[derive(Debug, Clone)]
struct StoredMemoryEmbedding {
    memory_id: String,
    normalized: Vec<f32>,
}

async fn list_memory_embeddings(pool: &SqlitePool) -> Result<Vec<StoredMemoryEmbedding>, String> {
    let rows: Vec<(String, String, f64)> = sqlx::query_as(
        r#"
        SELECT e.memory_id, e.vector_json, e.norm
        FROM memory_embeddings e
        JOIN memories m ON m.id = e.memory_id
        WHERE m.status = 'active'
          AND m.deleted_at IS NULL
          AND e.model = ?
          AND e.dim = ?
        "#,
    )
    .bind(crate::gemini::EMBEDDING_MODEL)
    .bind(crate::gemini::EMBEDDING_OUTPUT_DIMENSIONALITY as i64)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut out = Vec::with_capacity(rows.len());
    for (memory_id, vector_json, norm) in rows {
        let values: Vec<f32> = match serde_json::from_str(&vector_json) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if values.is_empty() || norm.abs() < f64::EPSILON {
            continue;
        }
        let inv_norm = 1.0_f32 / (norm as f32);
        let normalized = values.iter().map(|v| v * inv_norm).collect::<Vec<_>>();
        out.push(StoredMemoryEmbedding {
            memory_id,
            normalized,
        });
    }
    Ok(out)
}

fn normalize_vector(values: &[f32]) -> Vec<f32> {
    let norm = vector_norm(values);
    if norm.abs() < f32::EPSILON {
        return values.to_vec();
    }
    let inv = 1.0_f32 / norm;
    values.iter().map(|v| v * inv).collect()
}

fn vector_norm(values: &[f32]) -> f32 {
    values.iter().map(|v| v * v).sum::<f32>().sqrt()
}

fn cosine_score(stored: &[f32], query: &[f32]) -> Option<f64> {
    if stored.len() != query.len() || stored.is_empty() {
        return None;
    }
    let dot: f32 = stored.iter().zip(query).map(|(a, b)| a * b).sum();
    let value = dot.clamp(-1.0, 1.0);
    Some(((value as f64) + 1.0) / 2.0)
}

pub async fn invalidate_memory_embedding(pool: &SqlitePool, memory_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM memory_embeddings WHERE memory_id = ?")
        .bind(memory_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn upsert_memory_embedding(
    pool: &SqlitePool,
    memory_id: &str,
    text: &str,
    api_key: &str,
) -> Result<(), String> {
    let fingerprint = embedding_fingerprint(text);
    let already: Option<(String, String, i64)> =
        sqlx::query_as("SELECT fingerprint, model, dim FROM memory_embeddings WHERE memory_id = ?")
            .bind(memory_id)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?;
    if matches!(
        already,
        Some((ref fp, ref model, dim))
            if fp == &fingerprint
                && model == crate::gemini::EMBEDDING_MODEL
                && dim == crate::gemini::EMBEDDING_OUTPUT_DIMENSIONALITY as i64
    ) {
        return Ok(());
    }

    let vector = crate::gemini::embed_retrieval_document(Some(pool), api_key, text).await?;
    let norm = vector_norm(&vector.values) as f64;
    let vector_json = json_string(&vector.values)?;
    let now = now_utc();

    sqlx::query(
        r#"
        INSERT INTO memory_embeddings (memory_id, model, dim, vector_json, norm, fingerprint, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(memory_id) DO UPDATE SET
            model = excluded.model,
            dim = excluded.dim,
            vector_json = excluded.vector_json,
            norm = excluded.norm,
            fingerprint = excluded.fingerprint,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(memory_id)
    .bind(&vector.model)
    .bind(vector.values.len() as i64)
    .bind(&vector_json)
    .bind(norm)
    .bind(&fingerprint)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

fn embedding_fingerprint(text: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    text.hash(&mut hasher);
    format!("h{:016x}", hasher.finish())
}

pub async fn ensure_active_memory_embeddings(
    pool: &SqlitePool,
    api_key: &str,
    limit: i64,
) -> Result<i64, String> {
    let limit = limit.clamp(1, 200);
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT m.id, m.title, m.body
        FROM memories m
        LEFT JOIN memory_embeddings e
          ON e.memory_id = m.id
         AND e.model = ?
         AND e.dim = ?
        WHERE m.status = 'active' AND m.deleted_at IS NULL AND e.memory_id IS NULL
        ORDER BY m.importance DESC, m.updated_at DESC
        LIMIT ?
        "#,
    )
    .bind(crate::gemini::EMBEDDING_MODEL)
    .bind(crate::gemini::EMBEDDING_OUTPUT_DIMENSIONALITY as i64)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut count = 0;
    for (id, title, body) in rows {
        let text = format!("{title}\n{body}");
        if upsert_memory_embedding(pool, &id, &text, api_key)
            .await
            .is_ok()
        {
            count += 1;
        }
    }
    Ok(count)
}

async fn log_retrieval(
    pool: &SqlitePool,
    query: &str,
    scored: &[ScoredMemory],
    source_kind: Option<&str>,
    source_id: Option<&str>,
) -> Result<String, String> {
    let id = Ulid::new().to_string();
    let now = now_utc();
    let memory_ids: Vec<&str> = scored.iter().map(|item| item.memory.id.as_str()).collect();
    let memory_ids_json = json_string(&memory_ids)?;
    let mut score_map = serde_json::Map::new();
    for item in scored {
        score_map.insert(
            item.memory.id.clone(),
            serde_json::json!({
                "score": item.score,
                "semantic": item.semantic_score,
                "lexical": item.lexical_score,
                "recency": item.recency_score,
                "pin": item.procedural_pin,
            }),
        );
    }
    let scores_json = serde_json::Value::Object(score_map).to_string();

    sqlx::query(
        r#"
        INSERT INTO memory_retrievals (id, query, memory_ids_json, scores_json, context_kind, source_kind, source_id, created_at)
        VALUES (?, ?, ?, ?, 'auto', ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(query)
    .bind(&memory_ids_json)
    .bind(&scores_json)
    .bind(source_kind)
    .bind(source_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(id)
}

