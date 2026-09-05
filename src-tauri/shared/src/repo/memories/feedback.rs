use std::collections::BTreeMap;

use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{
        DeliverableFilters, ListMemoryEventsFilters, MemoryConsolidationResult, MemoryEvent,
        MemoryKind,
    },
};

use super::super::{
    empty_as_placeholder, list_conversations, list_deliverables, list_initiatives_by_title,
    list_stakeholders, now_utc, optional_context_fragment,
};
use super::{record_memory_event, upsert_generated_memory, DeliverableMemoryImportance};

pub async fn consolidate_memories(pool: &SqlitePool) -> Result<MemoryConsolidationResult, String> {
    let run_id = Ulid::new().to_string();
    let started_at = now_utc();
    sqlx::query(
        r#"
        INSERT INTO memory_consolidation_runs (id, source, status, started_at)
        VALUES (?, 'workspace', 'running', ?)
        "#,
    )
    .bind(&run_id)
    .bind(&started_at)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    let result = consolidate_memories_inner(pool, &run_id).await;
    let completed_at = now_utc();
    match &result {
        Ok(summary) => {
            sqlx::query(
                r#"
                UPDATE memory_consolidation_runs
                SET status = 'succeeded',
                    created_count = ?,
                    updated_count = ?,
                    archived_count = ?,
                    completed_at = ?
                WHERE id = ?
                "#,
            )
            .bind(summary.created_count)
            .bind(summary.updated_count)
            .bind(summary.archived_count)
            .bind(&completed_at)
            .bind(&run_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
        Err(error) => {
            sqlx::query(
                r#"
                UPDATE memory_consolidation_runs
                SET status = 'failed', error_message = ?, completed_at = ?
                WHERE id = ?
                "#,
            )
            .bind(error)
            .bind(&completed_at)
            .bind(&run_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }

    result
}

async fn consolidate_memories_inner(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<MemoryConsolidationResult, String> {
    let mut created_count = 0;
    let mut updated_count = 0;
    let archived_count = 0;
    let mut touched = Vec::new();

    let initiatives = list_initiatives_by_title(pool).await?;
    let stakeholders = list_stakeholders(pool).await?;
    let conversations = list_conversations(pool).await?;
    let deliverables = list_deliverables(pool, DeliverableFilters::default()).await?;

    let live_initiatives = initiatives
        .iter()
        .filter(|initiative| initiative.status == "live" || initiative.status == "paused")
        .map(|initiative| initiative.title.as_str())
        .collect::<Vec<_>>();
    let work_profile = format!(
        "The user's current work system contains {} initiative(s), {} deliverable(s), {} stakeholder(s), and {} ingested conversation(s). Active initiatives: {}.",
        initiatives.len(),
        deliverables.len(),
        stakeholders.len(),
        conversations.len(),
        if live_initiatives.is_empty() { "none".to_string() } else { live_initiatives.join(", ") },
    );
    let (memory, created) = upsert_generated_memory(
        pool,
        MemoryKind::Semantic,
        "Current work profile",
        &work_profile,
        "semantic:workspace-profile",
        "workspace",
        None,
        &["workspace", "profile", "continuity"],
        &["Consolidated from current Trace workspace records."],
        0.90,
        0.95,
    )
    .await?;
    increment_created_updated(created, &mut created_count, &mut updated_count);
    touched.push(memory);

    for initiative in initiatives {
        let body = format!(
            "The user is tracking initiative '{}'. Status: {}. Why it matters / framing: {}",
            initiative.title,
            initiative.status,
            empty_as_placeholder(&initiative.framing)
        );
        let key = format!("semantic:initiative:{}", initiative.id);
        let (memory, created) = upsert_generated_memory(
            pool,
            MemoryKind::Semantic,
            &format!("Initiative: {}", initiative.title),
            &body,
            &key,
            "initiative",
            Some(&initiative.id),
            &["initiative", "work"],
            &[&format!("Source initiative id: {}", initiative.id)],
            0.88,
            if initiative.status == "live" {
                0.86
            } else {
                0.62
            },
        )
        .await?;
        increment_created_updated(created, &mut created_count, &mut updated_count);
        touched.push(memory);
    }

    for stakeholder in stakeholders {
        let body = format!(
            "The user works with {}{}{}.",
            stakeholder.name,
            optional_context_fragment(" role", &stakeholder.role),
            optional_context_fragment(" notes", &stakeholder.notes)
        );
        let key = format!("semantic:stakeholder:{}", stakeholder.id);
        let (memory, created) = upsert_generated_memory(
            pool,
            MemoryKind::Semantic,
            &format!("Stakeholder: {}", stakeholder.name),
            &body,
            &key,
            "stakeholder",
            Some(&stakeholder.id),
            &["stakeholder", "relationship"],
            &[&format!("Source stakeholder id: {}", stakeholder.id)],
            0.84,
            0.70,
        )
        .await?;
        increment_created_updated(created, &mut created_count, &mut updated_count);
        touched.push(memory);
    }

    for conversation in conversations {
        let title = conversation
            .title
            .as_deref()
            .unwrap_or("Untitled conversation");
        let body = format!(
            "Conversation '{}'. Occurred at: {}. Summary: {}",
            title,
            conversation.occurred_at.as_deref().unwrap_or("unknown"),
            conversation
                .summary
                .as_deref()
                .unwrap_or("No summary saved.")
        );
        let key = format!("episodic:conversation:{}", conversation.id);
        let (memory, created) = upsert_generated_memory(
            pool,
            MemoryKind::Episodic,
            &format!("Conversation: {title}"),
            &body,
            &key,
            "conversation",
            Some(&conversation.id),
            &["conversation", "episode"],
            &[&format!("Source conversation id: {}", conversation.id)],
            0.78,
            0.68,
        )
        .await?;
        increment_created_updated(created, &mut created_count, &mut updated_count);
        touched.push(memory);
    }

    for deliverable in &deliverables {
        let kind = if deliverable.state == "shipped" {
            MemoryKind::Episodic
        } else {
            MemoryKind::Semantic
        };
        let body = format!(
            "The user {} deliverable '{}'. Type: {}. State: {}. Goal/claim: {}{}{}",
            if deliverable.state == "shipped" {
                "shipped"
            } else {
                "is working on"
            },
            deliverable.title,
            deliverable.deliverable_type,
            deliverable.state,
            empty_as_placeholder(&deliverable.claim),
            deliverable
                .stakeholder_name
                .as_deref()
                .map(|name| format!(". Stakeholder(s): {name}"))
                .unwrap_or_default(),
            deliverable
                .deadline
                .as_deref()
                .map(|deadline| format!(". Deadline: {deadline}"))
                .unwrap_or_default()
        );
        let key = format!("{}:deliverable:{}", kind.as_str(), deliverable.id);
        let (memory, created) = upsert_generated_memory(
            pool,
            kind,
            &format!("Deliverable: {}", deliverable.title),
            &body,
            &key,
            "deliverable",
            Some(&deliverable.id),
            &["deliverable", "work-item"],
            &[&format!("Source deliverable id: {}", deliverable.id)],
            0.82,
            deliverable.importance_hint(),
        )
        .await?;
        increment_created_updated(created, &mut created_count, &mut updated_count);
        touched.push(memory);
    }

    let mut type_counts = BTreeMap::<String, i64>::new();
    for deliverable in &deliverables {
        *type_counts
            .entry(deliverable.deliverable_type.clone())
            .or_default() += 1;
    }
    if !type_counts.is_empty() {
        let patterns = type_counts
            .iter()
            .map(|(kind, count)| format!("{kind}: {count}"))
            .collect::<Vec<_>>()
            .join(", ");
        let body = format!(
            "The user's work patterns are visible from deliverable history. Common deliverable types: {patterns}. Use this to adapt task breakdowns, examples, and default workflows."
        );
        let (memory, created) = upsert_generated_memory(
            pool,
            MemoryKind::Procedural,
            "Observed work patterns",
            &body,
            "procedural:observed-work-patterns",
            "workspace",
            None,
            &["procedure", "workflow", "pattern"],
            &["Consolidated from deliverable type history."],
            0.72,
            0.74,
        )
        .await?;
        increment_created_updated(created, &mut created_count, &mut updated_count);
        touched.push(memory);
    }

    record_memory_event(
        pool,
        None,
        "consolidated",
        serde_json::json!({
            "run_id": run_id,
            "created_count": created_count,
            "updated_count": updated_count,
            "archived_count": archived_count
        }),
    )
    .await?;

    Ok(MemoryConsolidationResult {
        run_id: run_id.to_string(),
        created_count,
        updated_count,
        archived_count,
        memories: touched,
    })
}

pub async fn record_memory_feedback(
    pool: &SqlitePool,
    retrieval_id: &str,
    feedback: &str,
) -> Result<(), String> {
    let trimmed = feedback.trim();
    if trimmed.is_empty() {
        return Err("feedback is required".to_string());
    }
    let cleaned = match trimmed {
        "useful" | "not_useful" | "wrong" => trimmed.to_string(),
        _ => return Err("feedback must be useful, not_useful, or wrong".to_string()),
    };
    let result = sqlx::query("UPDATE memory_retrievals SET feedback = ? WHERE id = ?")
        .bind(&cleaned)
        .bind(retrieval_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    if result.rows_affected() == 0 {
        return Err("retrieval not found".to_string());
    }

    let row: Option<(String,)> =
        sqlx::query_as("SELECT memory_ids_json FROM memory_retrievals WHERE id = ?")
            .bind(retrieval_id)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?;
    if let Some((memory_ids_json,)) = row {
        let ids: Vec<String> = serde_json::from_str(&memory_ids_json).unwrap_or_default();
        match cleaned.as_str() {
            "useful" => {
                for id in &ids {
                    let _ = sqlx::query(
                        "UPDATE memories SET success_count = success_count + 1 WHERE id = ?",
                    )
                    .bind(id)
                    .execute(pool)
                    .await;
                }
            }
            "wrong" => {
                for id in &ids {
                    let _ = sqlx::query(
                        "UPDATE memories SET contradiction_count = contradiction_count + 1 WHERE id = ?",
                    )
                    .bind(id)
                    .execute(pool)
                    .await;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub async fn list_memory_events(
    pool: &SqlitePool,
    filters: ListMemoryEventsFilters,
) -> Result<Vec<MemoryEvent>, String> {
    let limit = filters.limit.unwrap_or(120).clamp(1, 500);
    let rows = if let Some(memory_id) = filters.memory_id.as_deref() {
        sqlx::query_as::<_, MemoryEvent>(
            r#"
            SELECT id, memory_id, action, detail_json, created_at
            FROM memory_events
            WHERE memory_id = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(memory_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    } else {
        sqlx::query_as::<_, MemoryEvent>(
            r#"
            SELECT id, memory_id, action, detail_json, created_at
            FROM memory_events
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    };
    Ok(rows)
}
fn increment_created_updated(created: bool, created_count: &mut i64, updated_count: &mut i64) {
    if created {
        *created_count += 1;
    } else {
        *updated_count += 1;
    }
}


