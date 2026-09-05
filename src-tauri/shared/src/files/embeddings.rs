//! Drive content embeddings + semantic file search tools. From legacy.rs (13-std5).

use sqlx::SqlitePool;

use crate::db::sql_error;
use crate::models::*;
use super::legacy::*;
use super::linking::files_for_entity;
fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = text.chars().collect();
    while start < chars.len() {
        let end = (start + max_chars).min(chars.len());
        // walk back to last whitespace to avoid splitting mid-word
        let end = if end < chars.len() {
            chars[start..end]
                .iter()
                .rposition(|c| c.is_whitespace())
                .map(|p| start + p + 1)
                .unwrap_or(end)
        } else {
            end
        };
        let chunk: String = chars[start..end].iter().collect();
        let trimmed = chunk.trim().to_string();
        if !trimmed.is_empty() {
            chunks.push(trimmed);
        }
        if end == start {
            // safety: avoid infinite loop on zero-width advance
            start += 1;
        } else {
            start = end;
        }
    }
    chunks
}

fn embedding_fingerprint(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    format!("h{:016x}", h.finish())
}

fn vector_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Embed the text content of a Google Drive Workspace file into `file_embeddings`.
/// - Chunks the exported text into ~2 000-char pieces.
/// - Skips chunks whose fingerprint hasn't changed (idempotent).
/// - Prunes stale trailing chunks after the current set.
/// - No-ops for non-Workspace mime types.
pub async fn embed_drive_file_content(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    token_dir: &std::path::Path,
    file_id: &str,
    drive_file_id: &str,
    mime_type: &str,
) -> Result<(), String> {
    if !SUPPORTED_EMBEDDING_DRIVE_MIME.contains(&mime_type) {
        return Ok(());
    }

    let text = crate::google_drive::export_doc_text(token_dir, drive_file_id, mime_type).await?;
    if text.trim().is_empty() {
        return Ok(());
    }

    let chunks = chunk_text(&text, 2000);
    for (i, chunk) in chunks.iter().enumerate() {
        let fp = embedding_fingerprint(chunk);
        let existing: Option<(String, String, i64)> = sqlx::query_as(
            "SELECT fingerprint, model, dim FROM file_embeddings WHERE file_id = ? AND chunk_index = ?",
        )
        .bind(file_id)
        .bind(i as i64)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;

        if matches!(
            existing,
            Some((ref existing_fp, ref model, dim))
                if existing_fp == &fp
                    && model == crate::gemini::EMBEDDING_MODEL
                    && dim == crate::gemini::EMBEDDING_OUTPUT_DIMENSIONALITY as i64
        ) {
            continue;
        }

        let vec = crate::gemini::embed_retrieval_document(Some(pool), api_key, chunk).await?;
        let norm = vector_norm(&vec.values) as f64;
        let vec_json = serde_json::to_string(&vec.values).map_err(|e| e.to_string())?;
        let ts = now();
        sqlx::query(
            "INSERT INTO file_embeddings \
              (file_id, chunk_index, chunk_text, model, dim, vector_json, norm, fingerprint, created_at, updated_at) \
             VALUES (?,?,?,?,?,?,?,?,?,?) \
             ON CONFLICT(file_id, chunk_index) DO UPDATE SET \
               chunk_text=excluded.chunk_text, model=excluded.model, dim=excluded.dim, \
               vector_json=excluded.vector_json, norm=excluded.norm, \
               fingerprint=excluded.fingerprint, updated_at=excluded.updated_at",
        )
        .bind(file_id)
        .bind(i as i64)
        .bind(chunk)
        .bind(&vec.model)
        .bind(vec.values.len() as i64)
        .bind(&vec_json)
        .bind(norm)
        .bind(&fp)
        .bind(&ts)
        .bind(&ts)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    // Prune stale chunks beyond the current set.
    sqlx::query("DELETE FROM file_embeddings WHERE file_id = ? AND chunk_index >= ?")
        .bind(file_id)
        .bind(chunks.len() as i64)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    Ok(())
}

/// Backfill missing/current Google Workspace file embeddings in small batches.
pub async fn embed_missing_drive_file_contents(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    token_dir: &std::path::Path,
    limit: i64,
) -> Result<i64, String> {
    let limit = limit.clamp(1, 25);
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id,
                COALESCE(drive_file_id, ''),
                COALESCE(NULLIF(drive_mime, ''), NULLIF(mime_type, ''), '')
           FROM files
          WHERE kind = 'drive'
            AND COALESCE(drive_file_id, '') != ''
            AND drive_trashed = 0
            AND COALESCE(NULLIF(drive_mime, ''), NULLIF(mime_type, ''), '') IN (
              'application/vnd.google-apps.document',
              'application/vnd.google-apps.spreadsheet',
              'application/vnd.google-apps.presentation'
            )
            AND NOT EXISTS (
              SELECT 1
                FROM file_embeddings fe
               WHERE fe.file_id = files.id
                 AND fe.model = ?
                 AND fe.dim = ?
            )
          ORDER BY updated_at DESC
          LIMIT ?",
    )
    .bind(crate::gemini::EMBEDDING_MODEL)
    .bind(crate::gemini::EMBEDDING_OUTPUT_DIMENSIONALITY as i64)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut embedded = 0_i64;
    for (file_id, drive_file_id, mime_type) in rows {
        match embed_drive_file_content(
            pool,
            api_key,
            token_dir,
            &file_id,
            &drive_file_id,
            &mime_type,
        )
        .await
        {
            Ok(()) => embedded += 1,
            Err(error) => eprintln!("[files] backfill embed error for {file_id}: {error}"),
        }
    }
    Ok(embedded)
}

// ── Semantic search helper for dispatch_tool ───────────────────────────────────────

/// FTS5 + embedding cosine-similarity search over files.
///
/// Returns a JSON array where each entry has `name`, `kind`, `chunk_excerpt`, `score`.
/// Falls back to FTS5-only if no embeddings exist or the api_key is unavailable.
pub async fn tool_search_files_semantic(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    query: &str,
) -> serde_json::Value {
    // 1. FTS5 BM25 search — always run this.
    let fts_files = match search_files(pool, query, 10).await {
        Ok(f) => f,
        Err(e) => return serde_json::json!({ "ok": false, "error": e }),
    };

    // Build FTS result map: file_id → (file, bm25_score).
    let mut results: std::collections::HashMap<
        String,
        (crate::models::FileRow, f32, Option<String>),
    > = fts_files
        .into_iter()
        .enumerate()
        .map(|(rank, f)| {
            let score = 1.0_f32 / (1.0 + rank as f32); // rank 0 → 1.0, rank 1 → 0.5 …
            let id = f.id.clone();
            (id, (f, score, None))
        })
        .collect();

    // 2. Semantic embedding search.
    let embed_result = crate::gemini::embed_retrieval_query(Some(pool), api_key, query).await;
    if let Ok(qvec) = embed_result {
        let qnorm = vector_norm(&qvec.values);
        if qnorm > 0.0 {
            // Load all file_embeddings rows.
            #[derive(sqlx::FromRow)]
            struct EmbRow {
                file_id: String,
                chunk_text: String,
                vector_json: String,
                norm: f64,
            }
            let rows: Vec<EmbRow> = sqlx::query_as(
                "SELECT fe.file_id, fe.chunk_text, fe.vector_json, fe.norm \
                 FROM file_embeddings fe \
                 WHERE fe.model = ? AND fe.dim = ?",
            )
            .bind(crate::gemini::EMBEDDING_MODEL)
            .bind(crate::gemini::EMBEDDING_OUTPUT_DIMENSIONALITY as i64)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            // Find best-scoring chunk per file.
            let mut semantic: std::collections::HashMap<String, (f32, String)> =
                std::collections::HashMap::new();
            for row in rows {
                let stored_norm = row.norm as f32;
                if stored_norm == 0.0 || qnorm == 0.0 {
                    continue;
                }
                let Ok(stored_vals) = serde_json::from_str::<Vec<f32>>(&row.vector_json) else {
                    continue;
                };
                if stored_vals.len() != qvec.values.len() {
                    continue;
                }
                let dot: f32 = qvec
                    .values
                    .iter()
                    .zip(stored_vals.iter())
                    .map(|(a, b)| a * b)
                    .sum();
                let cos = dot / (qnorm * stored_norm);
                let entry = semantic
                    .entry(row.file_id.clone())
                    .or_insert((f32::NEG_INFINITY, String::new()));
                if cos > entry.0 {
                    *entry = (cos, row.chunk_text.clone());
                }
            }

            // Merge semantic hits into results.
            for (file_id, (cos, excerpt)) in semantic {
                if let Some(entry) = results.get_mut(&file_id) {
                    // Blend: take max of BM25 rank score and semantic score.
                    entry.1 = entry.1.max(cos);
                    entry.2 = Some(excerpt.chars().take(400).collect());
                } else {
                    // Semantic-only hit: look up the file row.
                    if let Ok(file) = get_file(pool, &file_id).await {
                        let excerpt_short: String = excerpt.chars().take(400).collect();
                        results.insert(file_id, (file, cos, Some(excerpt_short)));
                    }
                }
            }
        }
    }

    // 3. Sort by score descending, take top 10.
    let mut sorted: Vec<_> = results.into_values().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(10);

    let items: Vec<serde_json::Value> = sorted
        .into_iter()
        .map(|(f, score, excerpt)| {
            serde_json::json!({
                "id": f.id,
                "name": f.name,
                "kind": f.kind,
                "mime_type": f.mime_type,
                "drive_web_view_link": f.drive_web_view_link,
                "local_path": f.local_path,
                "is_missing": f.is_missing,
                "drive_trashed": f.drive_trashed,
                "chunk_excerpt": excerpt,
                "score": score
            })
        })
        .collect();

    serde_json::json!({ "ok": true, "files": items, "count": items.len() })
}

pub async fn tool_search_files(pool: &SqlitePool, query: &str) -> serde_json::Value {
    match search_files(pool, query, 10).await {
        Ok(files) => {
            let items: Vec<serde_json::Value> = files
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "id": f.id,
                        "name": f.name,
                        "kind": f.kind,
                        "mime_type": f.mime_type,
                        "local_path": f.local_path,
                        "drive_web_view_link": f.drive_web_view_link,
                        "is_missing": f.is_missing,
                        "drive_trashed": f.drive_trashed,
                        "linked_to": f.links.iter().map(|l| serde_json::json!({
                            "entity_kind": l.entity_kind,
                            "entity_id": l.entity_id
                        })).collect::<Vec<_>>()
                    })
                })
                .collect();
            serde_json::json!({ "ok": true, "files": items, "count": items.len() })
        }
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    }
}

pub async fn tool_list_files_for_entity(
    pool: &SqlitePool,
    entity_kind: &str,
    entity_id: &str,
) -> serde_json::Value {
    let kind = match entity_kind {
        "initiative" => FileEntityKind::Initiative,
        "deliverable" => FileEntityKind::Deliverable,
        "deliverable_task" => FileEntityKind::DeliverableTask,
        "stakeholder" => FileEntityKind::Stakeholder,
        "capture" => FileEntityKind::Capture,
        "meeting" => FileEntityKind::Meeting,
        "conversation" => FileEntityKind::Conversation,
        other => {
            return serde_json::json!({ "ok": false, "error": format!("unknown entity_kind: {other}") })
        }
    };
    match files_for_entity(pool, kind, entity_id).await {
        Ok(files) => {
            let items: Vec<serde_json::Value> = files
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "id": f.id,
                        "name": f.name,
                        "kind": f.kind,
                        "mime_type": f.mime_type,
                        "local_path": f.local_path,
                        "drive_web_view_link": f.drive_web_view_link,
                        "is_missing": f.is_missing,
                        "drive_trashed": f.drive_trashed
                    })
                })
                .collect();
            serde_json::json!({ "ok": true, "files": items, "count": items.len() })
        }
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    }
}

pub async fn tool_get_file_detail(pool: &SqlitePool, file_id: &str) -> serde_json::Value {
    match get_file(pool, file_id).await {
        Ok(f) => serde_json::json!({
            "ok": true,
            "id": f.id,
            "name": f.name,
            "kind": f.kind,
            "mime_type": f.mime_type,
            "size_bytes": f.size_bytes,
            "description": f.description,
            "local_path": f.local_path,
            "is_missing": f.is_missing,
            "drive_file_id": f.drive_file_id,
            "drive_web_view_link": f.drive_web_view_link,
            "drive_trashed": f.drive_trashed,
            "created_at": f.created_at,
            "updated_at": f.updated_at,
            "linked_to": f.links.iter().map(|l| serde_json::json!({
                "entity_kind": l.entity_kind,
                "entity_id": l.entity_id,
                "linked_at": l.linked_at
            })).collect::<Vec<_>>()
        }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    }
}
