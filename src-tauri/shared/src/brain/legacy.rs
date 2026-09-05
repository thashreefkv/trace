use std::{
    collections::{BTreeMap, BTreeSet, HashSet, VecDeque},
    path::{Path, PathBuf},
};

use chrono::{Duration, NaiveDate, SecondsFormat, Utc};
use kuzu::{Connection, Database, SystemConfig, Value};
use serde_json::json;
use sqlx::{FromRow, SqlitePool};

use crate::models::{
    BrainBrief, BrainGraphFilters, BrainInferenceRecord, BrainStatus, BrainTemplateInput,
    BrainTemplateKind, BrainTemplateResult, WorkGraph, WorkGraphEdge, WorkGraphFilters,
    WorkGraphNode,
};

use super::inferences::{add_inferences, refresh_brain_inferences};
use super::projection::{
    read_meta, remove_brain_path, write_meta, BrainEntity, BrainMeta, BrainProjection,
    BrainRelation, BRAIN_FILE_NAME, BRAIN_SCHEMA_VERSION,
};
use super::reasoning::add_reasoning_artifacts;
use super::retrieval::graph_ai_context;
use super::state::{rebuild_lock, BRAIN_DIRTY, BRAIN_REBUILDING};
use super::templates::run_brain_template;

fn brain_lock_path(brain_path: &Path) -> PathBuf {
    let mut file_name = brain_path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from(BRAIN_FILE_NAME));
    file_name.push(".lock");
    brain_path.with_file_name(file_name)
}

fn write_projection_locked(
    path: &Path,
    projection: BrainProjection,
) -> Result<BrainStatus, String> {
    use fs2::FileExt;

    let lock_path = brain_lock_path(path);
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create brain directory: {error}"))?;
    }
    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| format!("failed to open brain lock file: {error}"))?;
    lock_file
        .lock_exclusive()
        .map_err(|error| format!("failed to lock brain: {error}"))?;
    let result = write_projection(path, projection);
    let _ = FileExt::unlock(&lock_file);
    result
}

pub async fn rebuild_brain(pool: &SqlitePool, path: &Path) -> Result<BrainStatus, String> {
    let _guard = rebuild_lock().lock().await;
    let projection = build_projection(pool).await?;
    let path_owned = path.to_path_buf();
    let status =
        tokio::task::spawn_blocking(move || write_projection_locked(&path_owned, projection))
            .await
            .map_err(|error| format!("brain rebuild task failed: {error}"))??;
    // Wipe cached layouts — they were keyed on the previous graph_version and
    // would point at nodes that may no longer exist.
    if let Err(error) = crate::brain::invalidate_brain_layouts(pool).await {
        eprintln!("[brain] layout cache invalidation failed: {error}");
    }
    Ok(status)
}

/// Coalescing fire-and-forget rebuild. Returns immediately. If a rebuild is
/// already running, just marks the brain dirty so one more rebuild runs after
/// the current one finishes — N concurrent write tools in a single Ask turn
/// collapse to at most two sequential rebuilds instead of N.
///
/// Use this from hot paths (e.g. tool dispatch). Use `rebuild_brain` only when
/// you need to await the result.
pub fn request_rebuild(pool: SqlitePool, path: std::path::PathBuf) {
    use std::sync::atomic::Ordering;
    BRAIN_DIRTY.store(true, Ordering::Release);
    // Already running? The current rebuild will pick up the dirty bit and
    // re-run when it finishes.
    if BRAIN_REBUILDING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    crate::runtime::spawn(async move {
        loop {
            BRAIN_DIRTY.store(false, Ordering::Release);
            let _ = rebuild_brain(&pool, &path).await;
            if !BRAIN_DIRTY.load(Ordering::Acquire) {
                BRAIN_REBUILDING.store(false, Ordering::Release);
                // A late write between the load and the store would set
                // DIRTY=true; double-check so we don't drop it.
                if BRAIN_DIRTY.load(Ordering::Acquire)
                    && BRAIN_REBUILDING
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                {
                    continue;
                }
                break;
            }
        }
    });
}

pub async fn get_brain_graph(
    pool: &SqlitePool,
    path: &Path,
    filters: BrainGraphFilters,
) -> Result<WorkGraph, String> {
    if !path.exists() {
        rebuild_brain(pool, path).await?;
    }

    match read_graph(path, filters.clone()).await {
        Ok(graph) => Ok(graph),
        Err(first_error) => {
            // `rebuild_brain` already calls `remove_brain_path` from inside
            // the serialization lock — doing it again here would race with
            // any other in-flight rebuild and corrupt its Kuzu files.
            rebuild_brain(pool, path).await?;
            read_graph(path, filters).await.map_err(|second_error| {
                format!("{second_error}; after rebuild from: {first_error}")
            })
        }
    }
}

pub async fn get_work_context_graph(
    pool: &SqlitePool,
    path: &Path,
    filters: WorkGraphFilters,
) -> Result<WorkGraph, String> {
    get_brain_graph(pool, path, filters.into()).await
}

pub async fn get_daily_brain_brief(pool: &SqlitePool, path: &Path) -> Result<BrainBrief, String> {
    let focus_today = run_brain_template(
        pool,
        path,
        BrainTemplateInput {
            template: BrainTemplateKind::FocusToday,
            focus_entity_id: None,
            limit: Some(36),
        },
    )
    .await?;
    let blocked_or_waiting = run_brain_template(
        pool,
        path,
        BrainTemplateInput {
            template: BrainTemplateKind::BlockedWork,
            focus_entity_id: None,
            limit: Some(36),
        },
    )
    .await?;
    let email_followups = run_brain_template(
        pool,
        path,
        BrainTemplateInput {
            template: BrainTemplateKind::EmailFollowups,
            focus_entity_id: None,
            limit: Some(36),
        },
    )
    .await?;
    let stale_work = run_brain_template(
        pool,
        path,
        BrainTemplateInput {
            template: BrainTemplateKind::StaleWork,
            focus_entity_id: None,
            limit: Some(36),
        },
    )
    .await?;

    let pending = sqlx::query_as::<_, BrainInferenceRecord>(
        r#"
        SELECT id, source_kind, source_id, relation_kind, target_kind, target_id,
               confidence, rationale, evidence_json, status, generated_by,
               created_at, updated_at, reviewed_at,
               template, superseded_by, supersede_reason
        FROM brain_inferences
        WHERE status = 'pending'
        ORDER BY confidence DESC, updated_at DESC
        LIMIT 8
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?
    .into_iter()
    .map(|row| serde_json::to_value(row).unwrap_or_else(|_| json!({})))
    .collect::<Vec<_>>();

    let generated_at = now_utc();
    let markdown = brain_brief_markdown(
        &generated_at,
        &focus_today,
        &blocked_or_waiting,
        &email_followups,
        &stale_work,
        &pending,
    );

    Ok(BrainBrief {
        generated_at,
        focus_today,
        blocked_or_waiting,
        email_followups,
        stale_work,
        inferences_to_review: pending,
        markdown,
    })
}

pub async fn tool_get_daily_brain_brief(pool: &SqlitePool, path: &Path) -> serde_json::Value {
    match get_daily_brain_brief(pool, path).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

async fn build_projection(pool: &SqlitePool) -> Result<BrainProjection, String> {
    let mut projection = BrainProjection::default();
    refresh_brain_inferences(pool).await?;
    add_initiatives(pool, &mut projection).await?;
    add_stakeholders(pool, &mut projection).await?;
    add_deliverables(pool, &mut projection).await?;
    add_deliverable_children(pool, &mut projection).await?;
    add_labels(pool, &mut projection).await?;
    add_conversations(pool, &mut projection).await?;
    add_captures(pool, &mut projection).await?;
    add_memories(pool, &mut projection).await?;
    add_meetings(pool, &mut projection).await?;
    add_week_plan(pool, &mut projection).await?;
    add_calendar(pool, &mut projection).await?;
    add_gmail(pool, &mut projection).await?;
    add_ask(pool, &mut projection).await?;
    add_work_intake(pool, &mut projection).await?;
    add_files(pool, &mut projection).await?;
    add_identity_links(pool, &mut projection).await?;
    add_cross_links(pool, &mut projection).await?;
    add_open_loops_and_attention(pool, &mut projection).await?;
    add_inferences(pool, &mut projection).await?;
    add_reasoning_artifacts(pool, &mut projection).await?;
    Ok(projection)
}

#[derive(Debug, FromRow)]
struct InitiativeRow {
    id: String,
    title: String,
    framing: String,
    status: String,
    created_at: String,
    updated_at: String,
}

async fn add_initiatives(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let rows = sqlx::query_as::<_, InitiativeRow>(
        "SELECT id, title, framing, status, created_at, updated_at FROM initiatives",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in rows {
        projection.push_node(entity(
            "initiative",
            "initiatives",
            &row.id,
            &row.title,
            &row.framing,
            &row.status,
            Some(format!("/initiatives/{}", row.id)),
            &row.created_at,
            &row.updated_at,
            0.85,
            json!({ "status": row.status }),
        ));
    }

    let sections = sqlx::query_as::<_, InitiativeSectionRow>(
        r#"
        SELECT id, initiative_id, title, position, created_at, updated_at
        FROM initiative_sections
        ORDER BY initiative_id, position
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in sections {
        let section_id = graph_node_id("initiative_section", &row.id);
        projection.push_node(entity(
            "initiative_section",
            "initiative_sections",
            &row.id,
            &row.title,
            &format!("Initiative section at position {}.", row.position),
            "",
            Some(format!("/initiatives/{}", row.initiative_id)),
            &row.created_at,
            &row.updated_at,
            0.45,
            json!({ "initiative_id": row.initiative_id, "position": row.position }),
        ));
        projection.push_edge(relation(
            &graph_node_id("initiative", &row.initiative_id),
            &section_id,
            "CONTAINS",
            "contains section",
            0.9,
            json!({ "source": "initiative_sections" }),
            &row.created_at,
            &row.updated_at,
            json!({}),
        ));
    }

    Ok(())
}

#[derive(Debug, FromRow)]
struct InitiativeSectionRow {
    id: String,
    initiative_id: String,
    title: String,
    position: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct StakeholderRow {
    id: String,
    name: String,
    email: String,
    role: String,
    notes: String,
}

async fn add_stakeholders(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let rows = sqlx::query_as::<_, StakeholderRow>(
        r#"
        SELECT id, name, COALESCE(email, '') AS email, COALESCE(role, '') AS role, COALESCE(notes, '') AS notes
        FROM stakeholders
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in rows {
        let notes_summary = truncate(&row.notes, 600);
        let summary = join_nonempty([
            row.role.as_str(),
            row.email.as_str(),
            notes_summary.as_str(),
        ]);
        projection.push_node(entity(
            "stakeholder",
            "stakeholders",
            &row.id,
            &row.name,
            &summary,
            &row.role,
            Some(format!("/stakeholders/{}", row.id)),
            "",
            "",
            0.75,
            json!({ "email": row.email, "role": row.role }),
        ));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct DeliverableRow {
    id: String,
    title: String,
    deliverable_type: String,
    state: String,
    claim: String,
    artifact_url: Option<String>,
    conversation_id: Option<String>,
    stakeholder_id: Option<String>,
    created_at: String,
    shipped_at: Option<String>,
    updated_at: String,
    deadline: Option<String>,
    is_focused: i64,
    effort: Option<i64>,
    impact: Option<i64>,
    blocker_reason: Option<String>,
    start_date: Option<String>,
    section_id: Option<String>,
    priority: Option<String>,
}

async fn add_deliverables(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let rows = sqlx::query_as::<_, DeliverableRow>(
        r#"
        SELECT id, title, type AS deliverable_type, state, claim, artifact_url, conversation_id,
               stakeholder_id, created_at, shipped_at, updated_at, deadline, is_focused,
               effort, impact, blocker_reason, start_date, section_id, priority
        FROM deliverables
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in rows {
        let mut summary_parts = vec![
            format!("{} deliverable in {}.", row.deliverable_type, row.state),
            truncate(&row.claim, 900),
        ];
        if let Some(deadline) = &row.deadline {
            summary_parts.push(format!("Deadline: {deadline}."));
        }
        if let Some(blocker) = &row.blocker_reason {
            summary_parts.push(format!("Blocked by: {}.", truncate(blocker, 320)));
        }
        let importance: f64 = 0.65
            + if row.is_focused != 0 { 0.25 } else { 0.0 }
            + if row.blocker_reason.is_some() {
                0.1
            } else {
                0.0
            };
        projection.push_node(entity(
            "deliverable",
            "deliverables",
            &row.id,
            &row.title,
            &join_nonempty(summary_parts.iter().map(String::as_str)),
            &row.state,
            Some(format!("/deliverables/{}", row.id)),
            &row.created_at,
            &row.updated_at,
            importance.min(1.0),
            json!({
                "type": &row.deliverable_type,
                "artifact_url": &row.artifact_url,
                "conversation_id": &row.conversation_id,
                "stakeholder_id": &row.stakeholder_id,
                "deadline": &row.deadline,
                "start_date": &row.start_date,
                "shipped_at": &row.shipped_at,
                "section_id": &row.section_id,
                "priority": &row.priority,
                "effort": &row.effort,
                "impact": &row.impact,
                "is_focused": row.is_focused != 0,
            }),
        ));

        if let Some(section_id) = &row.section_id {
            projection.push_edge(relation(
                &graph_node_id("initiative_section", section_id),
                &graph_node_id("deliverable", &row.id),
                "CONTAINS",
                "contains deliverable",
                0.8,
                json!({ "source": "deliverables.section_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
        if let Some(conversation_id) = &row.conversation_id {
            projection.push_edge(relation(
                &graph_node_id("conversation", conversation_id),
                &graph_node_id("deliverable", &row.id),
                "PRODUCED",
                "produced",
                0.8,
                json!({ "source": "deliverables.conversation_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
        if let Some(stakeholder_id) = &row.stakeholder_id {
            projection.push_edge(relation(
                &graph_node_id("stakeholder", stakeholder_id),
                &graph_node_id("deliverable", &row.id),
                "TARGETS",
                "targets",
                0.55,
                json!({ "source": "deliverables.stakeholder_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
    }

    let initiative_links = sqlx::query_as::<_, (String, String)>(
        "SELECT deliverable_id, initiative_id FROM deliverable_initiatives",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (deliverable_id, initiative_id) in initiative_links {
        projection.push_edge(relation(
            &graph_node_id("initiative", &initiative_id),
            &graph_node_id("deliverable", &deliverable_id),
            "CONTAINS",
            "contains deliverable",
            0.95,
            json!({ "source": "deliverable_initiatives" }),
            "",
            "",
            json!({}),
        ));
    }

    let stakeholder_links = sqlx::query_as::<_, (String, String)>(
        "SELECT deliverable_id, stakeholder_id FROM deliverable_stakeholders",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (deliverable_id, stakeholder_id) in stakeholder_links {
        projection.push_edge(relation(
            &graph_node_id("stakeholder", &stakeholder_id),
            &graph_node_id("deliverable", &deliverable_id),
            "TARGETS",
            "targets",
            0.85,
            json!({ "source": "deliverable_stakeholders" }),
            "",
            "",
            json!({}),
        ));
    }

    Ok(())
}

#[derive(Debug, FromRow)]
struct TaskRow {
    id: String,
    deliverable_id: String,
    title: String,
    status: String,
    due_date: Option<String>,
    display_order: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct NoteRow {
    id: String,
    deliverable_id: String,
    body: String,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct StateHistoryRow {
    id: String,
    deliverable_id: String,
    from_state: String,
    to_state: String,
    friction_note: Option<String>,
    moved_at: String,
}

async fn add_deliverable_children(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let tasks = sqlx::query_as::<_, TaskRow>(
        r#"
        SELECT id, deliverable_id, title, status, due_date, display_order, created_at, updated_at
        FROM deliverable_tasks
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in tasks {
        projection.push_node(entity(
            "task",
            "deliverable_tasks",
            &row.id,
            &row.title,
            &format!(
                "Task {} for deliverable {}{}.",
                row.status,
                row.deliverable_id,
                row.due_date
                    .as_ref()
                    .map(|date| format!(" due {date}"))
                    .unwrap_or_default()
            ),
            &row.status,
            Some(format!("/deliverables/{}", row.deliverable_id)),
            &row.created_at,
            &row.updated_at,
            if row.status == "done" { 0.35 } else { 0.65 },
            json!({ "deliverable_id": row.deliverable_id, "due_date": row.due_date, "display_order": row.display_order }),
        ));
        projection.push_edge(relation(
            &graph_node_id("deliverable", &row.deliverable_id),
            &graph_node_id("task", &row.id),
            "CONTAINS",
            "contains task",
            0.9,
            json!({ "source": "deliverable_tasks" }),
            &row.created_at,
            &row.updated_at,
            json!({}),
        ));
    }

    let notes = sqlx::query_as::<_, NoteRow>(
        "SELECT id, deliverable_id, body, created_at FROM deliverable_notes",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in notes {
        projection.push_node(entity(
            "note",
            "deliverable_notes",
            &row.id,
            &truncate(&row.body, 80),
            &truncate(&row.body, 1200),
            "",
            Some(format!("/deliverables/{}", row.deliverable_id)),
            &row.created_at,
            &row.created_at,
            0.5,
            json!({ "deliverable_id": row.deliverable_id }),
        ));
        projection.push_edge(relation(
            &graph_node_id("deliverable", &row.deliverable_id),
            &graph_node_id("note", &row.id),
            "CONTAINS",
            "contains note",
            0.7,
            json!({ "source": "deliverable_notes" }),
            &row.created_at,
            &row.created_at,
            json!({}),
        ));
    }

    let history = sqlx::query_as::<_, StateHistoryRow>(
        r#"
        SELECT id, deliverable_id, from_state, to_state, friction_note, moved_at
        FROM deliverable_state_history
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in history {
        let title = format!("{} -> {}", row.from_state, row.to_state);
        projection.push_node(entity(
            "state_history",
            "deliverable_state_history",
            &row.id,
            &title,
            row.friction_note.as_deref().unwrap_or("State change"),
            &row.to_state,
            Some(format!("/deliverables/{}", row.deliverable_id)),
            &row.moved_at,
            &row.moved_at,
            0.3,
            json!({ "deliverable_id": row.deliverable_id, "from_state": row.from_state, "to_state": row.to_state }),
        ));
        projection.push_edge(relation(
            &graph_node_id("deliverable", &row.deliverable_id),
            &graph_node_id("state_history", &row.id),
            "UPDATED_BY",
            "state change",
            0.45,
            json!({ "source": "deliverable_state_history" }),
            &row.moved_at,
            &row.moved_at,
            json!({}),
        ));
    }

    Ok(())
}

#[derive(Debug, FromRow)]
struct LabelRow {
    id: String,
    name: String,
    color: String,
}

async fn add_labels(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let labels = sqlx::query_as::<_, LabelRow>("SELECT id, name, color FROM labels")
        .fetch_all(pool)
        .await
        .map_err(sql_error)?;
    for row in labels {
        projection.push_node(entity(
            "label",
            "labels",
            &row.id,
            &row.name,
            &format!("Board label {}", row.name),
            "",
            None,
            "",
            "",
            0.35,
            json!({ "color": row.color }),
        ));
    }

    let links = sqlx::query_as::<_, (String, String)>(
        "SELECT deliverable_id, label_id FROM deliverable_labels",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (deliverable_id, label_id) in links {
        projection.push_edge(relation(
            &graph_node_id("deliverable", &deliverable_id),
            &graph_node_id("label", &label_id),
            "TAGGED_WITH",
            "tagged with",
            0.65,
            json!({ "source": "deliverable_labels" }),
            "",
            "",
            json!({}),
        ));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct ConversationRow {
    id: String,
    chat_url: String,
    title: Option<String>,
    summary: Option<String>,
    occurred_at: Option<String>,
    ingested_at: String,
}

async fn add_conversations(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let rows = sqlx::query_as::<_, ConversationRow>(
        "SELECT id, chat_url, title, summary, occurred_at, ingested_at FROM conversations",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in rows {
        let title = row
            .title
            .unwrap_or_else(|| "Untitled conversation".to_string());
        projection.push_node(entity(
            "conversation",
            "conversations",
            &row.id,
            &title,
            row.summary.as_deref().unwrap_or(""),
            "",
            Some(row.chat_url),
            row.occurred_at.as_deref().unwrap_or(""),
            &row.ingested_at,
            0.65,
            json!({}),
        ));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct CaptureRow {
    id: String,
    kind: String,
    body: String,
    status: String,
    promoted_deliverable_id: Option<String>,
    promoted_conversation_id: Option<String>,
    promoted_initiative_id: Option<String>,
    promoted_task_id: Option<String>,
    promoted_task_title: Option<String>,
    created_at: String,
    updated_at: String,
    promoted_at: Option<String>,
}

async fn add_captures(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let rows = sqlx::query_as::<_, CaptureRow>(
        r#"
        SELECT id, kind, body, status, promoted_deliverable_id, promoted_conversation_id,
               promoted_initiative_id, promoted_task_id, promoted_task_title,
               created_at, updated_at, promoted_at
        FROM captures
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in rows {
        projection.push_node(entity(
            "capture",
            "captures",
            &row.id,
            &truncate(&row.body, 80),
            &truncate(&row.body, 1200),
            &row.status,
            Some(format!("/captures?selected={}", row.id)),
            &row.created_at,
            &row.updated_at,
            if row.status == "inbox" { 0.7 } else { 0.45 },
            json!({ "kind": &row.kind, "promoted_at": &row.promoted_at }),
        ));
        let capture_node = graph_node_id("capture", &row.id);
        if let Some(deliverable_id) = &row.promoted_deliverable_id {
            projection.push_edge(relation(
                &capture_node,
                &graph_node_id("deliverable", deliverable_id),
                "PROMOTED_TO",
                "promoted to deliverable",
                0.9,
                json!({ "source": "captures.promoted_deliverable_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
        if let Some(initiative_id) = &row.promoted_initiative_id {
            projection.push_edge(relation(
                &capture_node,
                &graph_node_id("initiative", initiative_id),
                "PROMOTED_TO",
                "promoted to initiative",
                0.9,
                json!({ "source": "captures.promoted_initiative_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
        if let Some(task_id) = &row.promoted_task_id {
            projection.push_edge(relation(
                &capture_node,
                &graph_node_id("task", task_id),
                "PROMOTED_TO",
                "promoted to task",
                0.9,
                json!({
                    "source": "captures.promoted_task_id",
                    "title": &row.promoted_task_title,
                }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
        if let Some(conversation_id) = &row.promoted_conversation_id {
            projection.push_edge(relation(
                &capture_node,
                &graph_node_id("conversation", conversation_id),
                "PROMOTED_TO",
                "promoted to conversation",
                0.9,
                json!({ "source": "captures.promoted_conversation_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct MemoryRow {
    id: String,
    kind: String,
    status: String,
    scope: String,
    title: String,
    body: String,
    source: String,
    source_kind: Option<String>,
    source_id: Option<String>,
    confidence: f64,
    importance: f64,
    tags_json: String,
    evidence_json: String,
    created_at: String,
    updated_at: String,
    sensitivity: String,
    pinned: i64,
}

async fn add_memories(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let rows = sqlx::query_as::<_, MemoryRow>(
        r#"
        SELECT id, kind, status, scope, title, body, source, source_kind, source_id,
               confidence, importance, tags_json, evidence_json, created_at, updated_at,
               COALESCE(sensitivity, 'normal') AS sensitivity, COALESCE(pinned, 0) AS pinned
        FROM memories
        WHERE status != 'deleted'
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in rows {
        projection.push_node(entity(
            "memory",
            "memories",
            &row.id,
            &row.title,
            &row.body,
            &row.status,
            None,
            &row.created_at,
            &row.updated_at,
            row.importance,
            json!({
                "kind": &row.kind,
                "scope": &row.scope,
                "source": &row.source,
                "source_kind": &row.source_kind,
                "source_id": &row.source_id,
                "confidence": row.confidence,
                "tags_json": &row.tags_json,
                "evidence_json": &row.evidence_json,
                "sensitivity": &row.sensitivity,
                "pinned": row.pinned != 0,
            }),
        ));
        if let (Some(source_kind), Some(source_id)) = (&row.source_kind, &row.source_id) {
            if let Some(target) = source_node_id(source_kind, source_id) {
                projection.push_edge(relation(
                    &graph_node_id("memory", &row.id),
                    &target,
                    "SOURCE_OF",
                    "source",
                    row.confidence.clamp(0.1, 1.0),
                    json!({ "source": "memories.source_kind/source_id" }),
                    &row.created_at,
                    &row.updated_at,
                    json!({}),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct MeetingRow {
    id: String,
    title: String,
    date: String,
    duration_secs: Option<i64>,
    summary: Option<String>,
    key_decisions: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct MeetingActionRow {
    id: String,
    meeting_id: String,
    kind: String,
    target_id: Option<String>,
    target_title: Option<String>,
    body: String,
    applied: i64,
    created_at: String,
    payload: Option<String>,
}

#[derive(Debug, FromRow)]
struct MeetingStakeholderRow {
    meeting_id: String,
    stakeholder_id: String,
}

#[derive(Debug, FromRow)]
struct InitiativeNoteRow {
    id: String,
    initiative_id: String,
    body: String,
    created_at: String,
}

async fn add_meetings(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let meetings = sqlx::query_as::<_, MeetingRow>(
        r#"
        SELECT id, title, date, duration_secs, summary, key_decisions, status, created_at, updated_at
        FROM meetings
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in meetings {
        let summary = join_nonempty([
            row.summary.as_deref().unwrap_or(""),
            row.key_decisions.as_deref().unwrap_or(""),
        ]);
        projection.push_node(entity(
            "meeting",
            "meetings",
            &row.id,
            &row.title,
            &summary,
            &row.status,
            Some(format!("/meetings/{}", row.id)),
            &row.created_at,
            &row.updated_at,
            0.7,
            json!({ "date": row.date, "duration_secs": row.duration_secs }),
        ));
    }

    let stakeholders = sqlx::query_as::<_, MeetingStakeholderRow>(
        "SELECT meeting_id, stakeholder_id FROM meeting_stakeholders",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in stakeholders {
        projection.push_edge(relation(
            &graph_node_id("meeting", &row.meeting_id),
            &graph_node_id("stakeholder", &row.stakeholder_id),
            "ATTENDED_BY",
            "attended by",
            0.8,
            json!({ "source": "meeting_stakeholders" }),
            "",
            "",
            json!({}),
        ));
    }

    let actions = sqlx::query_as::<_, MeetingActionRow>(
        r#"
        SELECT id, meeting_id, kind, target_id, target_title, body, applied, created_at, payload
        FROM meeting_actions
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in actions {
        let title = row
            .target_title
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(row.body.as_str());
        let status = if row.applied != 0 {
            "applied"
        } else {
            "pending"
        };
        projection.push_node(entity(
            "meeting_action",
            "meeting_actions",
            &row.id,
            &truncate(title, 100),
            &truncate(&row.body, 1000),
            status,
            Some(format!("/meetings/{}", row.meeting_id)),
            &row.created_at,
            &row.created_at,
            if row.applied != 0 { 0.7 } else { 0.55 },
            json!({ "meeting_id": &row.meeting_id, "kind": &row.kind, "target_id": &row.target_id, "payload": &row.payload }),
        ));
        projection.push_edge(relation(
            &graph_node_id("meeting", &row.meeting_id),
            &graph_node_id("meeting_action", &row.id),
            "CONTAINS",
            "contains action",
            0.85,
            json!({ "source": "meeting_actions.meeting_id" }),
            &row.created_at,
            &row.created_at,
            json!({}),
        ));
        if let Some(target_id) = &row.target_id {
            if let Some(target) = meeting_action_target_node(&row.kind, target_id) {
                projection.push_edge(relation(
                    &graph_node_id("meeting_action", &row.id),
                    &target,
                    "GENERATED",
                    "generated",
                    0.8,
                    json!({ "source": "meeting_actions.target_id" }),
                    &row.created_at,
                    &row.created_at,
                    json!({ "action_kind": &row.kind }),
                ));
            }
        }
    }

    let notes = sqlx::query_as::<_, InitiativeNoteRow>(
        "SELECT id, initiative_id, body, created_at FROM initiative_notes",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in notes {
        projection.push_node(entity(
            "initiative_note",
            "initiative_notes",
            &row.id,
            &truncate(&row.body, 80),
            &truncate(&row.body, 1200),
            "",
            Some(format!("/initiatives/{}", row.initiative_id)),
            &row.created_at,
            &row.created_at,
            0.45,
            json!({ "initiative_id": &row.initiative_id }),
        ));
        projection.push_edge(relation(
            &graph_node_id("initiative", &row.initiative_id),
            &graph_node_id("initiative_note", &row.id),
            "CONTAINS",
            "contains note",
            0.65,
            json!({ "source": "initiative_notes" }),
            &row.created_at,
            &row.created_at,
            json!({}),
        ));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct WeekPlanRow {
    week_start: String,
    day_index: i64,
    deliverable_id: String,
    updated_at: String,
}

async fn add_week_plan(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let rows = sqlx::query_as::<_, WeekPlanRow>(
        "SELECT week_start, day_index, deliverable_id, updated_at FROM week_plans",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in rows {
        let day_id = format!("{}:{}", row.week_start, row.day_index);
        projection.push_node(entity(
            "week_day",
            "week_plans",
            &day_id,
            &format!("{} day {}", row.week_start, row.day_index + 1),
            "Scheduled work day",
            "",
            Some("/week".to_string()),
            &row.updated_at,
            &row.updated_at,
            0.35,
            json!({ "week_start": row.week_start, "day_index": row.day_index }),
        ));
        projection.push_edge(relation(
            &graph_node_id("week_day", &day_id),
            &graph_node_id("deliverable", &row.deliverable_id),
            "SCHEDULED_FOR",
            "scheduled",
            0.7,
            json!({ "source": "week_plans" }),
            &row.updated_at,
            &row.updated_at,
            json!({}),
        ));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct CalendarEventRow {
    id: String,
    gcal_event_id: String,
    title: String,
    description: String,
    location: String,
    attendees: String,
    start_date: String,
    end_date: Option<String>,
    start_datetime: Option<String>,
    end_datetime: Option<String>,
    is_all_day: i64,
    html_link: Option<String>,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct CalendarSyncMapRow {
    entity_type: String,
    entity_id: String,
    gcal_event_id: String,
    last_synced_at: String,
}

async fn add_calendar(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let events = sqlx::query_as::<_, CalendarEventRow>(
        r#"
        SELECT id, gcal_event_id, title, COALESCE(description, '') AS description,
               COALESCE(location, '') AS location, COALESCE(attendees, '[]') AS attendees,
               start_date, end_date, start_datetime, end_datetime, is_all_day, html_link, updated_at
        FROM gcal_events
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in events {
        let time_summary = if row.is_all_day != 0 {
            format!("All-day calendar event on {}.", row.start_date)
        } else {
            format!(
                "Calendar event from {} to {}.",
                row.start_datetime.as_deref().unwrap_or(&row.start_date),
                row.end_datetime.as_deref().unwrap_or("")
            )
        };
        let summary = join_nonempty([
            time_summary.as_str(),
            row.location.as_str(),
            row.description.as_str(),
        ]);
        projection.push_node(entity(
            "calendar_event",
            "gcal_events",
            &row.id,
            &row.title,
            &truncate(&summary, 1200),
            "scheduled",
            row.html_link.clone().or_else(|| Some("/week".to_string())),
            row.start_datetime.as_deref().unwrap_or(&row.start_date),
            &row.updated_at,
            if is_today(&row.start_date) {
                0.75
            } else {
                0.45
            },
            json!({
                "gcal_event_id": &row.gcal_event_id,
                "start_date": &row.start_date,
                "end_date": &row.end_date,
                "start_datetime": &row.start_datetime,
                "end_datetime": &row.end_datetime,
                "is_all_day": row.is_all_day != 0,
                "location": &row.location,
                "attendees": json_from_string(&row.attendees),
            }),
        ));
    }

    let sync_rows = sqlx::query_as::<_, CalendarSyncMapRow>(
        "SELECT entity_type, entity_id, gcal_event_id, last_synced_at FROM gcal_sync_map",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in sync_rows {
        let event_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM gcal_events WHERE gcal_event_id = ?")
                .bind(&row.gcal_event_id)
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?;
        let Some(event_id) = event_id else {
            continue;
        };
        if let Some(source) = source_node_id(&row.entity_type, &row.entity_id) {
            projection.push_edge(relation(
                &source,
                &graph_node_id("calendar_event", &event_id),
                "SCHEDULED_FOR",
                "scheduled on calendar",
                0.9,
                json!({ "source": "gcal_sync_map", "gcal_event_id": row.gcal_event_id }),
                &row.last_synced_at,
                &row.last_synced_at,
                json!({}),
            ));
        }
    }

    Ok(())
}

#[derive(Debug, FromRow)]
struct GmailThreadRow {
    thread_id: String,
    subject: String,
    snippet: String,
    participants: String,
    last_message_at: Option<i64>,
    message_count: i64,
    has_unread: i64,
    last_from_name: String,
    last_from_email: String,
    summary: Option<String>,
    sentiment: Option<String>,
    urgency: Option<String>,
    last_sync_at: String,
    ai_category: Option<String>,
    ai_priority: Option<String>,
}

#[derive(Debug, FromRow)]
struct GmailParticipantRow {
    email: String,
    name: String,
    first_seen_at: String,
    last_seen_at: String,
    sent_count: i64,
    received_count: i64,
    thread_count: i64,
}

#[derive(Debug, FromRow)]
struct GmailThreadParticipantRow {
    thread_id: String,
    email: String,
    name: String,
    role: String,
    message_count: i64,
    first_seen_at: String,
    last_seen_at: String,
}

#[derive(Debug, FromRow)]
struct GmailLabelRow {
    gmail_label_id: String,
    name: String,
    label_type: String,
    color: Option<String>,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct GmailDraftRow {
    draft_id: String,
    message_id: String,
    thread_id: Option<String>,
    subject: String,
    to_json: String,
    body_preview: String,
    updated_at: Option<String>,
    synced_at: String,
}

#[derive(Debug, FromRow)]
struct GmailMessageRow {
    message_id: String,
    thread_id: String,
    subject: String,
    snippet: String,
    from_name: String,
    from_email: String,
    internal_date_ts: Option<i64>,
    label_ids_json: String,
    is_sent: i64,
    is_draft: i64,
    is_unread: i64,
    artifact_urls_json: String,
    synced_at: String,
}

#[derive(Debug, FromRow)]
struct GmailAttachmentRow {
    id: String,
    message_id: String,
    thread_id: String,
    filename: String,
    mime_type: String,
    size: Option<i64>,
    shared_by_email: String,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct GmailSuggestionRow {
    id: String,
    thread_id: String,
    kind: String,
    title: String,
    body: String,
    payload: String,
    status: String,
    created_at: String,
    applied_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct GmailFollowupRow {
    id: String,
    thread_id: String,
    message_id: Option<String>,
    sent_at: String,
    expected_reply_after_days: i64,
    due_at: String,
    status: String,
    resolved_at: Option<String>,
    created_at: String,
    updated_at: String,
}

async fn add_gmail(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let threads = sqlx::query_as::<_, GmailThreadRow>(
        r#"
        SELECT thread_id, subject, snippet, participants, last_message_at, message_count,
               has_unread, last_from_name, last_from_email, summary, sentiment, urgency,
               last_sync_at, COALESCE(ai_category, 'other') AS ai_category,
               COALESCE(ai_priority, 'low') AS ai_priority
        FROM gmail_threads
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in threads {
        let title = if row.subject.trim().is_empty() {
            "Untitled email thread"
        } else {
            row.subject.as_str()
        };
        let summary = join_nonempty([
            row.summary.as_deref().unwrap_or(""),
            row.snippet.as_str(),
            row.participants.as_str(),
        ]);
        projection.push_node(entity(
            "email_thread",
            "gmail_threads",
            &row.thread_id,
            title,
            &truncate(&summary, 1000),
            row.ai_priority.as_deref().unwrap_or("low"),
            Some(format!("/email?thread={}", row.thread_id)),
            "",
            &row.last_sync_at,
            email_importance(row.ai_priority.as_deref(), row.has_unread != 0),
            json!({
                "participants": &row.participants,
                "last_message_at": &row.last_message_at,
                "message_count": row.message_count,
                "has_unread": row.has_unread != 0,
                "last_from_name": &row.last_from_name,
                "last_from_email": &row.last_from_email,
                "sentiment": &row.sentiment,
                "urgency": &row.urgency,
                "ai_category": &row.ai_category,
                "ai_priority": &row.ai_priority,
            }),
        ));
    }

    let participants = sqlx::query_as::<_, GmailParticipantRow>(
        r#"
        SELECT email, name, first_seen_at, last_seen_at, sent_count, received_count, thread_count
        FROM gmail_participants
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in participants {
        let title = if row.name.trim().is_empty() {
            row.email.clone()
        } else {
            row.name.clone()
        };
        projection.push_node(entity(
            "email_participant",
            "gmail_participants",
            &row.email,
            &title,
            &format!(
                "{} email thread(s), {} sent, {} received. Email: {}",
                row.thread_count, row.sent_count, row.received_count, row.email
            ),
            "",
            Some(format!("/email?participant={}", row.email)),
            &row.first_seen_at,
            &row.last_seen_at,
            0.45,
            json!({ "email": &row.email, "sent_count": row.sent_count, "received_count": row.received_count, "thread_count": row.thread_count }),
        ));
    }

    let thread_participants = sqlx::query_as::<_, GmailThreadParticipantRow>(
        r#"
        SELECT thread_id, email, name, role, message_count, first_seen_at, last_seen_at
        FROM gmail_thread_participants
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in thread_participants {
        let participant_id = graph_node_id("email_participant", &row.email);
        if !projection.nodes.contains_key(&participant_id) {
            projection.push_node(entity(
                "email_participant",
                "gmail_thread_participants",
                &row.email,
                if row.name.trim().is_empty() {
                    &row.email
                } else {
                    &row.name
                },
                &format!("Email participant {}", row.email),
                "",
                Some(format!("/email?participant={}", row.email)),
                &row.first_seen_at,
                &row.last_seen_at,
                0.35,
                json!({ "email": &row.email }),
            ));
        }
        projection.push_edge(relation(
            &participant_id,
            &graph_node_id("email_thread", &row.thread_id),
            "PARTICIPATED_IN",
            &row.role,
            0.55 + (row.message_count as f64).min(10.0) / 25.0,
            json!({ "source": "gmail_thread_participants", "message_count": row.message_count }),
            &row.first_seen_at,
            &row.last_seen_at,
            json!({ "role": &row.role }),
        ));
    }

    let labels = sqlx::query_as::<_, GmailLabelRow>(
        r#"
        SELECT gmail_label_id, name, type AS label_type, color, updated_at
        FROM gmail_labels
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in labels {
        projection.push_node(entity(
            "email_label",
            "gmail_labels",
            &row.gmail_label_id,
            &row.name,
            &format!("Gmail {} label", row.label_type),
            &row.label_type,
            Some("/email".to_string()),
            &row.updated_at,
            &row.updated_at,
            0.25,
            json!({ "color": &row.color }),
        ));
    }
    let thread_labels = sqlx::query_as::<_, (String, String)>(
        "SELECT thread_id, gmail_label_id FROM gmail_thread_labels",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (thread_id, label_id) in thread_labels {
        projection.push_edge(relation(
            &graph_node_id("email_thread", &thread_id),
            &graph_node_id("email_label", &label_id),
            "TAGGED_WITH",
            "gmail label",
            0.35,
            json!({ "source": "gmail_thread_labels" }),
            "",
            "",
            json!({}),
        ));
    }

    let messages = sqlx::query_as::<_, GmailMessageRow>(
        r#"
        SELECT message_id, thread_id, subject, snippet, from_name, from_email, internal_date_ts,
               label_ids_json, is_sent, is_draft, is_unread, artifact_urls_json, synced_at
        FROM gmail_messages
        ORDER BY COALESCE(internal_date_ts, 0) DESC
        LIMIT 500
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in messages {
        let title = if row.subject.trim().is_empty() {
            truncate(&row.snippet, 80)
        } else {
            truncate(&row.subject, 120)
        };
        projection.push_node(entity(
            "email_message",
            "gmail_messages",
            &row.message_id,
            &title,
            &truncate(&row.snippet, 600),
            if row.is_unread != 0 { "unread" } else { "" },
            Some(format!("/email?thread={}", row.thread_id)),
            "",
            &row.synced_at,
            if row.is_unread != 0 { 0.45 } else { 0.25 },
            json!({
                "thread_id": &row.thread_id,
                "from_name": &row.from_name,
                "from_email": &row.from_email,
                "internal_date_ts": &row.internal_date_ts,
                "label_ids_json": &row.label_ids_json,
                "is_sent": row.is_sent != 0,
                "is_draft": row.is_draft != 0,
                "is_unread": row.is_unread != 0,
                "artifact_urls_json": &row.artifact_urls_json,
            }),
        ));
        projection.push_edge(relation(
            &graph_node_id("email_thread", &row.thread_id),
            &graph_node_id("email_message", &row.message_id),
            "CONTAINS",
            "contains message",
            0.45,
            json!({ "source": "gmail_messages" }),
            "",
            &row.synced_at,
            json!({}),
        ));
    }

    let drafts = sqlx::query_as::<_, GmailDraftRow>(
        r#"
        SELECT draft_id, message_id, thread_id, subject, to_json, body_preview, updated_at, synced_at
        FROM gmail_drafts
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in drafts {
        projection.push_node(entity(
            "email_draft",
            "gmail_drafts",
            &row.draft_id,
            if row.subject.trim().is_empty() { "Email draft" } else { &row.subject },
            &truncate(&row.body_preview, 800),
            "draft",
            Some("/email".to_string()),
            row.updated_at.as_deref().unwrap_or(&row.synced_at),
            &row.synced_at,
            0.55,
            json!({ "message_id": &row.message_id, "thread_id": &row.thread_id, "to_json": &row.to_json }),
        ));
        if let Some(thread_id) = &row.thread_id {
            projection.push_edge(relation(
                &graph_node_id("email_thread", thread_id),
                &graph_node_id("email_draft", &row.draft_id),
                "CONTAINS",
                "contains draft",
                0.65,
                json!({ "source": "gmail_drafts.thread_id" }),
                row.updated_at.as_deref().unwrap_or(""),
                &row.synced_at,
                json!({}),
            ));
        }
    }

    let attachments = sqlx::query_as::<_, GmailAttachmentRow>(
        r#"
        SELECT id, message_id, thread_id, filename, mime_type, size, shared_by_email, created_at
        FROM gmail_attachments
        ORDER BY created_at DESC
        LIMIT 500
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in attachments {
        projection.push_node(entity(
            "email_attachment",
            "gmail_attachments",
            &row.id,
            if row.filename.trim().is_empty() { "Email attachment" } else { &row.filename },
            &format!(
                "{} attachment{} shared by {}.",
                row.mime_type,
                row.size
                    .map(|size| format!(" ({size} bytes)"))
                    .unwrap_or_default(),
                row.shared_by_email
            ),
            "",
            Some(format!("/email?thread={}", row.thread_id)),
            &row.created_at,
            &row.created_at,
            0.25,
            json!({ "message_id": &row.message_id, "thread_id": &row.thread_id, "mime_type": &row.mime_type, "size": &row.size }),
        ));
        projection.push_edge(relation(
            &graph_node_id("email_thread", &row.thread_id),
            &graph_node_id("email_attachment", &row.id),
            "CONTAINS",
            "contains attachment",
            0.35,
            json!({ "source": "gmail_attachments" }),
            &row.created_at,
            &row.created_at,
            json!({}),
        ));
    }

    let suggestions = sqlx::query_as::<_, GmailSuggestionRow>(
        r#"
        SELECT id, thread_id, kind, title, body, payload, status, created_at, applied_at
        FROM gmail_ai_suggestions
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in suggestions {
        projection.push_node(entity(
            "email_suggestion",
            "gmail_ai_suggestions",
            &row.id,
            if row.title.trim().is_empty() {
                &row.kind
            } else {
                &row.title
            },
            &truncate(&row.body, 900),
            &row.status,
            Some(format!("/email?thread={}", row.thread_id)),
            &row.created_at,
            row.applied_at.as_deref().unwrap_or(&row.created_at),
            0.45,
            json!({ "thread_id": &row.thread_id, "kind": &row.kind, "payload": &row.payload }),
        ));
        projection.push_edge(relation(
            &graph_node_id("email_thread", &row.thread_id),
            &graph_node_id("email_suggestion", &row.id),
            "SUGGESTS",
            "suggests",
            0.6,
            json!({ "source": "gmail_ai_suggestions" }),
            &row.created_at,
            row.applied_at.as_deref().unwrap_or(&row.created_at),
            json!({}),
        ));
    }

    let followups = sqlx::query_as::<_, GmailFollowupRow>(
        r#"
        SELECT id, thread_id, message_id, sent_at, expected_reply_after_days,
               due_at, status, resolved_at, created_at, updated_at
        FROM gmail_followups
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in followups {
        let overdue = row.status == "open" && date_is_before_today(&row.due_at);
        let title = if overdue {
            format!("Overdue email follow-up: {}", row.thread_id)
        } else {
            format!("Email follow-up: {}", row.thread_id)
        };
        projection.push_node(entity(
            "email_followup",
            "gmail_followups",
            &row.id,
            &title,
            &format!(
                "Follow-up due {} after {} day(s) without reply.",
                row.due_at, row.expected_reply_after_days
            ),
            if overdue { "overdue" } else { &row.status },
            Some(format!("/email?thread={}", row.thread_id)),
            &row.created_at,
            &row.updated_at,
            if overdue { 0.9 } else { 0.7 },
            json!({
                "thread_id": &row.thread_id,
                "message_id": &row.message_id,
                "sent_at": &row.sent_at,
                "expected_reply_after_days": row.expected_reply_after_days,
                "due_at": &row.due_at,
                "resolved_at": &row.resolved_at,
            }),
        ));
        projection.push_edge(relation(
            &graph_node_id("email_thread", &row.thread_id),
            &graph_node_id("email_followup", &row.id),
            "HAS_FOLLOWUP",
            "has follow-up",
            0.95,
            json!({ "source": "gmail_followups.thread_id" }),
            &row.created_at,
            &row.updated_at,
            json!({}),
        ));
        if let Some(message_id) = &row.message_id {
            projection.push_edge(relation(
                &graph_node_id("email_message", message_id),
                &graph_node_id("email_followup", &row.id),
                "HAS_FOLLOWUP",
                "has follow-up",
                0.75,
                json!({ "source": "gmail_followups.message_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
    }

    for (thread_id, deliverable_id, linked_at) in sqlx::query_as::<_, (String, String, String)>(
        "SELECT thread_id, deliverable_id, linked_at FROM gmail_thread_deliverables",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?
    {
        projection.push_edge(relation(
            &graph_node_id("email_thread", &thread_id),
            &graph_node_id("deliverable", &deliverable_id),
            "RELATED_TO",
            "relates to deliverable",
            0.85,
            json!({ "source": "gmail_thread_deliverables" }),
            &linked_at,
            &linked_at,
            json!({}),
        ));
    }
    for (thread_id, initiative_id, linked_at) in sqlx::query_as::<_, (String, String, String)>(
        "SELECT thread_id, initiative_id, linked_at FROM gmail_thread_initiatives",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?
    {
        projection.push_edge(relation(
            &graph_node_id("email_thread", &thread_id),
            &graph_node_id("initiative", &initiative_id),
            "RELATED_TO",
            "relates to initiative",
            0.8,
            json!({ "source": "gmail_thread_initiatives" }),
            &linked_at,
            &linked_at,
            json!({}),
        ));
    }
    for (thread_id, capture_id, linked_at) in sqlx::query_as::<_, (String, String, String)>(
        "SELECT thread_id, capture_id, linked_at FROM gmail_thread_captures",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?
    {
        projection.push_edge(relation(
            &graph_node_id("email_thread", &thread_id),
            &graph_node_id("capture", &capture_id),
            "RELATED_TO",
            "relates to capture",
            0.75,
            json!({ "source": "gmail_thread_captures" }),
            &linked_at,
            &linked_at,
            json!({}),
        ));
    }

    Ok(())
}

#[derive(Debug, FromRow)]
struct AskChatRow {
    id: String,
    title: String,
    mode: String,
    summary: String,
    archived_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct AskTurnRow {
    id: String,
    chat_id: String,
    parent_id: Option<String>,
    fork_of: Option<String>,
    mode: Option<String>,
    question: String,
    answer: String,
    reasoning: String,
    status: String,
    error: Option<String>,
    refs_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct AskAttachmentRow {
    id: String,
    turn_id: String,
    mime_type: String,
    filename: Option<String>,
    size_bytes: i64,
    created_at: String,
}

async fn add_ask(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let chats = sqlx::query_as::<_, AskChatRow>(
        "SELECT id, title, mode, summary, archived_at, created_at, updated_at FROM ask_chats",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in chats {
        projection.push_node(entity(
            "ask_chat",
            "ask_chats",
            &row.id,
            &row.title,
            &truncate(&row.summary, 1200),
            if row.archived_at.is_some() {
                "archived"
            } else {
                &row.mode
            },
            Some(format!("/ask?chat={}", row.id)),
            &row.created_at,
            &row.updated_at,
            0.5,
            json!({ "mode": row.mode, "archived_at": row.archived_at }),
        ));
    }

    let turns = sqlx::query_as::<_, AskTurnRow>(
        r#"
        SELECT id, chat_id, parent_id, fork_of, mode, question, answer, reasoning,
               status, error, refs_json, created_at, updated_at
        FROM ask_turns
        ORDER BY created_at DESC
        LIMIT 500
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in turns {
        let title = truncate(&row.question, 100);
        let question_summary = format!("Q: {}", truncate(&row.question, 600));
        let answer_summary = format!("A: {}", truncate(&row.answer, 900));
        let summary = join_nonempty([
            question_summary.as_str(),
            answer_summary.as_str(),
            if row.error.is_some() {
                "Turn has an error."
            } else {
                ""
            },
        ]);
        projection.push_node(entity(
            "ask_turn",
            "ask_turns",
            &row.id,
            &title,
            &summary,
            &row.status,
            Some(format!("/ask?chat={}", row.chat_id)),
            &row.created_at,
            &row.updated_at,
            0.35,
            json!({
                "chat_id": &row.chat_id,
                "parent_id": &row.parent_id,
                "fork_of": &row.fork_of,
                "mode": &row.mode,
                "reasoning_summary": truncate(&row.reasoning, 600),
                "refs_json": &row.refs_json,
            }),
        ));
        projection.push_edge(relation(
            &graph_node_id("ask_chat", &row.chat_id),
            &graph_node_id("ask_turn", &row.id),
            "CONTAINS",
            "contains turn",
            0.55,
            json!({ "source": "ask_turns.chat_id" }),
            &row.created_at,
            &row.updated_at,
            json!({}),
        ));
        add_ask_ref_edges(&row, projection);
    }

    let attachments = sqlx::query_as::<_, AskAttachmentRow>(
        "SELECT id, turn_id, mime_type, filename, size_bytes, created_at FROM ask_turn_attachments",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in attachments {
        let title = row.filename.as_deref().unwrap_or("Ask attachment");
        projection.push_node(entity(
            "ask_attachment",
            "ask_turn_attachments",
            &row.id,
            title,
            &format!("{} attachment, {} bytes.", row.mime_type, row.size_bytes),
            "",
            Some("/ask".to_string()),
            &row.created_at,
            &row.created_at,
            0.25,
            json!({ "turn_id": &row.turn_id, "mime_type": &row.mime_type, "size_bytes": row.size_bytes }),
        ));
        projection.push_edge(relation(
            &graph_node_id("ask_turn", &row.turn_id),
            &graph_node_id("ask_attachment", &row.id),
            "CONTAINS",
            "contains attachment",
            0.4,
            json!({ "source": "ask_turn_attachments" }),
            &row.created_at,
            &row.created_at,
            json!({}),
        ));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct WorkIntakeRow {
    id: String,
    source_kind: String,
    source_id: Option<String>,
    source_title: String,
    source_route: Option<String>,
    item_kind: String,
    title: String,
    body: String,
    target_deliverable_id: Option<String>,
    target_initiative_id: Option<String>,
    due_date: Option<String>,
    suggested_type: Option<String>,
    confidence: Option<f64>,
    rationale: String,
    status: String,
    payload: String,
    created_at: String,
    updated_at: String,
    applied_at: Option<String>,
}

async fn add_work_intake(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let rows = sqlx::query_as::<_, WorkIntakeRow>(
        r#"
        SELECT id, source_kind, source_id, source_title, source_route, item_kind, title, body,
               target_deliverable_id, target_initiative_id, due_date, suggested_type,
               confidence, rationale, status, payload, created_at, updated_at, applied_at
        FROM work_intake_suggestions
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in rows {
        let summary = join_nonempty([
            row.body.as_str(),
            row.rationale.as_str(),
            row.due_date.as_deref().unwrap_or(""),
        ]);
        projection.push_node(entity(
            "work_intake_suggestion",
            "work_intake_suggestions",
            &row.id,
            &row.title,
            &truncate(&summary, 1000),
            &row.status,
            row.source_route
                .clone()
                .or_else(|| Some("/email".to_string())),
            &row.created_at,
            &row.updated_at,
            row.confidence.unwrap_or(0.5).clamp(0.1, 1.0),
            json!({
                "source_kind": &row.source_kind,
                "source_id": &row.source_id,
                "source_title": &row.source_title,
                "item_kind": &row.item_kind,
                "target_deliverable_id": &row.target_deliverable_id,
                "target_initiative_id": &row.target_initiative_id,
                "due_date": &row.due_date,
                "suggested_type": &row.suggested_type,
                "payload": &row.payload,
                "applied_at": &row.applied_at,
            }),
        ));
        if let Some(source_id) = &row.source_id {
            if let Some(source_node) = source_node_id(&row.source_kind, source_id) {
                projection.push_edge(relation(
                    &source_node,
                    &graph_node_id("work_intake_suggestion", &row.id),
                    "SUGGESTS",
                    "suggests",
                    0.75,
                    json!({ "source": "work_intake_suggestions.source" }),
                    &row.created_at,
                    &row.updated_at,
                    json!({}),
                ));
            }
        }
        if let Some(deliverable_id) = &row.target_deliverable_id {
            projection.push_edge(relation(
                &graph_node_id("work_intake_suggestion", &row.id),
                &graph_node_id("deliverable", deliverable_id),
                "TARGETS",
                "targets deliverable",
                0.75,
                json!({ "source": "work_intake_suggestions.target_deliverable_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
        if let Some(initiative_id) = &row.target_initiative_id {
            projection.push_edge(relation(
                &graph_node_id("work_intake_suggestion", &row.id),
                &graph_node_id("initiative", initiative_id),
                "TARGETS",
                "targets initiative",
                0.7,
                json!({ "source": "work_intake_suggestions.target_initiative_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct TraceFolderRow {
    id: String,
    parent_id: Option<String>,
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct FileProjectionRow {
    id: String,
    trace_folder_id: Option<String>,
    name: String,
    kind: String,
    created_at: String,
    updated_at: String,
    description: Option<String>,
}

async fn add_files(pool: &SqlitePool, projection: &mut BrainProjection) -> Result<(), String> {
    let folders = sqlx::query_as::<_, TraceFolderRow>(
        "SELECT id, parent_id, name, created_at, updated_at FROM trace_folders",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in folders {
        projection.push_node(entity(
            "trace_folder",
            "trace_folders",
            &row.id,
            &row.name,
            "Trace file folder",
            "",
            Some(format!("/files?folder={}", row.id)),
            &row.created_at,
            &row.updated_at,
            0.45,
            json!({ "parent_id": &row.parent_id }),
        ));
        if let Some(parent_id) = &row.parent_id {
            projection.push_edge(relation(
                &graph_node_id("trace_folder", parent_id),
                &graph_node_id("trace_folder", &row.id),
                "CONTAINS",
                "contains folder",
                0.75,
                json!({ "source": "trace_folders.parent_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
    }

    let rows = sqlx::query_as::<_, FileProjectionRow>(
        r#"
        SELECT id, trace_folder_id, name, kind, created_at, updated_at, description
        FROM files
        WHERE drive_trashed = 0
        ORDER BY updated_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for row in rows {
        let summary = row.description.unwrap_or_default();
        projection.push_node(entity(
            "file",
            "files",
            &row.id,
            &row.name,
            &summary,
            &row.kind,
            Some(format!("/files?file={}", row.id)),
            &row.created_at,
            &row.updated_at,
            0.5,
            json!({ "kind": row.kind, "trace_folder_id": &row.trace_folder_id }),
        ));
        if let Some(folder_id) = &row.trace_folder_id {
            projection.push_edge(relation(
                &graph_node_id("trace_folder", folder_id),
                &graph_node_id("file", &row.id),
                "CONTAINS",
                "contains file",
                0.75,
                json!({ "source": "files.trace_folder_id" }),
                &row.created_at,
                &row.updated_at,
                json!({}),
            ));
        }
    }

    let link_rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT file_id, entity_kind, entity_id, linked_at FROM file_links",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for (file_id, entity_kind, entity_id, linked_at) in link_rows {
        if let Some(target) = source_node_id(&entity_kind, &entity_id) {
            projection.push_edge(relation(
                &graph_node_id("file", &file_id),
                &target,
                "ATTACHED_TO",
                "attached to",
                0.7,
                json!({ "source": "file_links" }),
                &linked_at,
                &linked_at,
                json!({}),
            ));
        }
    }

    let folder_link_rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT folder_id, entity_kind, entity_id, linked_at FROM folder_entity_links",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for (folder_id, entity_kind, entity_id, linked_at) in folder_link_rows {
        if let Some(target) = source_node_id(&entity_kind, &entity_id) {
            projection.push_edge(relation(
                &graph_node_id("trace_folder", &folder_id),
                &target,
                "ATTACHED_TO",
                "folder attached to",
                0.75,
                json!({ "source": "folder_entity_links" }),
                &linked_at,
                &linked_at,
                json!({}),
            ));
        }
    }

    Ok(())
}

async fn add_identity_links(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT id, COALESCE(email, '') AS email FROM stakeholders WHERE TRIM(COALESCE(email, '')) != ''",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let participant_ids_by_email = projection
        .nodes
        .values()
        .filter(|node| node.kind == "email_participant")
        .map(|node| (normalize_email(&node.source_id), node.id.clone()))
        .collect::<BTreeMap<_, _>>();

    for (stakeholder_id, email) in rows {
        if let Some(participant_id) = participant_ids_by_email.get(&normalize_email(&email)) {
            projection.push_edge(relation(
                &graph_node_id("stakeholder", &stakeholder_id),
                participant_id,
                "IDENTIFIES",
                "same email identity",
                1.0,
                json!({ "source": "stakeholders.email + gmail_participants.email", "email": email }),
                "",
                "",
                json!({}),
            ));
        }
    }

    Ok(())
}

async fn add_cross_links(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let blocked = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT id, title, blocker_reason, updated_at
        FROM deliverables
        WHERE blocker_reason IS NOT NULL AND TRIM(blocker_reason) != ''
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (deliverable_id, title, blocker, updated_at) in blocked {
        let blocker_id = format!("{}:{}", deliverable_id, stable_suffix(&blocker));
        projection.push_node(entity(
            "blocker",
            "deliverables",
            &blocker_id,
            &format!("Blocker for {title}"),
            &truncate(&blocker, 800),
            "open",
            Some(format!("/deliverables/{deliverable_id}")),
            &updated_at,
            &updated_at,
            0.75,
            json!({ "deliverable_id": deliverable_id }),
        ));
        projection.push_edge(relation(
            &graph_node_id("deliverable", &deliverable_id),
            &graph_node_id("blocker", &blocker_id),
            "BLOCKED_BY",
            "blocked by",
            0.95,
            json!({ "source": "deliverables.blocker_reason" }),
            &updated_at,
            &updated_at,
            json!({}),
        ));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct AskOpenLoopRow {
    id: String,
    chat_id: String,
    question: String,
    status: String,
    questions_json: String,
    created_at: String,
    updated_at: String,
}

async fn add_open_loops_and_attention(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let tasks = sqlx::query_as::<_, TaskRow>(
        r#"
        SELECT id, deliverable_id, title, status, due_date, display_order, created_at, updated_at
        FROM deliverable_tasks
        WHERE status != 'done'
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in tasks {
        let loop_source_id = format!("task:{}", row.id);
        let is_overdue = row
            .due_date
            .as_deref()
            .map(date_is_before_today)
            .unwrap_or(false);
        projection.push_node(entity(
            "open_loop",
            "deliverable_tasks",
            &loop_source_id,
            &format!("Task pending: {}", row.title),
            &format!(
                "Pending task on deliverable {}{}.",
                row.deliverable_id,
                row.due_date
                    .as_ref()
                    .map(|date| format!(" due {date}"))
                    .unwrap_or_default()
            ),
            if is_overdue { "overdue" } else { "open" },
            Some(format!("/deliverables/{}", row.deliverable_id)),
            &row.created_at,
            &row.updated_at,
            if is_overdue { 0.9 } else { 0.7 },
            json!({
                "open_loop_type": "task",
                "task_id": &row.id,
                "deliverable_id": &row.deliverable_id,
                "due_date": &row.due_date,
            }),
        ));
        projection.push_edge(relation(
            &graph_node_id("task", &row.id),
            &graph_node_id("open_loop", &loop_source_id),
            "HAS_OPEN_LOOP",
            "has open loop",
            0.95,
            json!({ "source": "deliverable_tasks.status" }),
            &row.created_at,
            &row.updated_at,
            json!({}),
        ));
        if let Some(due_date) = &row.due_date {
            if is_overdue || date_is_within_days(due_date, 2) {
                add_attention_signal(
                    projection,
                    "task_due",
                    &row.id,
                    &row.title,
                    if is_overdue { "overdue" } else { "due soon" },
                    &graph_node_id("task", &row.id),
                    &row.updated_at,
                    if is_overdue { 0.95 } else { 0.75 },
                    json!({ "due_date": due_date }),
                );
            }
        }
    }

    let blocked = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT id, title, blocker_reason, updated_at
        FROM deliverables
        WHERE blocker_reason IS NOT NULL AND TRIM(blocker_reason) != ''
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (deliverable_id, title, blocker, updated_at) in blocked {
        let loop_source_id = format!("blocker:{deliverable_id}");
        projection.push_node(entity(
            "open_loop",
            "deliverables",
            &loop_source_id,
            &format!("Blocked: {title}"),
            &truncate(&blocker, 900),
            "blocked",
            Some(format!("/deliverables/{deliverable_id}")),
            &updated_at,
            &updated_at,
            0.95,
            json!({ "open_loop_type": "blocker", "deliverable_id": &deliverable_id }),
        ));
        projection.push_edge(relation(
            &graph_node_id("deliverable", &deliverable_id),
            &graph_node_id("open_loop", &loop_source_id),
            "HAS_OPEN_LOOP",
            "has open loop",
            1.0,
            json!({ "source": "deliverables.blocker_reason" }),
            &updated_at,
            &updated_at,
            json!({}),
        ));
        add_attention_signal(
            projection,
            "blocked",
            &deliverable_id,
            &title,
            &blocker,
            &graph_node_id("deliverable", &deliverable_id),
            &updated_at,
            1.0,
            json!({ "blocker_reason": blocker }),
        );
    }

    let actions = sqlx::query_as::<_, MeetingActionRow>(
        r#"
        SELECT id, meeting_id, kind, target_id, target_title, body, applied, created_at, payload
        FROM meeting_actions
        WHERE applied = 0
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in actions {
        let loop_source_id = format!("meeting_action:{}", row.id);
        projection.push_node(entity(
            "open_loop",
            "meeting_actions",
            &loop_source_id,
            &format!("Meeting action: {}", truncate(&row.body, 80)),
            &truncate(&row.body, 900),
            "open",
            Some(format!("/meetings/{}", row.meeting_id)),
            &row.created_at,
            &row.created_at,
            0.7,
            json!({
                "open_loop_type": "meeting_action",
                "meeting_action_id": &row.id,
                "meeting_id": &row.meeting_id,
                "target_id": &row.target_id,
            }),
        ));
        projection.push_edge(relation(
            &graph_node_id("meeting_action", &row.id),
            &graph_node_id("open_loop", &loop_source_id),
            "HAS_OPEN_LOOP",
            "has open loop",
            0.9,
            json!({ "source": "meeting_actions.applied" }),
            &row.created_at,
            &row.created_at,
            json!({}),
        ));
    }

    let followups = sqlx::query_as::<_, GmailFollowupRow>(
        r#"
        SELECT id, thread_id, message_id, sent_at, expected_reply_after_days,
               due_at, status, resolved_at, created_at, updated_at
        FROM gmail_followups
        WHERE status = 'open'
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in followups {
        let loop_source_id = format!("email_followup:{}", row.id);
        let overdue = date_is_before_today(&row.due_at);
        projection.push_node(entity(
            "open_loop",
            "gmail_followups",
            &loop_source_id,
            &format!("Email follow-up due {}", row.due_at),
            &format!("Waiting for a reply on email thread {}.", row.thread_id),
            if overdue { "overdue" } else { "open" },
            Some(format!("/email?thread={}", row.thread_id)),
            &row.created_at,
            &row.updated_at,
            if overdue { 0.95 } else { 0.75 },
            json!({ "open_loop_type": "email_followup", "followup_id": &row.id, "due_at": &row.due_at }),
        ));
        projection.push_edge(relation(
            &graph_node_id("email_followup", &row.id),
            &graph_node_id("open_loop", &loop_source_id),
            "HAS_OPEN_LOOP",
            "has open loop",
            0.95,
            json!({ "source": "gmail_followups.status" }),
            &row.created_at,
            &row.updated_at,
            json!({}),
        ));
        if overdue || date_is_within_days(&row.due_at, 1) {
            add_attention_signal(
                projection,
                "email_followup_due",
                &row.id,
                &format!("Follow up on {}", row.thread_id),
                &row.due_at,
                &graph_node_id("email_followup", &row.id),
                &row.updated_at,
                if overdue { 0.95 } else { 0.8 },
                json!({ "due_at": &row.due_at, "thread_id": &row.thread_id }),
            );
        }
    }

    let ask_turns = sqlx::query_as::<_, AskOpenLoopRow>(
        r#"
        SELECT id, chat_id, question, status, questions_json, created_at, updated_at
        FROM ask_turns
        WHERE status NOT IN ('done', 'error')
           OR (TRIM(COALESCE(questions_json, '[]')) NOT IN ('[]', ''))
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for row in ask_turns {
        let loop_source_id = format!("ask_turn:{}", row.id);
        projection.push_node(entity(
            "open_loop",
            "ask_turns",
            &loop_source_id,
            &format!("Ask follow-up: {}", truncate(&row.question, 80)),
            &format!("Ask turn status {} with pending questions.", row.status),
            "open",
            Some(format!("/ask?chat={}", row.chat_id)),
            &row.created_at,
            &row.updated_at,
            0.65,
            json!({
                "open_loop_type": "ask_turn",
                "turn_id": &row.id,
                "questions": json_from_string(&row.questions_json),
            }),
        ));
        projection.push_edge(relation(
            &graph_node_id("ask_turn", &row.id),
            &graph_node_id("open_loop", &loop_source_id),
            "HAS_OPEN_LOOP",
            "has open loop",
            0.85,
            json!({ "source": "ask_turns.status/questions_json" }),
            &row.created_at,
            &row.updated_at,
            json!({}),
        ));
    }

    let focused = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, title, updated_at FROM deliverables WHERE is_focused = 1",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (id, title, updated_at) in focused {
        add_attention_signal(
            projection,
            "current_focus",
            &id,
            &title,
            "Currently marked as focused.",
            &graph_node_id("deliverable", &id),
            &updated_at,
            0.9,
            json!({}),
        );
    }

    let unread = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT thread_id, subject, COALESCE(ai_priority, 'low') AS ai_priority, last_sync_at
        FROM gmail_threads
        WHERE has_unread = 1 AND COALESCE(ai_priority, 'low') IN ('high', 'urgent')
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (thread_id, subject, priority, updated_at) in unread {
        add_attention_signal(
            projection,
            "unread_email",
            &thread_id,
            &subject,
            &format!("Unread {priority} priority email."),
            &graph_node_id("email_thread", &thread_id),
            &updated_at,
            if priority == "urgent" { 1.0 } else { 0.85 },
            json!({ "priority": priority }),
        );
    }

    let stale_cutoff = (Utc::now().date_naive() - Duration::days(14)).to_string();
    let stale = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT id, title, state, updated_at
        FROM deliverables
        WHERE state NOT IN ('shipped', 'killed')
          AND updated_at < ?
          AND (priority IN ('high', 'urgent') OR is_focused = 1)
        "#,
    )
    .bind(stale_cutoff)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (id, title, state, updated_at) in stale {
        add_attention_signal(
            projection,
            "stale_work",
            &id,
            &title,
            &format!("Important {state} deliverable has no recent activity."),
            &graph_node_id("deliverable", &id),
            &updated_at,
            0.8,
            json!({ "state": state }),
        );
    }

    Ok(())
}

fn write_projection(path: &Path, projection: BrainProjection) -> Result<BrainStatus, String> {
    remove_brain_path(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create brain directory: {error}"))?;
    }

    let db = Database::new(path, SystemConfig::default())
        .map_err(|error| format!("failed to open Kuzu brain: {error}"))?;
    let conn = Connection::new(&db)
        .map_err(|error| format!("failed to connect to Kuzu brain: {error}"))?;
    create_schema(&conn)?;

    let mut node_stmt = conn
        .prepare(
            r#"
            CREATE (:Entity {
              id: $id,
              kind: $kind,
              source_table: $source_table,
              source_id: $source_id,
              title: $title,
              summary: $summary,
              status: $status,
              route: $route,
              created_at: $created_at,
              updated_at: $updated_at,
              importance: $importance,
              payload_json: $payload_json
            });
            "#,
        )
        .map_err(|error| format!("failed to prepare Entity insert: {error}"))?;

    for entity in projection.nodes.values() {
        conn.execute(
            &mut node_stmt,
            vec![
                ("id", Value::String(entity.id.clone())),
                ("kind", Value::String(entity.kind.clone())),
                ("source_table", Value::String(entity.source_table.clone())),
                ("source_id", Value::String(entity.source_id.clone())),
                ("title", Value::String(entity.title.clone())),
                ("summary", Value::String(entity.summary.clone())),
                ("status", Value::String(entity.status.clone())),
                ("route", Value::String(entity.route.clone())),
                ("created_at", Value::String(entity.created_at.clone())),
                ("updated_at", Value::String(entity.updated_at.clone())),
                ("importance", Value::Double(entity.importance)),
                ("payload_json", Value::String(entity.payload_json.clone())),
            ],
        )
        .map_err(|error| format!("failed to insert brain node {}: {error}", entity.id))?;
    }

    let mut edge_stmt = conn
        .prepare(
            r#"
            MATCH (source:Entity {id: $source}), (target:Entity {id: $target})
            CREATE (source)-[:Related {
              kind: $kind,
              label: $label,
              strength: $strength,
              evidence_json: $evidence_json,
              created_at: $created_at,
              updated_at: $updated_at,
              payload_json: $payload_json
            }]->(target);
            "#,
        )
        .map_err(|error| format!("failed to prepare Related insert: {error}"))?;

    for relation in projection.edges.values() {
        conn.execute(
            &mut edge_stmt,
            vec![
                ("source", Value::String(relation.source.clone())),
                ("target", Value::String(relation.target.clone())),
                ("kind", Value::String(relation.kind.clone())),
                ("label", Value::String(relation.label.clone())),
                ("strength", Value::Double(relation.strength)),
                (
                    "evidence_json",
                    Value::String(relation.evidence_json.clone()),
                ),
                ("created_at", Value::String(relation.created_at.clone())),
                ("updated_at", Value::String(relation.updated_at.clone())),
                ("payload_json", Value::String(relation.payload_json.clone())),
            ],
        )
        .map_err(|error| {
            format!(
                "failed to insert brain edge {} -> {}: {error}",
                relation.source, relation.target
            )
        })?;
    }

    let generated_at = now_utc();
    let meta = BrainMeta {
        schema_version: BRAIN_SCHEMA_VERSION,
        generated_at: generated_at.clone(),
        node_count: projection.nodes.len(),
        edge_count: projection.edges.len(),
    };
    write_meta(path, &meta)?;

    Ok(BrainStatus {
        path: path.display().to_string(),
        exists: true,
        schema_version: BRAIN_SCHEMA_VERSION,
        storage_version: kuzu::get_storage_version(),
        generated_at: Some(generated_at),
        node_count: meta.node_count,
        edge_count: meta.edge_count,
        error: None,
    })
}

fn create_schema(conn: &Connection<'_>) -> Result<(), String> {
    conn.query(
        r#"
        CREATE NODE TABLE Entity(
          id STRING,
          kind STRING,
          source_table STRING,
          source_id STRING,
          title STRING,
          summary STRING,
          status STRING,
          route STRING,
          created_at STRING,
          updated_at STRING,
          importance DOUBLE,
          payload_json STRING,
          PRIMARY KEY(id)
        );
        "#,
    )
    .map_err(|error| format!("failed to create Entity table: {error}"))?;

    conn.query(
        r#"
        CREATE REL TABLE Related(
          FROM Entity TO Entity,
          kind STRING,
          label STRING,
          strength DOUBLE,
          evidence_json STRING,
          created_at STRING,
          updated_at STRING,
          payload_json STRING
        );
        "#,
    )
    .map_err(|error| format!("failed to create Related table: {error}"))?;
    Ok(())
}

async fn read_graph(path: &Path, filters: BrainGraphFilters) -> Result<WorkGraph, String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || read_graph_blocking(&path, filters))
        .await
        .map_err(|error| format!("brain graph read task failed: {error}"))?
}

fn read_graph_blocking(path: &Path, filters: BrainGraphFilters) -> Result<WorkGraph, String> {
    let db = Database::new(path, SystemConfig::default().read_only(true))
        .map_err(|error| format!("failed to open Kuzu brain: {error}"))?;
    let conn = Connection::new(&db)
        .map_err(|error| format!("failed to connect to Kuzu brain: {error}"))?;

    let mut result = conn
        .query(
            r#"
            MATCH (e:Entity)
            RETURN e.id, e.kind, e.source_table, e.source_id, e.title, e.summary, e.status, e.route,
                   e.created_at, e.updated_at, e.importance, e.payload_json
            ORDER BY e.kind, e.title;
            "#,
        )
        .map_err(|error| format!("failed to read brain nodes: {error}"))?;
    let mut nodes = Vec::new();
    for row in &mut result {
        let id = value_string(row.get(0));
        let kind = value_string(row.get(1));
        let source_table = value_string(row.get(2));
        let source_id = value_string(row.get(3));
        let title = value_string(row.get(4));
        let summary = value_string(row.get(5));
        let status = value_string(row.get(6));
        let route = value_string(row.get(7));
        let created_at = value_string(row.get(8));
        let updated_at = value_string(row.get(9));
        let importance = value_f64(row.get(10)).unwrap_or(0.3);
        let payload_json = value_string(row.get(11));
        nodes.push(WorkGraphNode {
            id: id.clone(),
            entity_id: source_id.clone(),
            kind: kind.clone(),
            label: title.clone(),
            subtitle: clean_optional(summary.clone()),
            status: clean_optional(status.clone()),
            url: clean_optional(route.clone()),
            updated_at: clean_optional(updated_at.clone()),
            hidden_by_default: hidden_by_default(&kind, &status),
            weight: ((importance * 10.0).round() as i64).clamp(1, 10),
            context: format!(
                "{} '{}'{}{}",
                kind,
                title,
                optional_sentence("status", &status),
                optional_sentence("summary", &summary)
            ),
            properties: json!({
                "id": id,
                "kind": kind,
                "source_table": source_table,
                "source_id": source_id,
                "title": title,
                "summary": clean_optional(summary),
                "status": clean_optional(status),
                "route": clean_optional(route),
                "created_at": clean_optional(created_at),
                "updated_at": clean_optional(updated_at),
                "importance": importance,
                "payload": json_from_string(&payload_json),
            }),
        });
    }

    let mut result = conn
        .query(
            r#"
            MATCH (source:Entity)-[r:Related]->(target:Entity)
            RETURN source.id, target.id, r.kind, r.label, r.strength, r.evidence_json,
                   r.created_at, r.updated_at, r.payload_json
            ORDER BY r.kind, source.id, target.id;
            "#,
        )
        .map_err(|error| format!("failed to read brain edges: {error}"))?;
    let mut edges = Vec::new();
    for row in &mut result {
        let kind = value_string(row.get(2));
        let source = value_string(row.get(0));
        let target = value_string(row.get(1));
        let label = value_string(row.get(3));
        let strength = value_f64(row.get(4)).unwrap_or(0.5);
        let evidence_json = value_string(row.get(5));
        let created_at = value_string(row.get(6));
        let updated_at = value_string(row.get(7));
        let payload_json = value_string(row.get(8));
        edges.push(WorkGraphEdge {
            id: graph_edge_id(&kind, &source, &target),
            source: source.clone(),
            target: target.clone(),
            kind: kind.clone(),
            label: label.clone(),
            properties: json!({
                "source": source,
                "target": target,
                "kind": kind,
                "label": label,
                "strength": strength,
                "evidence": json_from_string(&evidence_json),
                "created_at": clean_optional(created_at),
                "updated_at": clean_optional(updated_at),
                "payload": json_from_string(&payload_json),
            }),
        });
    }

    let (nodes, edges) = filter_graph(nodes, edges, &filters);
    Ok(WorkGraph {
        generated_at: read_meta(path)
            .map(|meta| meta.generated_at)
            .unwrap_or_else(|_| now_utc()),
        ai_context: graph_ai_context(&nodes, &edges),
        nodes,
        edges,
    })
}

fn filter_graph(
    nodes: Vec<WorkGraphNode>,
    edges: Vec<WorkGraphEdge>,
    filters: &BrainGraphFilters,
) -> (Vec<WorkGraphNode>, Vec<WorkGraphEdge>) {
    let node_kinds = filters
        .node_kinds
        .iter()
        .map(|kind| kind.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let relation_kinds = filters
        .relation_kinds
        .iter()
        .map(|kind| kind.to_ascii_uppercase())
        .collect::<BTreeSet<_>>();
    let query = filters
        .query
        .as_ref()
        .map(|value| value.trim().to_ascii_lowercase());

    let mut visible_nodes = nodes
        .into_iter()
        .filter(|node| {
            if !filters.include_killed_deliverables
                && node.kind == "deliverable"
                && node.status.as_deref() == Some("killed")
            {
                return false;
            }
            if !filters.include_dismissed_captures
                && node.kind == "capture"
                && node.status.as_deref() == Some("dismissed")
            {
                return false;
            }
            if !node_kinds.is_empty() && !node_kinds.contains(&node.kind.to_ascii_lowercase()) {
                return false;
            }
            if let Some(query) = &query {
                if !query.is_empty() {
                    let haystack = [
                        node.id.as_str(),
                        node.entity_id.as_str(),
                        node.kind.as_str(),
                        node.label.as_str(),
                        node.subtitle.as_deref().unwrap_or(""),
                        node.status.as_deref().unwrap_or(""),
                        node.context.as_str(),
                    ]
                    .join(" ")
                    .to_ascii_lowercase();
                    return haystack.contains(query);
                }
            }
            true
        })
        .collect::<Vec<_>>();
    visible_nodes.sort_by(|left, right| {
        right
            .weight
            .cmp(&left.weight)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.label.cmp(&right.label))
    });
    if let Some(limit) = filters.limit {
        visible_nodes.truncate(limit);
    }
    let visible_ids = visible_nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let visible_edges = edges
        .into_iter()
        .filter(|edge| {
            visible_ids.contains(&edge.source)
                && visible_ids.contains(&edge.target)
                && (relation_kinds.is_empty()
                    || relation_kinds.contains(&edge.kind.to_ascii_uppercase()))
        })
        .collect::<Vec<_>>();
    (visible_nodes, visible_edges)
}

pub(super) fn expand_neighborhood(
    seed_ids: &BTreeSet<String>,
    edges: &[WorkGraphEdge],
    max_hops: usize,
    limit: usize,
) -> BTreeSet<String> {
    let mut selected = seed_ids.clone();
    let mut queue = seed_ids
        .iter()
        .map(|id| (id.clone(), 0usize))
        .collect::<VecDeque<_>>();

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_hops || selected.len() >= limit {
            continue;
        }
        for edge in edges {
            let next = if edge.source == current {
                Some(edge.target.clone())
            } else if edge.target == current {
                Some(edge.source.clone())
            } else {
                None
            };
            if let Some(next) = next {
                if selected.insert(next.clone()) {
                    queue.push_back((next, depth + 1));
                }
            }
            if selected.len() >= limit {
                break;
            }
        }
    }

    selected
}

fn brain_brief_markdown(
    generated_at: &str,
    focus_today: &BrainTemplateResult,
    blocked_or_waiting: &BrainTemplateResult,
    email_followups: &BrainTemplateResult,
    stale_work: &BrainTemplateResult,
    pending: &[serde_json::Value],
) -> String {
    let mut lines = vec![format!("# Daily Brain Brief\n\nGenerated: {generated_at}")];
    append_brief_section(&mut lines, "Focus today", focus_today);
    append_brief_section(&mut lines, "Blocked or waiting", blocked_or_waiting);
    append_brief_section(&mut lines, "Emails needing follow-up", email_followups);
    append_brief_section(&mut lines, "Stale important work", stale_work);
    lines.push("\n## New inferred links to review".to_string());
    if pending.is_empty() {
        lines.push("- None".to_string());
    } else {
        for item in pending.iter().take(8) {
            lines.push(format!(
                "- {} -> {} ({:.0}%): {}",
                item.get("source_kind")
                    .and_then(|value| value.as_str())
                    .unwrap_or("source"),
                item.get("target_kind")
                    .and_then(|value| value.as_str())
                    .unwrap_or("target"),
                item.get("confidence")
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0)
                    * 100.0,
                item.get("rationale")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
            ));
        }
    }
    lines.join("\n")
}

fn append_brief_section(lines: &mut Vec<String>, title: &str, result: &BrainTemplateResult) {
    lines.push(format!("\n## {title}"));
    if result.rows.is_empty() {
        lines.push("- None".to_string());
        return;
    }
    for row in result.rows.iter().take(6) {
        lines.push(format!(
            "- [{}] {}{}",
            row.get("kind")
                .and_then(|value| value.as_str())
                .unwrap_or("item"),
            row.get("title")
                .and_then(|value| value.as_str())
                .unwrap_or("Untitled"),
            row.get("status")
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .map(|value| format!(" ({value})"))
                .unwrap_or_default()
        ));
    }
}

fn add_ask_ref_edges(row: &AskTurnRow, projection: &mut BrainProjection) {
    let Ok(refs) = serde_json::from_str::<serde_json::Value>(&row.refs_json) else {
        return;
    };
    let Some(items) = refs.as_array() else {
        return;
    };
    for item in items {
        let kind = item
            .get("kind")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let entity_id = item
            .get("entity_id")
            .or_else(|| item.get("id"))
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if entity_id.is_empty() {
            continue;
        }
        if let Some(target) = source_node_id(kind, entity_id) {
            projection.push_edge(relation(
                &graph_node_id("ask_turn", &row.id),
                &target,
                "MENTIONS",
                "mentions",
                0.5,
                json!({ "source": "ask_turns.refs_json" }),
                &row.created_at,
                &row.updated_at,
                json!({ "ref_kind": kind }),
            ));
        }
    }
}

pub(super) fn source_node_id(source_kind: &str, source_id: &str) -> Option<String> {
    let kind = match source_kind {
        "initiative" | "initiatives" => "initiative",
        "deliverable" | "deliverables" => "deliverable",
        "task" | "deliverable_task" | "deliverable_tasks" => "task",
        "note" | "deliverable_note" | "deliverable_notes" => "note",
        "stakeholder" | "stakeholders" => "stakeholder",
        "conversation" | "conversations" => "conversation",
        "capture" | "captures" => "capture",
        "memory" | "memories" => "memory",
        "meeting" | "meetings" => "meeting",
        "meeting_action" | "meeting_actions" => "meeting_action",
        "gmail_thread" | "email_thread" | "thread" => "email_thread",
        "gmail_message" | "email_message" | "message" => "email_message",
        "gmail_followup" | "gmail_followups" | "email_followup" | "email_followups" => {
            "email_followup"
        }
        "ask_chat" | "ask_chats" => "ask_chat",
        "ask_turn" | "ask_turns" => "ask_turn",
        "work_intake_suggestion" | "work_intake_suggestions" => "work_intake_suggestion",
        "blocker" | "blockers" => "blocker",
        "calendar_event" | "gcal_event" | "gcal_events" => "calendar_event",
        "file" | "files" => "file",
        "trace_folder" | "trace_folders" | "folder" | "folders" => "trace_folder",
        "open_loop" | "open_loops" => "open_loop",
        "attention_signal" | "attention_signals" => "attention_signal",
        "inference" | "brain_inference" | "brain_inferences" => "inference",
        _ => return None,
    };
    Some(graph_node_id(kind, source_id))
}

fn meeting_action_target_node(kind: &str, target_id: &str) -> Option<String> {
    match kind {
        "deliverable" | "create_deliverable" | "update_deliverable" => {
            Some(graph_node_id("deliverable", target_id))
        }
        "task" | "create_task" | "update_task" => Some(graph_node_id("task", target_id)),
        "initiative" | "create_initiative" | "update_initiative" => {
            Some(graph_node_id("initiative", target_id))
        }
        "note" | "create_note" => Some(graph_node_id("note", target_id)),
        _ => source_node_id(kind, target_id),
    }
}

fn add_attention_signal(
    projection: &mut BrainProjection,
    signal_type: &str,
    source_id: &str,
    title: &str,
    summary: &str,
    source_node: &str,
    updated_at: &str,
    importance: f64,
    payload: serde_json::Value,
) {
    let signal_source_id = format!("{signal_type}:{source_id}");
    projection.push_node(entity(
        "attention_signal",
        "derived_attention",
        &signal_source_id,
        &format!("{signal_type}: {title}"),
        summary,
        signal_type,
        None,
        updated_at,
        updated_at,
        importance,
        json!({
            "signal_type": signal_type,
            "source_id": source_id,
            "payload": payload,
        }),
    ));
    projection.push_edge(relation(
        source_node,
        &graph_node_id("attention_signal", &signal_source_id),
        "HAS_ATTENTION",
        "has attention signal",
        importance,
        json!({ "source": "derived_attention" }),
        updated_at,
        updated_at,
        json!({ "signal_type": signal_type }),
    ));
}

pub(super) fn entity(
    kind: &str,
    source_table: &str,
    source_id: &str,
    title: &str,
    summary: &str,
    status: &str,
    route: Option<String>,
    created_at: &str,
    updated_at: &str,
    importance: f64,
    payload: serde_json::Value,
) -> BrainEntity {
    BrainEntity {
        id: graph_node_id(kind, source_id),
        kind: kind.to_string(),
        source_table: source_table.to_string(),
        source_id: source_id.to_string(),
        title: truncate(title, 180),
        summary: truncate(summary, 1800),
        status: status.to_string(),
        route: route.unwrap_or_default(),
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
        importance: importance.clamp(0.05, 1.5),
        payload_json: serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
    }
}

pub(super) fn relation(
    source: &str,
    target: &str,
    kind: &str,
    label: &str,
    strength: f64,
    evidence: serde_json::Value,
    created_at: &str,
    updated_at: &str,
    payload: serde_json::Value,
) -> BrainRelation {
    BrainRelation {
        source: source.to_string(),
        target: target.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        strength: strength.clamp(0.05, 1.5),
        evidence_json: serde_json::to_string(&evidence).unwrap_or_else(|_| "{}".to_string()),
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
        payload_json: serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
    }
}

pub(super) fn graph_node_id(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

pub(super) fn graph_edge_id(kind: &str, source: &str, target: &str) -> String {
    format!("{kind}:{source}->{target}")
}

fn email_importance(priority: Option<&str>, has_unread: bool) -> f64 {
    let base = match priority.unwrap_or("low") {
        "urgent" | "high" => 0.75,
        "medium" => 0.55,
        _ => 0.35,
    };
    if has_unread {
        base + 0.15
    } else {
        base
    }
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

pub(super) fn parse_date_prefix(value: &str) -> Option<NaiveDate> {
    let prefix = value.get(0..10)?;
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

fn is_today(value: &str) -> bool {
    parse_date_prefix(value) == Some(Utc::now().date_naive())
}

pub(super) fn date_is_before_today(value: &str) -> bool {
    parse_date_prefix(value)
        .map(|date| date < Utc::now().date_naive())
        .unwrap_or(false)
}

pub(super) fn date_is_within_days(value: &str, days: i64) -> bool {
    let today = Utc::now().date_naive();
    parse_date_prefix(value)
        .map(|date| date >= today && date <= today + Duration::days(days))
        .unwrap_or(false)
}

fn hidden_by_default(kind: &str, status: &str) -> bool {
    status == "dismissed"
        || status == "killed"
        || matches!(
            kind,
            "email_message"
                | "email_attachment"
                | "ask_turn"
                | "ask_attachment"
                | "state_history"
                | "email_label"
        )
}

fn optional_sentence(label: &str, value: &str) -> String {
    if value.trim().is_empty() {
        String::new()
    } else {
        format!(". {label}: {value}")
    }
}

fn clean_optional(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn join_nonempty<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    parts
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn truncate(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let take = max_chars.saturating_sub(3);
    format!("{}...", trimmed.chars().take(take).collect::<String>())
}

pub(super) fn stable_suffix(value: &str) -> String {
    let mut hash = 5381u64;
    for byte in value.as_bytes() {
        hash = (hash.wrapping_shl(5)).wrapping_add(hash) ^ u64::from(*byte);
    }
    format!("{hash:x}")
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null(_)) | None => String::new(),
        Some(value) => value.to_string(),
    }
}

fn value_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Double(value)) => Some(*value),
        Some(Value::Float(value)) => Some(f64::from(*value)),
        Some(Value::Int64(value)) => Some(*value as f64),
        Some(Value::Int32(value)) => Some(f64::from(*value)),
        Some(Value::Int16(value)) => Some(f64::from(*value)),
        Some(Value::Int8(value)) => Some(f64::from(*value)),
        _ => None,
    }
}

pub(super) fn json_from_string(value: &str) -> serde_json::Value {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return serde_json::Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| json!(trimmed))
}

pub(super) fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub(super) fn sql_error(error: sqlx::Error) -> String {
    format!("database error: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        brain::{
            cypher::validate_read_only_cypher, get_brain_learning_snapshot, record_brain_feedback,
            record_brain_learning_event,
        },
        db,
        models::{BrainFeedbackInput, BrainLearningEventInput},
    };

    #[tokio::test]
    async fn rebuild_creates_expected_nodes_and_edges() {
        let pool = db::connect_memory().await.unwrap();
        seed_brain_fixture(&pool).await;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(BRAIN_FILE_NAME);

        let status = rebuild_brain(&pool, &path).await.unwrap();
        assert!(status.exists);
        assert!(status.node_count >= 7);

        let graph = get_brain_graph(&pool, &path, BrainGraphFilters::default())
            .await
            .unwrap();
        assert!(graph.nodes.iter().any(|node| node.kind == "task"));
        assert!(graph.nodes.iter().any(|node| node.kind == "note"));
        assert!(graph.nodes.iter().any(|node| node.kind == "meeting_action"));
        assert!(graph.nodes.iter().any(|node| node.kind == "email_thread"));
        assert!(graph.nodes.iter().any(|node| node.kind == "email_followup"));
        assert!(graph.nodes.iter().any(|node| node.kind == "calendar_event"));
        assert!(graph.nodes.iter().any(|node| node.kind == "trace_folder"));
        assert!(graph.nodes.iter().any(|node| node.kind == "open_loop"));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == "attention_signal"));
        assert!(graph.nodes.iter().any(|node| node.kind == "ask_chat"));
        assert!(graph.nodes.iter().any(|node| node.kind == "memory"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "CONTAINS"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "TARGETS"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "RELATED_TO"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "ATTENDED_BY"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "HAS_FOLLOWUP"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "SCHEDULED_FOR"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "ATTACHED_TO"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "IDENTIFIES"));
        assert!(graph.edges.iter().any(|edge| edge.kind == "HAS_OPEN_LOOP"));
    }

    #[tokio::test]
    async fn rebuild_is_idempotent() {
        let pool = db::connect_memory().await.unwrap();
        seed_brain_fixture(&pool).await;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(BRAIN_FILE_NAME);

        let first = rebuild_brain(&pool, &path).await.unwrap();
        let second = rebuild_brain(&pool, &path).await.unwrap();
        assert_eq!(first.node_count, second.node_count);
        assert_eq!(first.edge_count, second.edge_count);
    }

    #[tokio::test]
    async fn brain_templates_follow_graph_paths() {
        let pool = db::connect_memory().await.unwrap();
        seed_brain_fixture(&pool).await;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(BRAIN_FILE_NAME);

        rebuild_brain(&pool, &path).await.unwrap();

        let focus = run_brain_template(
            &pool,
            &path,
            BrainTemplateInput {
                template: BrainTemplateKind::FocusToday,
                focus_entity_id: None,
                limit: Some(30),
            },
        )
        .await
        .unwrap();
        assert!(focus.cypher.contains("Related*1..3"));
        assert!(focus
            .graph
            .nodes
            .iter()
            .any(|node| node.kind == "attention_signal"));
        assert!(focus
            .graph
            .nodes
            .iter()
            .any(|node| node.kind == "deliverable"));

        let followups = run_brain_template(
            &pool,
            &path,
            BrainTemplateInput {
                template: BrainTemplateKind::EmailFollowups,
                focus_entity_id: None,
                limit: Some(30),
            },
        )
        .await
        .unwrap();
        assert!(followups.cypher.contains("HAS_FOLLOWUP"));
        assert!(followups
            .graph
            .nodes
            .iter()
            .any(|node| node.kind == "email_followup"));
        assert!(followups
            .graph
            .nodes
            .iter()
            .any(|node| node.kind == "deliverable"));
    }

    #[tokio::test]
    async fn feedback_can_accept_corrected_inferred_relationship() {
        let pool = db::connect_memory().await.unwrap();
        seed_brain_fixture(&pool).await;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(BRAIN_FILE_NAME);

        record_brain_feedback(
            &pool,
            BrainFeedbackInput {
                question: "which email mentions the Kuzu task?".to_string(),
                template: Some("email_followups".to_string()),
                feedback: "useful".to_string(),
                corrected: Some(json!({
                    "corrected_relationship": {
                        "source_kind": "email_thread",
                        "source_id": "thread1",
                        "relation_kind": "MENTIONS",
                        "target_kind": "task",
                        "target_id": "task1",
                        "confidence": 0.97,
                        "rationale": "User confirmed the email mentions the implementation task."
                    }
                })),
            },
        )
        .await
        .unwrap();

        rebuild_brain(&pool, &path).await.unwrap();
        let graph = get_brain_graph(&pool, &path, BrainGraphFilters::default())
            .await
            .unwrap();
        assert!(graph.nodes.iter().any(|node| node.kind == "inference"));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == "MENTIONS"
                && edge.source == graph_node_id("email_thread", "thread1")
                && edge.target == graph_node_id("task", "task1")
        }));
    }

    #[tokio::test]
    async fn brain_learning_event_updates_template_policy() {
        let pool = db::connect_memory().await.unwrap();
        seed_brain_fixture(&pool).await;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(BRAIN_FILE_NAME);

        let before = run_brain_template(
            &pool,
            &path,
            BrainTemplateInput {
                template: BrainTemplateKind::FocusToday,
                focus_entity_id: None,
                limit: Some(30),
            },
        )
        .await
        .unwrap();
        let deliverable = before
            .graph
            .nodes
            .iter()
            .find(|node| node.kind == "deliverable")
            .unwrap();
        let features = deliverable
            .properties
            .get("brain_rl")
            .and_then(|value| value.get("features"))
            .cloned()
            .unwrap();

        record_brain_learning_event(
            &pool,
            BrainLearningEventInput {
                template: Some("focus_today".to_string()),
                item_id: deliverable.id.clone(),
                item_kind: Some(deliverable.kind.clone()),
                event_type: "completed_after_seen".to_string(),
                reward: None,
                context: Some(json!({ "features": features })),
            },
        )
        .await
        .unwrap();

        let snapshot =
            get_brain_learning_snapshot(&pool, Some("focus_today".to_string()), Some(10))
                .await
                .unwrap();
        assert_eq!(snapshot.policies.len(), 1);
        assert_eq!(snapshot.policies[0].observations, 1);
        assert_eq!(snapshot.recent_events.len(), 1);
    }

    #[tokio::test]
    async fn read_only_cypher_rejects_write_keywords() {
        for keyword in ["CREATE", "MERGE", "SET", "DELETE", "DROP", "COPY", "LOAD"] {
            let query = format!("MATCH (e:Entity) {keyword} e RETURN e");
            assert!(validate_read_only_cypher(&query).is_err(), "{keyword}");
        }
        assert!(validate_read_only_cypher("MATCH (e:Entity) RETURN e.id LIMIT 5").is_ok());
    }

    async fn seed_brain_fixture(pool: &SqlitePool) {
        let now = "2026-05-09T00:00:00.000Z";
        sqlx::query("INSERT INTO initiatives (id, title, framing, status, created_at, updated_at) VALUES ('init1', 'Brain Initiative', 'Make Trace remember work context.', 'live', ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO stakeholders (id, name, display_order, email, role, notes) VALUES ('stake1', 'Ada', 1, 'ada@example.com', 'Reviewer', 'Cares about graph quality')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO deliverables (id, title, type, state, claim, stakeholder_id, created_at, updated_at, is_focused, priority) VALUES ('del1', 'Kuzu graph', 'code', 'drafting', 'Local graph brain', 'stake1', ?, ?, 1, 'high')")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO deliverable_initiatives (deliverable_id, initiative_id) VALUES ('del1', 'init1')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO deliverable_stakeholders (deliverable_id, stakeholder_id) VALUES ('del1', 'stake1')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO deliverable_tasks (id, deliverable_id, title, status, display_order, created_at, updated_at) VALUES ('task1', 'del1', 'Wire Kuzu', 'todo', 1, ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO deliverable_notes (id, deliverable_id, body, created_at) VALUES ('note1', 'del1', 'Keep raw email bodies out of graph.', ?)")
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO captures (id, kind, body, status, promoted_task_id, promoted_task_title, created_at, updated_at, promoted_at) VALUES ('cap1', 'thought', 'Turn this into a graph task.', 'promoted', 'task1', 'Wire Kuzu', ?, ?, ?)")
            .bind(now)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO meetings (id, title, date, summary, key_decisions, status, created_at, updated_at) VALUES ('meet1', 'Brain sync', '2026-05-09', 'Discussed graph brain.', 'Use Kuzu.', 'done', ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO meeting_stakeholders (meeting_id, stakeholder_id) VALUES ('meet1', 'stake1')")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO meeting_actions (id, meeting_id, kind, target_id, target_title, body, applied, created_at, payload) VALUES ('act1', 'meet1', 'deliverable', 'del1', 'Kuzu graph', 'Implement graph brain.', 1, ?, '{}')")
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO gmail_threads (thread_id, subject, snippet, participants, message_count, has_unread, last_from_name, last_from_email, last_sync_at, ai_category, ai_priority) VALUES ('thread1', 'Graph thread', 'Please connect this to the deliverable.', '[]', 1, 0, 'Ada', 'ada@example.com', ?, 'work', 'high')")
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO gmail_participants (email, name, first_seen_at, last_seen_at, sent_count, received_count, thread_count) VALUES ('ada@example.com', 'Ada', ?, ?, 1, 2, 1)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO gmail_thread_participants (thread_id, email, name, role, message_count, first_seen_at, last_seen_at) VALUES ('thread1', 'ada@example.com', 'Ada', 'from', 1, ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO gmail_thread_deliverables (thread_id, deliverable_id, linked_at, source) VALUES ('thread1', 'del1', ?, 'manual')")
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO gmail_followups (id, thread_id, sent_at, due_at, status, created_at, updated_at) VALUES ('follow1', 'thread1', ?, '2026-05-10T00:00:00.000Z', 'open', ?, ?)")
            .bind(now)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO gcal_events (id, gcal_event_id, title, description, start_date, is_all_day, html_link, created_at, updated_at) VALUES ('event1', 'gcal1', 'Kuzu graph review', 'Review graph topology.', '2026-05-09', 1, 'https://calendar.test/event', ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO gcal_sync_map (entity_type, entity_id, gcal_event_id, last_synced_at) VALUES ('deliverable', 'del1', 'gcal1', ?)")
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO trace_folders (id, parent_id, name, created_at, updated_at) VALUES ('folder1', NULL, 'Brain Files', ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO files (id, kind, trace_folder_id, name, description, created_at, updated_at) VALUES ('file1', 'local', 'folder1', 'graph-notes.md', 'Graph notes', ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO file_links (file_id, entity_kind, entity_id, linked_at, source) VALUES ('file1', 'deliverable_task', 'task1', ?, 'manual')")
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO folder_entity_links (folder_id, entity_kind, entity_id, linked_at) VALUES ('folder1', 'deliverable', 'del1', ?)")
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ask_chats (id, title, mode, summary, created_at, updated_at) VALUES ('chat1', 'Brain ask', 'agentic', 'Asked about graph context.', ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO ask_turns (id, chat_id, question, answer, reasoning, status, refs_json, questions_json, steps_json, created_at, updated_at) VALUES ('turn1', 'chat1', 'What is blocked?', 'Nothing yet.', '', 'done', '[{\"kind\":\"deliverable\",\"entity_id\":\"del1\"}]', '[]', '[]', ?, ?)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memories (id, kind, status, scope, title, body, canonical_key, source, confidence, importance, tags_json, evidence_json, created_at, updated_at, sensitivity, pinned) VALUES ('mem1', 'semantic', 'active', 'global', 'Graph privacy', 'Do not store raw Gmail bodies in the graph.', 'graph_privacy', 'manual', 0.95, 0.8, '[]', '[]', ?, ?, 'normal', 0)")
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .unwrap();
    }
}
