mod extraction;
mod feedback;
mod retrieval;

pub use extraction::{
    extract_memories_from_conversation, extract_memories_from_text, ingest_memory_candidates,
    upsert_generated_memory,
};
pub use feedback::{
    consolidate_memories, list_memory_events, record_memory_feedback,
};
pub use retrieval::{
    ensure_active_memory_embeddings, invalidate_memory_embedding, retrieve_memories,
    retrieve_memories_with_key, upsert_memory_embedding,
};

use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{
        CreateMemoryInput, Deliverable, ListMemoryFilters, MemoryRecord, MemoryRow, MemorySettings,
        MemorySettingsRow, MemoryStatus, UpdateMemoryInput, UpdateMemorySettingsInput,
    },
};

use super::{
    bool_as_i64, clamp_score, clean_optional, clean_required, clean_tags, fts_query, json_string,
    now_utc,
};

pub async fn get_memory_settings(pool: &SqlitePool) -> Result<MemorySettings, String> {
    ensure_memory_settings(pool).await?;
    sqlx::query_as::<_, MemorySettingsRow>(
        r#"
        SELECT enabled, auto_extract_enabled, work_related_only, preserve_continuity,
               require_confirmation, retrieval_limit, updated_at
        FROM memory_settings
        WHERE id = 1
        "#,
    )
    .fetch_one(pool)
    .await
    .map(MemorySettingsRow::into_settings)
    .map_err(sql_error)
}

pub async fn update_memory_settings(
    pool: &SqlitePool,
    input: UpdateMemorySettingsInput,
) -> Result<MemorySettings, String> {
    let now = now_utc();
    let retrieval_limit = input.retrieval_limit.clamp(3, 40);
    ensure_memory_settings(pool).await?;

    sqlx::query(
        r#"
        UPDATE memory_settings
        SET enabled = ?,
            auto_extract_enabled = ?,
            work_related_only = ?,
            preserve_continuity = ?,
            require_confirmation = ?,
            retrieval_limit = ?,
            updated_at = ?
        WHERE id = 1
        "#,
    )
    .bind(bool_as_i64(input.enabled))
    .bind(bool_as_i64(input.auto_extract_enabled))
    .bind(bool_as_i64(input.work_related_only))
    .bind(bool_as_i64(input.preserve_continuity))
    .bind(bool_as_i64(input.require_confirmation))
    .bind(retrieval_limit)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    record_memory_event(
        pool,
        None,
        "settings_updated",
        serde_json::json!({ "updated_at": now }),
    )
    .await?;
    get_memory_settings(pool).await
}

pub async fn list_memories(
    pool: &SqlitePool,
    filters: ListMemoryFilters,
) -> Result<Vec<MemoryRecord>, String> {
    let rows = if let Some(query) = clean_optional(filters.query.clone()) {
        if let Some(fts) = fts_query(&query) {
            sqlx::query_as::<_, MemoryRow>(
                r#"
                SELECT m.id, m.kind, m.status, m.scope, m.title, m.body, m.canonical_key,
                       m.source, m.source_kind, m.source_id, m.confidence, m.importance,
                       m.retrieval_count, m.success_count, m.contradiction_count, m.tags_json,
                       m.evidence_json, m.supersedes_id, m.version, m.expires_at, m.archived_at,
                       m.deleted_at, m.last_retrieved_at,
                       COALESCE(m.sensitivity, 'normal') AS sensitivity,
                       COALESCE(m.pinned, 0) AS pinned,
                       m.created_at, m.updated_at
                FROM memory_search ms
                JOIN memories m ON m.rowid = ms.rowid
                WHERE memory_search MATCH ?
                  AND m.status != 'deleted'
                  AND m.deleted_at IS NULL
                ORDER BY bm25(memory_search), m.importance DESC, m.updated_at DESC
                LIMIT 200
                "#,
            )
            .bind(&fts)
            .fetch_all(pool)
            .await
            .map_err(sql_error)?
        } else {
            fetch_memory_rows(pool).await?
        }
    } else {
        fetch_memory_rows(pool).await?
    };

    Ok(rows
        .into_iter()
        .filter(|memory| memory_matches_filters(memory, &filters))
        .map(MemoryRow::into_record)
        .collect())
}

pub async fn create_memory(
    pool: &SqlitePool,
    input: CreateMemoryInput,
) -> Result<MemoryRecord, String> {
    let title = clean_required(input.title, "memory title")?;
    let body = clean_required(input.body, "memory body")?;
    let scope = clean_memory_scope(&input.scope)?;
    let tags = clean_tags(input.tags);
    let canonical_key = memory_canonical_key(input.kind.as_str(), &title, &body);
    let confidence = clamp_score(input.confidence.unwrap_or(0.95));
    let importance = clamp_score(input.importance.unwrap_or(0.75));
    let sensitivity = clean_memory_sensitivity(input.sensitivity.as_deref())?;
    let pinned = bool_as_i64(input.pinned.unwrap_or(false));
    let expires_at = clean_optional(input.expires_at);
    let id = Ulid::new().to_string();
    let now = now_utc();
    let tags_json = json_string(&tags)?;
    let evidence_json = json_string(&Vec::<String>::new())?;

    sqlx::query(
        r#"
        INSERT INTO memories (
          id, kind, status, scope, title, body, canonical_key, source,
          confidence, importance, tags_json, evidence_json, sensitivity, pinned,
          expires_at, created_at, updated_at
        ) VALUES (?, ?, 'active', ?, ?, ?, ?, 'manual', ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(canonical_key) WHERE status = 'active' AND deleted_at IS NULL
        DO UPDATE SET
          kind = excluded.kind,
          scope = excluded.scope,
          title = excluded.title,
          body = excluded.body,
          source = 'manual',
          confidence = excluded.confidence,
          importance = excluded.importance,
          tags_json = excluded.tags_json,
          sensitivity = excluded.sensitivity,
          pinned = excluded.pinned,
          expires_at = excluded.expires_at,
          version = memories.version + 1,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(&id)
    .bind(input.kind.as_str())
    .bind(&scope)
    .bind(&title)
    .bind(&body)
    .bind(&canonical_key)
    .bind(confidence)
    .bind(importance)
    .bind(&tags_json)
    .bind(&evidence_json)
    .bind(&sensitivity)
    .bind(pinned)
    .bind(&expires_at)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    let memory = get_memory_by_key(pool, &canonical_key).await?;
    record_memory_event(
        pool,
        Some(&memory.id),
        "upserted_manual",
        serde_json::json!({ "title": &memory.title, "kind": memory.kind }),
    )
    .await?;
    let _ = invalidate_memory_embedding(pool, &memory.id).await;
    Ok(memory)
}

pub async fn update_memory(
    pool: &SqlitePool,
    id: &str,
    input: UpdateMemoryInput,
) -> Result<MemoryRecord, String> {
    let title = clean_required(input.title, "memory title")?;
    let body = clean_required(input.body, "memory body")?;
    let scope = clean_memory_scope(&input.scope)?;
    let tags = clean_tags(input.tags);
    let tags_json = json_string(&tags)?;
    let confidence = clamp_score(input.confidence.unwrap_or(0.95));
    let importance = clamp_score(input.importance.unwrap_or(0.75));
    let sensitivity = clean_memory_sensitivity(input.sensitivity.as_deref())?;
    let pinned = bool_as_i64(input.pinned.unwrap_or(false));
    let expires_at = clean_optional(input.expires_at);
    let canonical_key = memory_canonical_key(input.kind.as_str(), &title, &body);
    let now = now_utc();
    let archived_at = if input.status == MemoryStatus::Archived {
        Some(now.clone())
    } else {
        None
    };
    let deleted_at = if input.status == MemoryStatus::Deleted {
        Some(now.clone())
    } else {
        None
    };

    let result = sqlx::query(
        r#"
        UPDATE memories
        SET kind = ?,
            status = ?,
            scope = ?,
            title = ?,
            body = ?,
            canonical_key = ?,
            source = CASE WHEN source = 'system' THEN source ELSE 'manual' END,
            confidence = ?,
            importance = ?,
            tags_json = ?,
            sensitivity = ?,
            pinned = ?,
            expires_at = ?,
            archived_at = ?,
            deleted_at = ?,
            version = version + 1,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(input.kind.as_str())
    .bind(input.status.as_str())
    .bind(&scope)
    .bind(&title)
    .bind(&body)
    .bind(&canonical_key)
    .bind(confidence)
    .bind(importance)
    .bind(&tags_json)
    .bind(&sensitivity)
    .bind(pinned)
    .bind(&expires_at)
    .bind(&archived_at)
    .bind(&deleted_at)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("memory not found".to_string());
    }

    record_memory_event(
        pool,
        Some(id),
        "updated",
        serde_json::json!({ "status": input.status.as_str(), "updated_at": now }),
    )
    .await?;
    let _ = invalidate_memory_embedding(pool, id).await;
    get_memory(pool, id).await
}

pub async fn delete_memory(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let now = now_utc();
    let result = sqlx::query(
        r#"
        UPDATE memories
        SET status = 'deleted',
            deleted_at = ?,
            updated_at = ?,
            version = version + 1
        WHERE id = ? AND deleted_at IS NULL
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("memory not found".to_string());
    }

    record_memory_event(
        pool,
        Some(id),
        "deleted",
        serde_json::json!({ "deleted_at": now }),
    )
    .await
}

pub async fn get_memory(pool: &SqlitePool, id: &str) -> Result<MemoryRecord, String> {
    sqlx::query_as::<_, MemoryRow>(
        r#"
        SELECT id, kind, status, scope, title, body, canonical_key, source, source_kind,
               source_id, confidence, importance, retrieval_count, success_count,
               contradiction_count, tags_json, evidence_json, supersedes_id, version,
               expires_at, archived_at, deleted_at, last_retrieved_at,
               COALESCE(sensitivity, 'normal') AS sensitivity,
               COALESCE(pinned, 0) AS pinned,
               created_at, updated_at
        FROM memories
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .map(MemoryRow::into_record)
    .ok_or_else(|| "memory not found".to_string())
}

pub async fn fetch_memory_rows(pool: &SqlitePool) -> Result<Vec<MemoryRow>, String> {
    sqlx::query_as::<_, MemoryRow>(
        r#"
        SELECT id, kind, status, scope, title, body, canonical_key, source, source_kind,
               source_id, confidence, importance, retrieval_count, success_count,
               contradiction_count, tags_json, evidence_json, supersedes_id, version,
               expires_at, archived_at, deleted_at, last_retrieved_at,
               COALESCE(sensitivity, 'normal') AS sensitivity,
               COALESCE(pinned, 0) AS pinned,
               created_at, updated_at
        FROM memories
        WHERE status != 'deleted'
          AND deleted_at IS NULL
        ORDER BY importance DESC, updated_at DESC
        LIMIT 500
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn get_memory_by_key(
    pool: &SqlitePool,
    canonical_key: &str,
) -> Result<MemoryRecord, String> {
    sqlx::query_as::<_, MemoryRow>(
        r#"
        SELECT id, kind, status, scope, title, body, canonical_key, source, source_kind,
               source_id, confidence, importance, retrieval_count, success_count,
               contradiction_count, tags_json, evidence_json, supersedes_id, version,
               expires_at, archived_at, deleted_at, last_retrieved_at,
               COALESCE(sensitivity, 'normal') AS sensitivity,
               COALESCE(pinned, 0) AS pinned,
               created_at, updated_at
        FROM memories
        WHERE canonical_key = ?
          AND status = 'active'
          AND deleted_at IS NULL
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(canonical_key)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .map(MemoryRow::into_record)
    .ok_or_else(|| "memory not found".to_string())
}

pub async fn ensure_memory_settings(pool: &SqlitePool) -> Result<(), String> {
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO memory_settings (
          id, enabled, auto_extract_enabled, work_related_only, preserve_continuity,
          require_confirmation, retrieval_limit, updated_at
        ) VALUES (1, 1, 1, 1, 1, 0, 12, ?)
        "#,
    )
    .bind(now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn record_memory_event(
    pool: &SqlitePool,
    memory_id: Option<&str>,
    action: &str,
    detail: serde_json::Value,
) -> Result<(), String> {
    let id = Ulid::new().to_string();
    let now = now_utc();
    let detail_json = detail.to_string();
    sqlx::query(
        "INSERT INTO memory_events (id, memory_id, action, detail_json, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(memory_id)
    .bind(action)
    .bind(detail_json)
    .bind(now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub fn memory_matches_filters(memory: &MemoryRow, filters: &ListMemoryFilters) -> bool {
    if let Some(kind) = filters.kind {
        if memory.kind != kind.as_str() {
            return false;
        }
    }
    if let Some(status) = filters.status {
        if memory.status != status.as_str() {
            return false;
        }
    } else if !filters.include_archived && memory.status != "active" {
        return false;
    }
    true
}

pub fn clean_memory_scope(value: &str) -> Result<String, String> {
    let scope = value.trim();
    match scope {
        "" => Ok("global".to_string()),
        "global" | "project" | "session" => Ok(scope.to_string()),
        _ => Err("memory scope must be global, project, or session".to_string()),
    }
}

pub fn clean_memory_sensitivity(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok("normal".to_string()),
        Some("normal") | Some("pii") | Some("sensitive") => Ok(value.unwrap().trim().to_string()),
        Some(other) => Err(format!(
            "memory sensitivity must be normal, pii, or sensitive, got '{other}'"
        )),
    }
}

pub fn memory_canonical_key(kind: &str, title: &str, body: &str) -> String {
    let normalized = format!("{title} {body}")
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .take(12)
        .collect::<Vec<_>>()
        .join("-");
    format!(
        "{kind}:{}",
        if normalized.is_empty() {
            "memory"
        } else {
            &normalized
        }
    )
}

pub trait DeliverableMemoryImportance {
    fn importance_hint(&self) -> f64;
}

impl DeliverableMemoryImportance for Deliverable {
    fn importance_hint(&self) -> f64 {
        let priority = match self.priority.as_deref() {
            Some("p1") => 0.20,
            Some("p2") => 0.10,
            _ => 0.0,
        };
        let focus = if self.is_focused { 0.15 } else { 0.0 };
        let shipped = if self.state == "shipped" { 0.10 } else { 0.0 };
        (0.58_f64 + priority + focus + shipped).min(0.95)
    }
}
