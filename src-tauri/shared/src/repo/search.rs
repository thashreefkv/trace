use sqlx::SqlitePool;

use crate::{
    db::sql_error,
    models::{MemoryRow, RetrieveMemoryInput, SearchResult},
};

use super::{fts_query, retrieve_memories};

pub async fn search_all(pool: &SqlitePool, query: &str) -> Result<Vec<SearchResult>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut results: Vec<SearchResult> = Vec::new();

    if let Some(fts_q) = fts_query(trimmed) {
        let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            r#"
            SELECT d.id, d.title, d.claim, d.state, d.stakeholder_id
            FROM deliverable_search ds
            JOIN deliverables d ON d.rowid = ds.rowid
            WHERE deliverable_search MATCH ?
            ORDER BY rank
            LIMIT 8
            "#,
        )
        .bind(&fts_q)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?;

        for (id, title, claim, state, _) in rows {
            results.push(SearchResult {
                kind: "deliverable".to_string(),
                entity_id: id.clone(),
                title,
                subtitle: if claim.is_empty() {
                    None
                } else {
                    Some(claim.chars().take(80).collect())
                },
                route: format!("/deliverables/{id}"),
                state: Some(state),
            });
        }

        let email_rows = sqlx::query_as::<_, (String, String, String, Option<i64>)>(
            r#"
            SELECT t.thread_id, t.subject, t.snippet, t.last_message_at
            FROM gmail_thread_search gs
            JOIN gmail_threads t ON t.thread_id = gs.thread_id
            WHERE gmail_thread_search MATCH ?
            ORDER BY bm25(gmail_thread_search), t.last_message_at DESC
            LIMIT 8
            "#,
        )
        .bind(&fts_q)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (thread_id, subject, snippet, _) in email_rows {
            results.push(SearchResult {
                kind: "email".to_string(),
                entity_id: thread_id.clone(),
                title: subject,
                subtitle: if snippet.is_empty() {
                    None
                } else {
                    Some(snippet.chars().take(100).collect())
                },
                route: format!("/email?thread={thread_id}"),
                state: None,
            });
        }

        let memory_rows = sqlx::query_as::<_, MemoryRow>(
            r#"
            SELECT m.id, m.kind, m.status, m.scope, m.title, m.body, m.canonical_key,
                   m.source, m.source_kind, m.source_id, m.confidence, m.importance,
                   m.retrieval_count, m.success_count, m.contradiction_count, m.tags_json,
                   m.evidence_json, m.supersedes_id, m.version, m.expires_at, m.archived_at,
                   m.deleted_at, m.last_retrieved_at, m.created_at, m.updated_at
            FROM memory_search ms
            JOIN memories m ON m.rowid = ms.rowid
            WHERE memory_search MATCH ?
              AND m.status = 'active'
              AND m.deleted_at IS NULL
            ORDER BY bm25(memory_search), m.importance DESC, m.updated_at DESC
            LIMIT 8
            "#,
        )
        .bind(&fts_q)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for memory in memory_rows.into_iter().map(MemoryRow::into_record) {
            results.push(SearchResult {
                kind: "memory".to_string(),
                entity_id: memory.id,
                title: memory.title,
                subtitle: Some(memory.body.chars().take(100).collect()),
                route: "/context".to_string(),
                state: Some(memory.kind),
            });
        }
    }

    let like_q = format!("%{}%", trimmed);
    let init_rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, title, framing, status FROM initiatives WHERE title LIKE ? OR framing LIKE ? LIMIT 5",
    )
    .bind(&like_q)
    .bind(&like_q)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for (id, title, framing, status) in init_rows {
        results.push(SearchResult {
            kind: "initiative".to_string(),
            entity_id: id.clone(),
            title,
            subtitle: if framing.is_empty() {
                None
            } else {
                Some(framing.chars().take(80).collect())
            },
            route: format!("/initiatives/{id}"),
            state: Some(status),
        });
    }

    let sh_rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, role FROM stakeholders WHERE name LIKE ? OR role LIKE ? LIMIT 4",
    )
    .bind(&like_q)
    .bind(&like_q)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for (id, name, role) in sh_rows {
        results.push(SearchResult {
            kind: "stakeholder".to_string(),
            entity_id: id.clone(),
            title: name,
            subtitle: if role.is_empty() { None } else { Some(role) },
            route: format!("/stakeholders/{id}"),
            state: None,
        });
    }

    let meeting_rows = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT id, title, status, summary FROM meetings WHERE title LIKE ? LIMIT 4",
    )
    .bind(&like_q)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for (id, title, status, summary) in meeting_rows {
        results.push(SearchResult {
            kind: "meeting".to_string(),
            entity_id: id.clone(),
            title,
            subtitle: summary.map(|s| s.chars().take(80).collect()),
            route: format!("/meetings/{id}"),
            state: Some(status),
        });
    }

    let cap_rows = sqlx::query_as::<_, (String, String)>(
        "SELECT id, body FROM captures WHERE status = 'inbox' AND body LIKE ? LIMIT 4",
    )
    .bind(&like_q)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for (id, body) in cap_rows {
        let title: String = body.chars().take(60).collect();
        results.push(SearchResult {
            kind: "capture".to_string(),
            entity_id: id,
            title,
            subtitle: None,
            route: "/captures".to_string(),
            state: None,
        });
    }

    if let Some(ref fts_q) = fts_query(trimmed) {
        let file_rows = sqlx::query_as::<_, (String, String, Option<String>, String)>(
            "SELECT f.id, f.name, f.description, f.kind \
             FROM files f \
             JOIN file_search fs ON fs.id = f.id \
             WHERE file_search MATCH ? \
             ORDER BY rank LIMIT 6",
        )
        .bind(fts_q)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        for (id, name, description, kind) in file_rows {
            results.push(SearchResult {
                kind: "file".to_string(),
                entity_id: id.clone(),
                title: name,
                subtitle: description.map(|d| d.chars().take(80).collect()),
                route: format!("/files?file={id}"),
                state: Some(kind),
            });
        }
    }

    let file_like_rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, kind FROM files WHERE name LIKE ? LIMIT 5",
    )
    .bind(&like_q)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (id, name, kind) in file_like_rows {
        if !results.iter().any(|r| r.entity_id == id) {
            results.push(SearchResult {
                kind: "file".to_string(),
                entity_id: id.clone(),
                title: name,
                subtitle: None,
                route: format!("/files?file={id}"),
                state: Some(kind),
            });
        }
    }

    Ok(results)
}

pub async fn gather_ask_context(
    pool: &SqlitePool,
    question: &str,
) -> Result<serde_json::Value, String> {
    let like_q = format!("%{}%", question.trim());

    let initiatives = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, title, framing, status FROM initiatives ORDER BY updated_at DESC LIMIT 20",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let deliverables = if let Some(fts_q) = fts_query(question) {
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            r#"
            SELECT d.id, d.title, d.state, d.claim, d.blocker_reason
            FROM deliverable_search ds
            JOIN deliverables d ON d.rowid = ds.rowid
            WHERE deliverable_search MATCH ?
            ORDER BY rank LIMIT 12
            "#,
        )
        .bind(&fts_q)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    } else {
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT id, title, state, claim, blocker_reason FROM deliverables WHERE title LIKE ? OR claim LIKE ? LIMIT 12",
        )
        .bind(&like_q)
        .bind(&like_q)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    };

    let meetings = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
        "SELECT id, title, summary, key_decisions FROM meetings ORDER BY date DESC LIMIT 5",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let captures = sqlx::query_as::<_, (String, String)>(
        "SELECT id, body FROM captures WHERE status = 'inbox' ORDER BY created_at DESC LIMIT 8",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let memory = retrieve_memories(
        pool,
        RetrieveMemoryInput {
            query: question.to_string(),
            limit: Some(12),
            kinds: Vec::new(),
            source_kind: Some("ask_search".to_string()),
            source_id: None,
            task_type: None,
            include_pinned: Some(true),
        },
    )
    .await
    .ok();

    let ctx = serde_json::json!({
        "memory": memory.as_ref().map(|memory| serde_json::json!({
            "context": memory.context,
            "items": memory.memories.iter().map(|item| serde_json::json!({
                "id": item.id,
                "kind": item.kind,
                "title": item.title,
                "body": item.body,
                "source": item.source,
                "confidence": item.confidence,
                "importance": item.importance,
                "updated_at": item.updated_at
            })).collect::<Vec<_>>()
        })),
        "initiatives": initiatives.iter().map(|(id, title, framing, status)| serde_json::json!({
            "id": id, "title": title, "framing": framing, "status": status,
            "route": format!("/initiatives/{id}")
        })).collect::<Vec<_>>(),
        "deliverables": deliverables.iter().map(|(id, title, state, claim, blocker)| serde_json::json!({
            "id": id, "title": title, "state": state, "claim": claim,
            "blocker_reason": blocker,
            "route": format!("/deliverables/{id}")
        })).collect::<Vec<_>>(),
        "meetings": meetings.iter().map(|(id, title, summary, decisions)| serde_json::json!({
            "id": id, "title": title, "summary": summary,
            "key_decisions": decisions,
            "route": format!("/meetings/{id}")
        })).collect::<Vec<_>>(),
        "captures": captures.iter().map(|(id, body)| serde_json::json!({
            "id": id, "body": body
        })).collect::<Vec<_>>(),
    });

    Ok(ctx)
}
