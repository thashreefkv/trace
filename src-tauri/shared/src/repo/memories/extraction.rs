use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{MemoryExtractionResult, MemoryKind, MemoryRecord},
};

use super::super::{
    clamp_score, clean_tags, get_conversation, json_string, now_utc,
};
use super::{
    clean_memory_scope, clean_memory_sensitivity, get_memory, memory_canonical_key,
    record_memory_event, upsert_memory_embedding,
};

pub async fn extract_memories_from_conversation(
    pool: &SqlitePool,
    conversation_id: &str,
    api_key: &str,
    system_prompt: &str,
) -> Result<MemoryExtractionResult, String> {
    let conversation = get_conversation(pool, conversation_id).await?;
    let title = conversation
        .title
        .as_deref()
        .unwrap_or("Untitled conversation");
    let summary = conversation
        .summary
        .as_deref()
        .unwrap_or("No summary saved.");
    let occurred = conversation
        .occurred_at
        .as_deref()
        .unwrap_or("unknown date");
    let source = format!(
        "Conversation '{title}' on {occurred}.\nChat URL: {url}\nSummary: {summary}",
        url = conversation.chat_url
    );

    extract_memories_from_text(
        pool,
        "conversation",
        Some(conversation_id),
        &source,
        api_key,
        system_prompt,
    )
    .await
}

pub async fn extract_memories_from_text(
    pool: &SqlitePool,
    source_kind: &str,
    source_id: Option<&str>,
    source_text: &str,
    api_key: &str,
    system_prompt: &str,
) -> Result<MemoryExtractionResult, String> {
    let trimmed = source_text.trim();
    if trimmed.is_empty() {
        return Ok(MemoryExtractionResult {
            created_count: 0,
            updated_count: 0,
            skipped_count: 0,
            memories: Vec::new(),
        });
    }
    let payload =
        crate::gemini::extract_memory_candidates(pool, api_key, system_prompt, trimmed).await?;
    ingest_memory_candidates(pool, payload.memories, source_kind, source_id, api_key).await
}

pub async fn ingest_memory_candidates(
    pool: &SqlitePool,
    candidates: Vec<crate::models::ExtractedMemoryCandidate>,
    source_kind: &str,
    source_id: Option<&str>,
    api_key: &str,
) -> Result<MemoryExtractionResult, String> {
    let mut created_count = 0_i64;
    let mut updated_count = 0_i64;
    let mut skipped_count = 0_i64;
    let mut touched: Vec<MemoryRecord> = Vec::new();

    for candidate in candidates {
        let title = candidate.title.trim();
        let body = candidate.body.trim();
        if title.is_empty() || body.is_empty() {
            skipped_count += 1;
            continue;
        }
        let scope = match clean_memory_scope(&candidate.scope) {
            Ok(scope) => scope,
            Err(_) => "global".to_string(),
        };
        let sensitivity = clean_memory_sensitivity(candidate.sensitivity.as_deref())
            .unwrap_or_else(|_| "normal".to_string());
        let confidence = clamp_score(candidate.confidence.unwrap_or(0.78));
        let importance = clamp_score(candidate.importance.unwrap_or(0.7));
        let canonical_key = memory_canonical_key(candidate.kind.as_str(), title, body);
        let tags = clean_tags(candidate.tags);
        let evidence: Vec<String> = candidate
            .evidence
            .into_iter()
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty())
            .take(6)
            .collect();

        let existing_id: Option<String> = sqlx::query_scalar(
            r#"
            SELECT id FROM memories
            WHERE canonical_key = ?
              AND status != 'deleted'
              AND deleted_at IS NULL
            LIMIT 1
            "#,
        )
        .bind(&canonical_key)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;

        let now = now_utc();
        let tags_json = json_string(&tags)?;
        let evidence_json = json_string(&evidence)?;

        let id = match existing_id {
            Some(id) => {
                sqlx::query(
                    r#"
                    UPDATE memories
                    SET kind = ?,
                        status = 'active',
                        scope = ?,
                        title = ?,
                        body = ?,
                        canonical_key = ?,
                        source = CASE WHEN source = 'manual' OR source = 'system' THEN source ELSE 'generated' END,
                        source_kind = ?,
                        source_id = ?,
                        confidence = MAX(confidence, ?),
                        importance = MAX(importance, ?),
                        tags_json = ?,
                        evidence_json = ?,
                        sensitivity = ?,
                        version = version + 1,
                        archived_at = NULL,
                        deleted_at = NULL,
                        updated_at = ?
                    WHERE id = ?
                    "#,
                )
                .bind(candidate.kind.as_str())
                .bind(&scope)
                .bind(title)
                .bind(body)
                .bind(&canonical_key)
                .bind(source_kind)
                .bind(source_id)
                .bind(confidence)
                .bind(importance)
                .bind(&tags_json)
                .bind(&evidence_json)
                .bind(&sensitivity)
                .bind(&now)
                .bind(&id)
                .execute(pool)
                .await
                .map_err(sql_error)?;
                updated_count += 1;
                id
            }
            None => {
                let id = Ulid::new().to_string();
                sqlx::query(
                    r#"
                    INSERT INTO memories (
                      id, kind, status, scope, title, body, canonical_key, source,
                      source_kind, source_id, confidence, importance, tags_json, evidence_json,
                      sensitivity, pinned, created_at, updated_at
                    ) VALUES (?, ?, 'active', ?, ?, ?, ?, 'generated', ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)
                    "#,
                )
                .bind(&id)
                .bind(candidate.kind.as_str())
                .bind(&scope)
                .bind(title)
                .bind(body)
                .bind(&canonical_key)
                .bind(source_kind)
                .bind(source_id)
                .bind(confidence)
                .bind(importance)
                .bind(&tags_json)
                .bind(&evidence_json)
                .bind(&sensitivity)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map_err(sql_error)?;
                created_count += 1;
                id
            }
        };

        record_memory_event(
            pool,
            Some(&id),
            "extracted",
            serde_json::json!({
                "title": title,
                "source_kind": source_kind,
                "source_id": source_id,
            }),
        )
        .await?;

        let memory = get_memory(pool, &id).await?;
        let combined = format!("{}\n{}", memory.title, memory.body);
        let _ = upsert_memory_embedding(pool, &memory.id, &combined, api_key).await;
        touched.push(memory);
    }

    Ok(MemoryExtractionResult {
        created_count,
        updated_count,
        skipped_count,
        memories: touched,
    })
}

pub async fn upsert_generated_memory(
    pool: &SqlitePool,
    kind: MemoryKind,
    title: &str,
    body: &str,
    canonical_key: &str,
    source_kind: &str,
    source_id: Option<&str>,
    tags: &[&str],
    evidence: &[&str],
    confidence: f64,
    importance: f64,
) -> Result<(MemoryRecord, bool), String> {
    let existing_id: Option<String> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM memories
        WHERE canonical_key = ?
          AND status != 'deleted'
          AND deleted_at IS NULL
        LIMIT 1
        "#,
    )
    .bind(canonical_key)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let created = existing_id.is_none();
    let id = existing_id.unwrap_or_else(|| Ulid::new().to_string());
    let now = now_utc();
    let tags = tags.iter().map(|tag| tag.to_string()).collect::<Vec<_>>();
    let evidence = evidence
        .iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>();
    let tags_json = json_string(&tags)?;
    let evidence_json = json_string(&evidence)?;

    if created {
        sqlx::query(
            r#"
            INSERT INTO memories (
              id, kind, status, scope, title, body, canonical_key, source,
              source_kind, source_id, confidence, importance, tags_json, evidence_json,
              created_at, updated_at
            ) VALUES (?, ?, 'active', 'global', ?, ?, ?, 'consolidated', ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(kind.as_str())
        .bind(title)
        .bind(body)
        .bind(canonical_key)
        .bind(source_kind)
        .bind(source_id)
        .bind(clamp_score(confidence))
        .bind(clamp_score(importance))
        .bind(&tags_json)
        .bind(&evidence_json)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    } else {
        sqlx::query(
            r#"
            UPDATE memories
            SET kind = ?,
                status = 'active',
                scope = 'global',
                title = ?,
                body = ?,
                source = CASE WHEN source = 'manual' THEN source ELSE 'consolidated' END,
                source_kind = ?,
                source_id = ?,
                confidence = MAX(confidence, ?),
                importance = MAX(importance, ?),
                tags_json = ?,
                evidence_json = ?,
                archived_at = NULL,
                deleted_at = NULL,
                version = version + 1,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(kind.as_str())
        .bind(title)
        .bind(body)
        .bind(source_kind)
        .bind(source_id)
        .bind(clamp_score(confidence))
        .bind(clamp_score(importance))
        .bind(&tags_json)
        .bind(&evidence_json)
        .bind(&now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    let memory = get_memory(pool, &id).await?;
    Ok((memory, created))
}



