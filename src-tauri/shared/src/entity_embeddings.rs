//! Semantic embeddings for retrieval-relevant entities.
//!
//! Plug-in for hybrid retrieval: each tracked entity (deliverable, initiative,
//! capture, stakeholder, meeting, memory, gmail thread) has a single
//! Gemini embedding vector stored in `entity_embeddings`. The brain scorer
//! blends cosine similarity into its existing BM25 + recency signals so
//! semantically related items rank even when they don't share keywords.
//!
//! Lifecycle:
//!   1. On any mutation, callers `enqueue` the entity for embedding (cheap —
//!      just an `INSERT OR REPLACE` into a pending table).
//!   2. The opportunistic worker (started in `bin/`/`commands/embedding.rs`)
//!      pulls pending rows in batches, hashes the text, skips if the hash
//!      hasn't changed, otherwise calls Gemini and writes the vector.
//!   3. Worker is rate-limit aware and resumable: closing the app pauses
//!      progress; reopening picks up where it left off.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::gemini;

pub const EMBEDDING_MODEL: &str = gemini::EMBEDDING_MODEL;
pub const EMBEDDING_DIM: i64 = gemini::EMBEDDING_OUTPUT_DIMENSIONALITY as i64;
pub const DEFAULT_BATCH_SIZE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityKind {
    Initiative,
    Deliverable,
    Capture,
    Stakeholder,
    Meeting,
    Memory,
    GmailThread,
    ReasoningSourceUnit,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntityKind::Initiative => "initiative",
            EntityKind::Deliverable => "deliverable",
            EntityKind::Capture => "capture",
            EntityKind::Stakeholder => "stakeholder",
            EntityKind::Meeting => "meeting",
            EntityKind::Memory => "memory",
            EntityKind::GmailThread => "gmail_thread",
            EntityKind::ReasoningSourceUnit => "reasoning_source_unit",
        }
    }

    pub fn all() -> [EntityKind; 8] {
        [
            EntityKind::Initiative,
            EntityKind::Deliverable,
            EntityKind::Capture,
            EntityKind::Stakeholder,
            EntityKind::Meeting,
            EntityKind::Memory,
            EntityKind::GmailThread,
            EntityKind::ReasoningSourceUnit,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProgress {
    pub model: String,
    pub dim: i64,
    pub total: i64,
    pub embedded: i64,
    pub pending: i64,
    pub stale: i64,
}

#[derive(Debug, Clone)]
pub struct PendingRow {
    pub entity_kind: String,
    pub entity_id: String,
    pub content_text: String,
    pub content_hash: String,
    pub attempts: i64,
}

/// Fingerprint the canonical text for an entity — cheap stable hash so we
/// can detect when re-embedding is needed.
pub fn hash_content(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// Build the text representation Gemini embeds for a given entity. Kept
/// short and structured so two similar entities (same title) embed similarly.
pub async fn build_text(
    pool: &SqlitePool,
    kind: EntityKind,
    entity_id: &str,
) -> Result<Option<String>, String> {
    let text = match kind {
        EntityKind::Initiative => sqlx::query_as::<_, (String, Option<String>, String)>(
            "SELECT title, COALESCE(description, ''), COALESCE(status, '') FROM initiatives WHERE id = ?",
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("read initiative: {e}"))?
        .map(|(title, description, status)| {
            format!(
                "Initiative: {title}\nStatus: {status}\n{description}",
                description = description.unwrap_or_default()
            )
        }),
        EntityKind::Deliverable => sqlx::query_as::<_, (
            String,
            Option<String>,
            String,
            Option<String>,
        )>(
            "SELECT title, COALESCE(claim, ''), COALESCE(state, ''), COALESCE(friction_note, '')
               FROM deliverables WHERE id = ?",
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("read deliverable: {e}"))?
        .map(|(title, claim, state, friction)| {
            let claim = claim.unwrap_or_default();
            let friction = friction.unwrap_or_default();
            let mut s = format!("Deliverable: {title}\nState: {state}\n{claim}");
            if !friction.is_empty() {
                s.push_str("\nBlocker: ");
                s.push_str(&friction);
            }
            s
        }),
        EntityKind::Capture => sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT text, COALESCE(kind, '') FROM captures WHERE id = ?",
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("read capture: {e}"))?
        .map(|(text, kind)| {
            let kind = kind.unwrap_or_default();
            if kind.is_empty() {
                format!("Capture: {text}")
            } else {
                format!("Capture ({kind}): {text}")
            }
        }),
        EntityKind::Stakeholder => sqlx::query_as::<_, (
            String,
            Option<String>,
            Option<String>,
        )>(
            "SELECT name, COALESCE(role, ''), COALESCE(notes, '')
               FROM stakeholders WHERE id = ?",
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("read stakeholder: {e}"))?
        .map(|(name, role, notes)| {
            format!(
                "Stakeholder: {name}\nRole: {role}\n{notes}",
                role = role.unwrap_or_default(),
                notes = notes.unwrap_or_default()
            )
        }),
        EntityKind::Meeting => sqlx::query_as::<_, (
            String,
            Option<String>,
            Option<String>,
        )>(
            "SELECT title, COALESCE(summary, ''), COALESCE(key_decisions, '')
               FROM meetings WHERE id = ?",
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("read meeting: {e}"))?
        .map(|(title, summary, decisions)| {
            format!(
                "Meeting: {title}\n{summary}\nDecisions: {decisions}",
                summary = summary.unwrap_or_default(),
                decisions = decisions.unwrap_or_default()
            )
        }),
        EntityKind::Memory => sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT content, COALESCE(kind, '') FROM memories WHERE id = ?",
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("read memory: {e}"))?
        .map(|(content, kind)| {
            let kind = kind.unwrap_or_default();
            if kind.is_empty() {
                format!("Memory: {content}")
            } else {
                format!("Memory ({kind}): {content}")
            }
        }),
        EntityKind::GmailThread => sqlx::query_as::<_, (
            String,
            Option<String>,
            Option<String>,
        )>(
            "SELECT COALESCE(subject, ''), COALESCE(snippet, ''), COALESCE(ai_category, '')
               FROM gmail_threads WHERE thread_id = ?",
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("read gmail thread: {e}"))?
        .map(|(subject, snippet, category)| {
            let subject = if subject.trim().is_empty() {
                "(no subject)".to_string()
            } else {
                subject
            };
            let snippet = snippet.unwrap_or_default();
            let category = category.unwrap_or_default();
            format!("Email ({category}): {subject}\n{snippet}")
        }),
        EntityKind::ReasoningSourceUnit => sqlx::query_as::<_, (String, String, String)>(
            "SELECT title, body, source_kind FROM reasoning_source_units WHERE id = ? AND access_state = 'included'",
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("read reasoning source unit: {e}"))?
        .map(|(title, body, kind)| format!("Evidence ({kind}): {title}\n{body}")),
    };

    Ok(text.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

/// Enqueue an entity for re-embedding. Cheap; calls during command handlers.
/// Pass `priority > 0` for actively-viewed entities so they jump the queue.
pub async fn enqueue(
    pool: &SqlitePool,
    kind: EntityKind,
    entity_id: &str,
    priority: i64,
) -> Result<(), String> {
    let Some(text) = build_text(pool, kind, entity_id).await? else {
        return Ok(());
    };
    let content_hash = hash_content(&text);

    // If we already have a current embedding for this exact content/model, skip.
    let current: Option<(String, String, i64)> = sqlx::query_as(
        "SELECT content_hash, model, dim
           FROM entity_embeddings
          WHERE entity_kind = ? AND entity_id = ?",
    )
    .bind(kind.as_str())
    .bind(entity_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("check existing: {e}"))?;
    if matches!(
        current,
        Some((ref hash, ref model, dim))
            if hash == &content_hash && model == EMBEDDING_MODEL && dim == EMBEDDING_DIM
    ) {
        return Ok(());
    }

    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO entity_embeddings_pending
           (entity_kind, entity_id, content_text, content_hash, enqueued_at, attempts, priority)
         VALUES (?, ?, ?, ?, ?, 0, ?)
         ON CONFLICT(entity_kind, entity_id) DO UPDATE SET
           content_text = excluded.content_text,
           content_hash = excluded.content_hash,
           enqueued_at = excluded.enqueued_at,
           attempts = 0,
           last_error = NULL,
           priority = MAX(entity_embeddings_pending.priority, excluded.priority)",
    )
    .bind(kind.as_str())
    .bind(entity_id)
    .bind(&text)
    .bind(&content_hash)
    .bind(now)
    .bind(priority)
    .execute(pool)
    .await
    .map_err(|e| format!("enqueue: {e}"))?;

    Ok(())
}

/// Debounced high-priority enqueue. When the user opens an entity in the
/// UI, this gets called fire-and-forget to push that entity's embedding
/// refresh to the front of the worker queue. Per-`(kind, id)` 3s debounce
/// prevents rapid clicks (e.g. arrow-key paging) from flooding inserts.
pub async fn enqueue_high_priority(
    pool: &SqlitePool,
    kind: EntityKind,
    entity_id: &str,
) -> Result<(), String> {
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    static RECENT: OnceLock<Mutex<std::collections::HashMap<(EntityKind, String), Instant>>> =
        OnceLock::new();
    const DEBOUNCE_MS: u128 = 3_000;
    const ACTIVE_PRIORITY: i64 = 100;

    let key = (kind, entity_id.to_string());
    let cell = RECENT.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    {
        let mut guard = cell.lock().expect("recent enqueue mutex poisoned");
        let now = Instant::now();
        if let Some(last) = guard.get(&key) {
            if now.duration_since(*last).as_millis() < DEBOUNCE_MS {
                return Ok(());
            }
        }
        guard.insert(key, now);
    }

    enqueue(pool, kind, entity_id, ACTIVE_PRIORITY).await
}

/// Enqueue every tracked entity that's missing or stale. Used at startup
/// for the first-run catch-up flow.
pub async fn enqueue_all_missing(pool: &SqlitePool) -> Result<i64, String> {
    let mut added = 0_i64;

    let stmts: &[(EntityKind, &str)] = &[
        (EntityKind::Initiative, "SELECT id FROM initiatives"),
        (EntityKind::Deliverable, "SELECT id FROM deliverables"),
        (EntityKind::Capture, "SELECT id FROM captures"),
        (EntityKind::Stakeholder, "SELECT id FROM stakeholders"),
        (EntityKind::Meeting, "SELECT id FROM meetings"),
        (EntityKind::Memory, "SELECT id FROM memories"),
        (
            EntityKind::GmailThread,
            "SELECT thread_id FROM gmail_threads",
        ),
        (
            EntityKind::ReasoningSourceUnit,
            "SELECT id FROM reasoning_source_units WHERE access_state = 'included'",
        ),
    ];

    for (kind, sql) in stmts {
        let ids: Vec<String> = sqlx::query_scalar(sql)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        for id in ids {
            if enqueue(pool, *kind, &id, 0).await.is_ok() {
                added += 1;
            }
        }
    }
    Ok(added)
}

/// Process up to `batch_size` pending entries. Returns how many were embedded.
pub async fn embed_pending(
    pool: &SqlitePool,
    api_key: &str,
    batch_size: usize,
) -> Result<usize, String> {
    let batch_size = batch_size.max(1);
    let rows: Vec<PendingRow> = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT entity_kind, entity_id, content_text, content_hash, attempts
           FROM entity_embeddings_pending
          ORDER BY priority DESC, enqueued_at ASC
          LIMIT ?",
    )
    .bind(batch_size as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("fetch pending: {e}"))?
    .into_iter()
    .map(|(k, id, t, h, a)| PendingRow {
        entity_kind: k,
        entity_id: id,
        content_text: t,
        content_hash: h,
        attempts: a,
    })
    .collect();

    let mut embedded = 0_usize;
    for row in rows {
        match gemini::embed_retrieval_document(Some(pool), api_key, &row.content_text).await {
            Ok(vec) => {
                let norm = vector_norm(&vec.values);
                let vector_json =
                    serde_json::to_string(&vec.values).unwrap_or_else(|_| "[]".into());
                let now = chrono::Utc::now().timestamp_millis();
                let tx_result = sqlx::query(
                    "INSERT INTO entity_embeddings
                       (entity_kind, entity_id, content_text, content_hash, model,
                        dim, vector_json, norm, embedded_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(entity_kind, entity_id) DO UPDATE SET
                       content_text = excluded.content_text,
                       content_hash = excluded.content_hash,
                       model = excluded.model,
                       dim = excluded.dim,
                       vector_json = excluded.vector_json,
                       norm = excluded.norm,
                       embedded_at = excluded.embedded_at",
                )
                .bind(&row.entity_kind)
                .bind(&row.entity_id)
                .bind(&row.content_text)
                .bind(&row.content_hash)
                .bind(&vec.model)
                .bind(vec.values.len() as i64)
                .bind(&vector_json)
                .bind(norm as f64)
                .bind(now)
                .execute(pool)
                .await;
                if let Err(error) = tx_result {
                    eprintln!("[entity_embeddings] insert failed: {error}");
                    bump_attempt(pool, &row, &error.to_string()).await;
                    continue;
                }
                let _ = sqlx::query(
                    "DELETE FROM entity_embeddings_pending
                      WHERE entity_kind = ? AND entity_id = ?",
                )
                .bind(&row.entity_kind)
                .bind(&row.entity_id)
                .execute(pool)
                .await;
                embedded += 1;
            }
            Err(error) => {
                eprintln!(
                    "[entity_embeddings] embed failed for {}:{}: {error}",
                    row.entity_kind, row.entity_id
                );
                bump_attempt(pool, &row, &error).await;
                // Stop the batch on any service-wide condition: rate limits,
                // auth failures, HTTP 5xx, network errors, or circuit open.
                // Per-item errors ("cannot embed empty text") don't match and
                // let the batch continue.
                let e = error.to_lowercase();
                let service_error = e.contains("rate")
                    || e.contains("circuit open")
                    || e.contains("403")
                    || e.contains("503")
                    || e.contains("500")
                    || e.contains("502")
                    || e.contains("504")
                    || e.contains("error sending request");
                if service_error {
                    break;
                }
            }
        }
    }
    Ok(embedded)
}

async fn bump_attempt(pool: &SqlitePool, row: &PendingRow, error: &str) {
    let _ = sqlx::query(
        "UPDATE entity_embeddings_pending
            SET attempts = attempts + 1, last_error = ?
          WHERE entity_kind = ? AND entity_id = ?",
    )
    .bind(error)
    .bind(&row.entity_kind)
    .bind(&row.entity_id)
    .execute(pool)
    .await;
}

pub async fn progress(pool: &SqlitePool) -> EmbeddingProgress {
    let embedded: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM entity_embeddings WHERE model = ? AND dim = ?")
            .bind(EMBEDDING_MODEL)
            .bind(EMBEDDING_DIM)
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entity_embeddings_pending")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let stale: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM entity_embeddings_pending p
          WHERE EXISTS (
            SELECT 1 FROM entity_embeddings e
             WHERE e.entity_kind = p.entity_kind AND e.entity_id = p.entity_id
          )",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    EmbeddingProgress {
        model: EMBEDDING_MODEL.to_string(),
        dim: EMBEDDING_DIM,
        total: embedded + pending,
        embedded,
        pending,
        stale,
    }
}

/// Load all stored embeddings for a set of entity_ids (across kinds). Returns
/// a map keyed by (kind, entity_id) → (vector, norm). Used by the brain
/// scorer to compute cosine similarity for candidate nodes.
pub async fn load_embeddings_for(
    pool: &SqlitePool,
    keys: &[(String, String)],
) -> Result<HashMap<(String, String), (Vec<f32>, f32)>, String> {
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    // Bulk fetch — small enough scale to OR them.
    let mut sql =
        String::from("SELECT entity_kind, entity_id, vector_json, norm FROM entity_embeddings WHERE (entity_kind, entity_id) IN (");
    for i in 0..keys.len() {
        if i > 0 {
            sql.push(',');
        }
        sql.push_str("(?, ?)");
    }
    sql.push_str(") AND model = ? AND dim = ?");

    let mut query = sqlx::query_as::<_, (String, String, String, f64)>(&sql);
    for (k, id) in keys {
        query = query.bind(k).bind(id);
    }
    query = query.bind(EMBEDDING_MODEL).bind(EMBEDDING_DIM);
    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("load embeddings: {e}"))?;

    let mut map = HashMap::with_capacity(rows.len());
    for (kind, id, vector_json, norm) in rows {
        let Ok(values) = serde_json::from_str::<Vec<f32>>(&vector_json) else {
            continue;
        };
        map.insert((kind, id), (values, norm as f32));
    }
    Ok(map)
}

pub fn vector_norm(values: &[f32]) -> f32 {
    values.iter().map(|v| v * v).sum::<f32>().sqrt()
}

pub fn cosine(a: &[f32], a_norm: f32, b: &[f32], b_norm: f32) -> f32 {
    if a_norm == 0.0 || b_norm == 0.0 || a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    dot / (a_norm * b_norm)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn insert_stakeholder(pool: &SqlitePool, id: &str, name: &str) {
        sqlx::query("INSERT INTO stakeholders (id, name) VALUES (?, ?)")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await
            .expect("insert stakeholder");
    }

    async fn insert_embedding(
        pool: &SqlitePool,
        entity_id: &str,
        content_text: &str,
        model: &str,
        dim: i64,
    ) {
        sqlx::query(
            "INSERT INTO entity_embeddings
               (entity_kind, entity_id, content_text, content_hash, model, dim,
                vector_json, norm, embedded_at)
             VALUES ('stakeholder', ?, ?, ?, ?, ?, '[1.0]', 1.0, 0)",
        )
        .bind(entity_id)
        .bind(content_text)
        .bind(hash_content(content_text))
        .bind(model)
        .bind(dim)
        .execute(pool)
        .await
        .expect("insert embedding");
    }

    #[tokio::test]
    async fn enqueue_treats_old_model_embedding_as_stale() {
        let pool = crate::db::connect_memory().await.expect("database");
        insert_stakeholder(&pool, "stakeholder-old-model", "Ada").await;
        let text = build_text(&pool, EntityKind::Stakeholder, "stakeholder-old-model")
            .await
            .expect("build text")
            .expect("text");
        insert_embedding(
            &pool,
            "stakeholder-old-model",
            &text,
            "text-embedding-004",
            768,
        )
        .await;

        enqueue(&pool, EntityKind::Stakeholder, "stakeholder-old-model", 0)
            .await
            .expect("enqueue");

        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM entity_embeddings_pending WHERE entity_id = ?",
        )
        .bind("stakeholder-old-model")
        .fetch_one(&pool)
        .await
        .expect("pending count");
        assert_eq!(pending, 1);
    }

    #[tokio::test]
    async fn enqueue_skips_current_model_embedding() {
        let pool = crate::db::connect_memory().await.expect("database");
        insert_stakeholder(&pool, "stakeholder-current-model", "Grace").await;
        let text = build_text(&pool, EntityKind::Stakeholder, "stakeholder-current-model")
            .await
            .expect("build text")
            .expect("text");
        insert_embedding(
            &pool,
            "stakeholder-current-model",
            &text,
            EMBEDDING_MODEL,
            EMBEDDING_DIM,
        )
        .await;

        enqueue(
            &pool,
            EntityKind::Stakeholder,
            "stakeholder-current-model",
            0,
        )
        .await
        .expect("enqueue");

        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM entity_embeddings_pending WHERE entity_id = ?",
        )
        .bind("stakeholder-current-model")
        .fetch_one(&pool)
        .await
        .expect("pending count");
        assert_eq!(pending, 0);
    }

    #[test]
    fn cosine_returns_zero_for_dimension_mismatch() {
        assert_eq!(cosine(&[1.0, 0.0], 1.0, &[1.0], 1.0), 0.0);
    }
}
