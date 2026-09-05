use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use ulid::Ulid;

use crate::db::sql_error;
use crate::models::{ReasoningIndexSummary, ReasoningScope, ReasoningSourceUnit};

use super::scope_resolver::ResolvedScope;
use super::{claims, communities};

const CHUNK_CHARS: usize = 2200;

#[derive(Debug, FromRow)]
struct RawSource {
    source_kind: String,
    source_id: String,
    title: String,
    body: String,
    route: Option<String>,
    updated_at: Option<String>,
}

pub async fn refresh_reasoning_index(pool: &SqlitePool) -> Result<ReasoningIndexSummary, String> {
    let now = crate::repo::now_utc();
    let sources = load_sources(pool).await?;
    let mut created = 0_i64;
    let mut updated = 0_i64;
    let mut unchanged = 0_i64;
    let mut invalidated_source_units = Vec::new();

    sqlx::query("UPDATE reasoning_source_units SET access_state = 'deleted', updated_at = ?")
        .bind(&now)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    for source in sources {
        for (chunk_index, chunk) in chunk_text(&source.body).into_iter().enumerate() {
            let id = format!(
                "rsu:{}:{}:{chunk_index}",
                source.source_kind, source.source_id
            );
            let hash = content_hash(&format!("{}\n{}", source.title, chunk));
            let existing: Option<String> =
                sqlx::query_scalar("SELECT content_hash FROM reasoning_source_units WHERE id = ?")
                    .bind(&id)
                    .fetch_optional(pool)
                    .await
                    .map_err(sql_error)?;
            match existing.as_deref() {
                None => created += 1,
                Some(value) if value == hash => unchanged += 1,
                Some(_) => {
                    updated += 1;
                    invalidated_source_units.push(id.clone());
                }
            }
            sqlx::query(
                r#"
                INSERT INTO reasoning_source_units (
                  id, source_kind, source_id, chunk_index, title, body,
                  citation_label, source_route, source_updated_at, content_hash,
                  access_state, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'included', ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                  title = excluded.title,
                  body = excluded.body,
                  citation_label = excluded.citation_label,
                  source_route = excluded.source_route,
                  source_updated_at = excluded.source_updated_at,
                  content_hash = excluded.content_hash,
                  access_state = 'included',
                  updated_at = excluded.updated_at
                "#,
            )
            .bind(&id)
            .bind(&source.source_kind)
            .bind(&source.source_id)
            .bind(chunk_index as i64)
            .bind(&source.title)
            .bind(&chunk)
            .bind(format!("{}: {}", source.source_kind, source.title))
            .bind(&source.route)
            .bind(&source.updated_at)
            .bind(&hash)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .map_err(sql_error)?;
            if existing.as_deref() != Some(hash.as_str()) {
                crate::entity_embeddings::enqueue(
                    pool,
                    crate::entity_embeddings::EntityKind::ReasoningSourceUnit,
                    &id,
                    0,
                )
                .await?;
            }
        }
    }

    let removed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reasoning_source_units WHERE access_state = 'deleted'",
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    invalidated_source_units.extend(
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM reasoning_source_units WHERE access_state = 'deleted'",
        )
        .fetch_all(pool)
        .await
        .map_err(sql_error)?,
    );
    for deleted_id in &invalidated_source_units {
        sqlx::query(
            "DELETE FROM entity_embeddings_pending WHERE entity_kind = 'reasoning_source_unit' AND entity_id = ? AND EXISTS (SELECT 1 FROM reasoning_source_units WHERE id = ? AND access_state = 'deleted')",
        )
        .bind(deleted_id)
        .bind(deleted_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
        sqlx::query(
            "DELETE FROM entity_embeddings WHERE entity_kind = 'reasoning_source_unit' AND entity_id = ? AND EXISTS (SELECT 1 FROM reasoning_source_units WHERE id = ? AND access_state = 'deleted')",
        )
        .bind(deleted_id)
        .bind(deleted_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }
    invalidate_derived_artifacts(pool, &invalidated_source_units, &now).await?;
    let canonical_claims_synced = claims::sync_canonical_claims(pool).await?;
    let communities_synced = communities::refresh_canonical_communities(pool).await?;

    Ok(ReasoningIndexSummary {
        source_units_created: created,
        source_units_updated: updated,
        source_units_unchanged: unchanged,
        source_units_removed: removed,
        canonical_claims_synced,
        communities_synced,
        candidate_claims_staged: 0,
    })
}

async fn invalidate_derived_artifacts(
    pool: &SqlitePool,
    source_unit_ids: &[String],
    now: &str,
) -> Result<(), String> {
    for source_unit_id in source_unit_ids {
        let pattern = format!("%{source_unit_id}%");
        sqlx::query(
            r#"DELETE FROM claim_versions WHERE status = 'pending' AND claim_key IN (
               SELECT claim_key FROM claim_versions
               WHERE status = 'approved' AND generated_by != 'canonical' AND evidence_json LIKE ?
            )"#,
        )
        .bind(&pattern)
        .execute(pool)
        .await
        .map_err(sql_error)?;
        sqlx::query(
            r#"UPDATE claim_versions SET status = 'pending', contradiction_state = 'possible',
               updated_at = ?, reviewed_at = NULL
               WHERE status = 'approved' AND generated_by != 'canonical' AND evidence_json LIKE ?"#,
        )
        .bind(now)
        .bind(&pattern)
        .execute(pool)
        .await
        .map_err(sql_error)?;
        sqlx::query(
            r#"UPDATE community_reports SET status = 'stale', updated_at = ?
               WHERE status = 'approved' AND evidence_json LIKE ?"#,
        )
        .bind(now)
        .bind(pattern)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }
    Ok(())
}

pub(super) async fn selected_sources(
    pool: &SqlitePool,
    query: &str,
    scope: &ReasoningScope,
    resolved: &ResolvedScope,
    limit: i64,
    api_key: &str,
) -> Result<Vec<ReasoningSourceUnit>, String> {
    let mut rows = sqlx::query_as::<_, ReasoningSourceUnit>(
        r#"
        SELECT id, source_kind, source_id, chunk_index, title, body, citation_label,
               source_route, source_updated_at, content_hash, access_state, created_at, updated_at
        FROM reasoning_source_units
        WHERE access_state = 'included'
        ORDER BY source_updated_at DESC, updated_at DESC
        LIMIT 500
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    let query_terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|term| term.len() > 2)
        .map(|term| term.to_ascii_lowercase())
        .collect();
    rows.retain(|row| matches_scope(row, scope, resolved));
    let keys = rows
        .iter()
        .map(|row| ("reasoning_source_unit".to_string(), row.id.clone()))
        .collect::<Vec<_>>();
    let stored_embeddings = crate::entity_embeddings::load_embeddings_for(pool, &keys)
        .await
        .unwrap_or_default();
    let embeddings = if stored_embeddings.is_empty() {
        None
    } else {
        crate::gemini::embed_retrieval_query(Some(pool), api_key, query)
            .await
            .ok()
            .and_then(|query_vector| {
                let norm = crate::entity_embeddings::vector_norm(&query_vector.values);
                (norm > 0.0).then_some((query_vector.values, norm))
            })
    };
    rows.sort_by_key(|row| {
        let semantic = embeddings.as_ref().and_then(|(query_vector, query_norm)| {
            stored_embeddings
                .get(&("reasoning_source_unit".to_string(), row.id.clone()))
                .map(|(vector, norm)| {
                    crate::entity_embeddings::cosine(query_vector, *query_norm, vector, *norm)
                })
        });
        std::cmp::Reverse(score_source(row, &query_terms, semantic))
    });
    rows.truncate(limit.max(1) as usize);
    Ok(rows)
}

fn matches_scope(
    row: &ReasoningSourceUnit,
    scope: &ReasoningScope,
    resolved: &ResolvedScope,
) -> bool {
    // User-driven exclusions always win.
    if resolved.excluded_source_unit_ids.contains(&row.id) {
        return false;
    }
    if !scope.source_kinds.is_empty() && !scope.source_kinds.contains(&row.source_kind) {
        return false;
    }
    let quarter_range = scope.quarter.as_deref().and_then(quarter_date_range);
    let from = scope
        .date_from
        .as_deref()
        .or_else(|| quarter_range.as_ref().map(|dates| dates.0.as_str()));
    let to = scope
        .date_to
        .as_deref()
        .or_else(|| quarter_range.as_ref().map(|dates| dates.1.as_str()));
    if let Some(from) = from {
        if row
            .source_updated_at
            .as_deref()
            .is_some_and(|date| date < from)
        {
            return false;
        }
    }
    if let Some(to) = to {
        if row
            .source_updated_at
            .as_deref()
            .is_some_and(|date| date > to)
        {
            return false;
        }
    }
    // Strict scope: a targeted report only sees source units in the resolved
    // allow-list. Cross-initiative content drops silently — that's what stops
    // a Notes Project report from quoting Articles Project emails.
    if resolved.is_targeted {
        return resolved
            .source_unit_membership
            .contains(&(row.source_kind.clone(), row.source_id.clone()));
    }
    // Untargeted: capture-style and process artifacts are never auto-included
    // (the user's rule: thoughts are not work). Date/source-kind filters above
    // still apply.
    !matches!(
        row.source_kind.as_str(),
        "capture" | "ask_turn" | "memory"
    )
}

fn quarter_date_range(value: &str) -> Option<(String, String)> {
    let normalized = value.trim().to_ascii_uppercase().replace('-', " ");
    let mut quarter = None;
    let mut year = None;
    for part in normalized.split_whitespace() {
        if let Some(digit) = part.strip_prefix('Q') {
            quarter = digit
                .parse::<u32>()
                .ok()
                .filter(|value| (1..=4).contains(value));
        } else if part.len() == 4 {
            year = part.parse::<u32>().ok();
        }
    }
    let (quarter, year) = (quarter?, year?);
    let start_month = (quarter - 1) * 3 + 1;
    let end_month = start_month + 2;
    let end_day = if end_month == 3 || end_month == 12 {
        31
    } else {
        30
    };
    Some((
        format!("{year:04}-{start_month:02}-01"),
        format!("{year:04}-{end_month:02}-{end_day:02}"),
    ))
}

fn score_source(row: &ReasoningSourceUnit, query_terms: &[String], semantic: Option<f32>) -> i64 {
    let haystack = format!("{} {}", row.title, row.body).to_ascii_lowercase();
    let hits = query_terms
        .iter()
        .filter(|term| haystack.contains(term.as_str()))
        .count() as i64;
    hits * 100
        + semantic
            .map(|score| (score.max(0.0) * 80.0) as i64)
            .unwrap_or(0)
        + if row.source_kind == "deliverable" {
            10
        } else {
            0
        }
}

async fn load_sources(pool: &SqlitePool) -> Result<Vec<RawSource>, String> {
    let mut rows = Vec::new();
    macro_rules! append {
        ($sql:expr) => {
            rows.extend(
                sqlx::query_as::<_, RawSource>($sql)
                    .fetch_all(pool)
                    .await
                    .map_err(sql_error)?,
            );
        };
    }
    append!("SELECT 'initiative' source_kind, id source_id, title, framing body, '/initiatives/' || id route, updated_at FROM initiatives");
    append!("SELECT 'deliverable' source_kind, id source_id, title, claim body, '/deliverables/' || id route, updated_at FROM deliverables WHERE state != 'killed'");
    append!("SELECT 'stakeholder' source_kind, id source_id, name title, COALESCE(role,'') || '\n' || COALESCE(notes,'') body, '/stakeholders/' || id route, NULL updated_at FROM stakeholders");
    append!("SELECT 'conversation' source_kind, id source_id, COALESCE(title, 'Conversation') title, COALESCE(summary, '') body, NULL route, ingested_at updated_at FROM conversations");
    append!("SELECT 'capture' source_kind, id source_id, 'Capture' title, body, '/captures' route, updated_at FROM captures WHERE status != 'dismissed'");
    append!("SELECT 'memory' source_kind, id source_id, title, body, '/context' route, updated_at FROM memories WHERE status = 'active' AND deleted_at IS NULL");
    append!("SELECT 'meeting' source_kind, id source_id, title, COALESCE(summary,'') || '\n' || COALESCE(key_decisions,'') || '\n' || COALESCE(transcript,'') body, '/meetings/' || id route, updated_at FROM meetings WHERE status != 'error'");
    append!("SELECT 'email_thread' source_kind, thread_id source_id, subject title, COALESCE(summary, snippet) body, '/email?thread=' || thread_id route, last_sync_at updated_at FROM gmail_threads");
    append!("SELECT 'email_message' source_kind, message_id source_id, subject title, COALESCE(plain_body, snippet) body, '/email?thread=' || thread_id route, synced_at updated_at FROM gmail_messages");
    append!("SELECT 'calendar_event' source_kind, id source_id, title, COALESCE(description,'') body, '/week' route, start_date updated_at FROM gcal_events");
    append!("SELECT 'file' source_kind, id source_id, name title, COALESCE(description, '') body, '/files' route, updated_at FROM files WHERE COALESCE(drive_trashed, 0) = 0");
    append!("SELECT 'ask_turn' source_kind, id source_id, question title, answer body, '/ask' route, updated_at FROM ask_turns WHERE status = 'done'");
    Ok(rows)
}

fn chunk_text(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec!["No source text captured.".to_string()];
    }
    let chars: Vec<char> = trimmed.chars().collect();
    chars
        .chunks(CHUNK_CHARS)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

pub(super) fn content_hash(value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(value.as_bytes());
    format!("{:x}", digest.finalize())
}

pub(super) fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Ulid::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_empty_sources_into_a_citable_placeholder() {
        assert_eq!(chunk_text("  "), vec!["No source text captured."]);
    }

    #[test]
    fn content_hash_is_stable() {
        assert_eq!(content_hash("same"), content_hash("same"));
        assert_ne!(content_hash("same"), content_hash("different"));
    }

    #[test]
    fn q3_scope_maps_to_calendar_dates() {
        assert_eq!(
            quarter_date_range("Q3 2026"),
            Some(("2026-07-01".to_string(), "2026-09-30".to_string()))
        );
    }
}
