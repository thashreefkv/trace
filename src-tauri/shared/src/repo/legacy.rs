use std::collections::{BTreeMap, BTreeSet};

use chrono::{Duration, SecondsFormat, Utc};
use serde_json::json;
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{
        AppendAskTurnInput, ApplyGeneratedTasksInput, AskChatDetail, AskChatRecord,
        AskChatSearchHit, AskTurnAttachmentRecord, AskTurnRecord, BriefingItem, BriefingSections,
        Capture, CaptureFilters, CaptureStatus, CreateDeliverableInput, CreateInitiativeInput,
        CreateSectionInput, Deliverable, DeliverableFilters, DeliverableState, DeliverableType,
        GanttDeliverable, Initiative, InitiativeGantt, InitiativeNote, InitiativeSection,
        InitiativeState, InitiativeStatus, ListAskChatsFilters, ListMemoryFilters, Meeting,
        MeetingAction, MeetingRow, MemoryRecord, MemoryStatus, Stakeholder, StakeholderBriefing,
        UpdateGanttDatesInput, UpdateSectionInput, UpdateUserProfileInput, UpsertAskChatInput,
        UserProfile, WeekDay, WeekTask, WeekView, WorkGraph, WorkGraphEdge, WorkGraphFilters,
        WorkGraphNode, WorkIntakeApplyResult, WorkIntakeFilters, WorkIntakeSuggestion,
    },
};

use super::{
    action_payload, create_deliverable, create_initiative, fetch_stakeholders_for_meeting,
    get_deliverable, get_initiative, get_meeting_config, get_stakeholder, list_captures,
    list_conversations, list_deliverables, list_deliverables_for_initiative, list_memories,
    list_stakeholders, parse_agentic_deliverable_type, resolve_initiative_title,
    resolve_stakeholder_name,
};

#[cfg(test)]
use super::{
    apply_meeting_action, commit_conversation_ingest, create_conversation, create_meeting,
    create_stakeholder, delete_initiative, get_capture, normalize_claude_link,
    promote_claude_capture_to_ingest, save_minutes_summary, update_initiative,
    validate_capture_input,
};
#[cfg(test)]
use crate::models::{
    ApplyMeetingActionInput, CaptureKind, CommitConversationIngestInput,
    CommitExtractedDeliverableInput, Conversation, ConversationExtractionResult,
    ConversationIngestResult, CreateCaptureInput, CreateConversationInput, CreateMeetingInput,
    CreateStakeholderInput, ExtractedConversation, UpdateInitiativeInput,
};

pub fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn clean_required(value: String, label: &str) -> Result<String, String> {
    let cleaned = value.trim().to_string();
    if cleaned.is_empty() {
        return Err(format!("{label} is required"));
    }
    Ok(cleaned)
}

pub fn clean_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(12)
        .collect()
}

pub fn json_string<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("failed to encode memory JSON: {error}"))
}

pub fn clamp_score(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

pub fn bool_as_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

pub async fn list_initiatives_by_title(pool: &SqlitePool) -> Result<Vec<Initiative>, String> {
    sqlx::query_as::<_, Initiative>(
        r#"
        SELECT id, title, framing, status,
               COALESCE(icon, 'target') AS icon,
               COALESCE(icon_color, '#6366f1') AS icon_color,
               created_at, updated_at
        FROM initiatives
        ORDER BY title ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn get_initiative_state_by_title(
    pool: &SqlitePool,
    title: &str,
) -> Result<InitiativeState, String> {
    let initiative_id = resolve_initiative_title(pool, title).await?;
    let initiative = get_initiative(pool, &initiative_id).await?;
    let mut deliverables = list_deliverables_for_initiative(pool, &initiative_id).await?;
    deliverables.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

    let drafting_count = deliverables
        .iter()
        .filter(|deliverable| deliverable.state == "drafting")
        .count() as i64;
    let in_review_count = deliverables
        .iter()
        .filter(|deliverable| deliverable.state == "in_review")
        .count() as i64;
    let shipped_count = deliverables
        .iter()
        .filter(|deliverable| deliverable.state == "shipped")
        .count() as i64;
    let killed_count = deliverables
        .iter()
        .filter(|deliverable| deliverable.state == "killed")
        .count() as i64;

    Ok(InitiativeState {
        initiative,
        deliverables,
        drafting_count,
        in_review_count,
        shipped_count,
        killed_count,
    })
}

pub async fn deliverables_for_stakeholder_since(
    pool: &SqlitePool,
    stakeholder_name: &str,
    since_days: i64,
) -> Result<(Stakeholder, Vec<Deliverable>), String> {
    let stakeholder_id = resolve_stakeholder_name(pool, stakeholder_name).await?;
    let stakeholder = get_stakeholder(pool, &stakeholder_id).await?;
    let since = (Utc::now() - Duration::days(since_days.max(1)))
        .to_rfc3339_opts(SecondsFormat::Millis, true);

    let deliverables = list_deliverables(
        pool,
        DeliverableFilters {
            stakeholder_id: Some(stakeholder_id),
            ..DeliverableFilters::default()
        },
    )
    .await?
    .into_iter()
    .filter(|deliverable| deliverable.updated_at >= since)
    .collect::<Vec<_>>();

    Ok((stakeholder, deliverables))
}

pub async fn deterministic_stakeholder_briefing(
    pool: &SqlitePool,
    stakeholder_name: &str,
    since_days: i64,
) -> Result<StakeholderBriefing, String> {
    let (stakeholder, deliverables) =
        deliverables_for_stakeholder_since(pool, stakeholder_name, since_days).await?;
    let sections = build_fallback_sections(&deliverables);
    Ok(StakeholderBriefing {
        stakeholder,
        generated_with: "fallback".to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        sections,
    })
}

pub fn build_fallback_sections(deliverables: &[Deliverable]) -> BriefingSections {
    let mut recent_wins: Vec<BriefingItem> = Vec::new();
    let mut in_flight: Vec<BriefingItem> = Vec::new();
    let mut open_commitments: Vec<BriefingItem> = Vec::new();

    for d in deliverables {
        match d.state.as_str() {
            "shipped" => {
                if recent_wins.len() < 3 {
                    recent_wins.push(BriefingItem {
                        text: if d.claim.is_empty() {
                            d.title.clone()
                        } else {
                            format!("{} — {}", d.title, d.claim)
                        },
                        source: Some("deliverable".to_string()),
                    });
                }
            }
            "in_review" | "drafting" => {
                if in_flight.len() < 4 {
                    in_flight.push(BriefingItem {
                        text: if d.claim.is_empty() {
                            d.title.clone()
                        } else {
                            format!("{} — {}", d.title, d.claim)
                        },
                        source: Some("deliverable".to_string()),
                    });
                }
                if d.state == "in_review" && open_commitments.len() < 4 {
                    open_commitments.push(BriefingItem {
                        text: format!("Deliver {} (awaiting review)", d.title),
                        source: Some("deliverable".to_string()),
                    });
                }
            }
            _ => {}
        }
    }

    let tldr = if deliverables.is_empty() {
        "No deliverables tracked for this stakeholder yet.".to_string()
    } else {
        format!(
            "{} shipped, {} in-flight across {} tracked deliverables.",
            recent_wins.len(),
            in_flight.len(),
            deliverables.len()
        )
    };

    BriefingSections {
        tldr,
        open_commitments,
        waiting_on_them: Vec::new(),
        recent_wins,
        in_flight,
        talking_points: Vec::new(),
        watch_out: None,
    }
}

pub async fn list_meetings_for_stakeholder(
    pool: &SqlitePool,
    stakeholder_id: &str,
) -> Result<Vec<Meeting>, String> {
    let rows = sqlx::query_as::<_, MeetingRow>(
        r#"
        SELECT m.id, m.title, m.date, m.duration_secs, m.transcript, m.summary,
               m.key_decisions, m.status, m.error_message, m.created_at, m.updated_at
        FROM meetings m
        JOIN meeting_stakeholders ms ON ms.meeting_id = m.id
        WHERE ms.stakeholder_id = ?
        ORDER BY m.date DESC
        LIMIT 8
        "#,
    )
    .bind(stakeholder_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut meetings = Vec::with_capacity(rows.len());
    for row in rows {
        let stakeholders = fetch_stakeholders_for_meeting(pool, &row.id).await?;
        meetings.push(row.with_stakeholders(stakeholders));
    }
    Ok(meetings)
}

pub async fn menu_bar_state(pool: &SqlitePool) -> Result<crate::models::MenuBarState, String> {
    #[derive(sqlx::FromRow)]
    struct ActiveDeliverableRow {
        id: String,
        title: String,
    }

    let active = sqlx::query_as::<_, ActiveDeliverableRow>(
        r#"
        SELECT id, title
        FROM deliverables
        WHERE state IN ('drafting', 'in_review')
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let seven_days_ago =
        (Utc::now() - Duration::days(7)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let shipped_this_week: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM deliverables
        WHERE state = 'shipped'
          AND shipped_at IS NOT NULL
          AND shipped_at >= ?
        "#,
    )
    .bind(seven_days_ago)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    let active_deliverable_id = active.as_ref().map(|row| row.id.clone());
    let active_deliverable_title = active.as_ref().map(|row| row.title.clone());
    let tray_title = active_deliverable_title
        .as_deref()
        .map(truncate_tray_title)
        .unwrap_or_else(|| "Trace".to_string());

    Ok(crate::models::MenuBarState {
        active_deliverable_id,
        active_deliverable_title,
        tray_title,
        shipped_this_week,
    })
}

// ── First-class memory ────────────────────────────────────────────────────────

pub async fn get_work_context_graph(
    pool: &SqlitePool,
    filters: WorkGraphFilters,
) -> Result<WorkGraph, String> {
    let initiatives = list_initiatives_by_title(pool).await?;
    let stakeholders = list_stakeholders(pool).await?;
    let conversations = list_conversations(pool).await?;
    let deliverables = list_deliverables(pool, DeliverableFilters::default()).await?;
    let captures = list_captures(pool, CaptureFilters::default()).await?;
    let memories = list_memories(
        pool,
        ListMemoryFilters {
            status: Some(MemoryStatus::Active),
            ..Default::default()
        },
    )
    .await?;

    let visible_deliverables = deliverables
        .into_iter()
        .filter(|deliverable| filters.include_killed_deliverables || deliverable.state != "killed")
        .collect::<Vec<_>>();
    let visible_captures = captures
        .into_iter()
        .filter(|capture| filters.include_dismissed_captures || capture.status != "dismissed")
        .collect::<Vec<_>>();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for initiative in &initiatives {
        let linked_count = visible_deliverables
            .iter()
            .filter(|deliverable| {
                deliverable
                    .initiatives
                    .iter()
                    .any(|linked| linked.id == initiative.id)
            })
            .count() as i64;

        nodes.push(WorkGraphNode {
            id: graph_node_id("initiative", &initiative.id),
            entity_id: initiative.id.clone(),
            kind: "initiative".to_string(),
            label: initiative.title.clone(),
            subtitle: clean_optional(Some(initiative.framing.clone())),
            status: Some(initiative.status.clone()),
            url: Some(format!("/initiatives/{}", initiative.id)),
            updated_at: Some(initiative.updated_at.clone()),
            hidden_by_default: false,
            weight: linked_count.max(1),
            context: format!(
                "Initiative '{}' is {} with {} visible linked deliverable(s). Framing: {}",
                initiative.title,
                initiative.status,
                linked_count,
                empty_as_placeholder(&initiative.framing)
            ),
            properties: json!({
                "id": &initiative.id,
                "kind": "initiative",
                "title": &initiative.title,
                "framing": &initiative.framing,
                "status": &initiative.status,
                "created_at": &initiative.created_at,
                "updated_at": &initiative.updated_at,
                "linked_deliverable_count": linked_count,
            }),
        });
    }

    for stakeholder in &stakeholders {
        let linked_count = visible_deliverables
            .iter()
            .filter(|deliverable| {
                deliverable
                    .stakeholders
                    .iter()
                    .any(|linked| linked.id == stakeholder.id)
            })
            .count() as i64;

        nodes.push(WorkGraphNode {
            id: graph_node_id("stakeholder", &stakeholder.id),
            entity_id: stakeholder.id.clone(),
            kind: "stakeholder".to_string(),
            label: stakeholder.name.clone(),
            subtitle: clean_optional(Some(stakeholder.role.clone())),
            status: clean_optional(Some(format!("{linked_count} deliverable(s)"))),
            url: Some(format!("/stakeholders/{}", stakeholder.id)),
            updated_at: None,
            hidden_by_default: false,
            weight: linked_count.max(1),
            context: format!(
                "Stakeholder '{}'{} has {} visible linked deliverable(s). Notes: {}",
                stakeholder.name,
                optional_context_fragment(" role", &stakeholder.role),
                linked_count,
                empty_as_placeholder(&stakeholder.notes)
            ),
            properties: json!({
                "id": &stakeholder.id,
                "kind": "stakeholder",
                "name": &stakeholder.name,
                "role": &stakeholder.role,
                "notes": &stakeholder.notes,
                "linked_deliverable_count": linked_count,
            }),
        });
    }

    for conversation in &conversations {
        nodes.push(WorkGraphNode {
            id: graph_node_id("conversation", &conversation.id),
            entity_id: conversation.id.clone(),
            kind: "conversation".to_string(),
            label: conversation
                .title
                .clone()
                .unwrap_or_else(|| "Untitled conversation".to_string()),
            subtitle: conversation.summary.clone(),
            status: None,
            url: Some(conversation.chat_url.clone()),
            updated_at: Some(conversation.ingested_at.clone()),
            hidden_by_default: false,
            weight: visible_deliverables
                .iter()
                .filter(|deliverable| {
                    deliverable.conversation_id.as_deref() == Some(&conversation.id)
                })
                .count()
                .max(1) as i64,
            context: format!(
                "Conversation '{}'. Summary: {}",
                conversation
                    .title
                    .as_deref()
                    .unwrap_or("Untitled conversation"),
                conversation.summary.as_deref().unwrap_or("No summary.")
            ),
            properties: json!({
                "id": &conversation.id,
                "kind": "conversation",
                "title": &conversation.title,
                "summary": &conversation.summary,
                "chat_url": &conversation.chat_url,
                "ingested_at": &conversation.ingested_at,
            }),
        });
    }

    for memory in &memories {
        nodes.push(WorkGraphNode {
            id: graph_node_id("memory", &memory.id),
            entity_id: memory.id.clone(),
            kind: "memory".to_string(),
            label: memory.title.clone(),
            subtitle: Some(memory.body.clone()),
            status: Some(format!("{} · {}", memory.kind, memory.source)),
            url: None,
            updated_at: Some(memory.updated_at.clone()),
            hidden_by_default: false,
            weight: ((memory.importance * 10.0).round() as i64).clamp(1, 10),
            context: format!(
                "Memory '{}'. Type: {}. Source: {}. Confidence: {:.2}. Importance: {:.2}. Body: {}",
                memory.title,
                memory.kind,
                memory.source,
                memory.confidence,
                memory.importance,
                memory.body
            ),
            properties: json!({
                "id": &memory.id,
                "kind": "memory",
                "memory_kind": &memory.kind,
                "status": &memory.status,
                "scope": &memory.scope,
                "title": &memory.title,
                "body": &memory.body,
                "source": &memory.source,
                "source_kind": &memory.source_kind,
                "source_id": &memory.source_id,
                "confidence": memory.confidence,
                "importance": memory.importance,
                "tags": &memory.tags,
                "created_at": &memory.created_at,
                "updated_at": &memory.updated_at,
            }),
        });

        if let (Some(source_kind), Some(source_id)) = (&memory.source_kind, &memory.source_id) {
            let target = match source_kind.as_str() {
                "initiative" | "stakeholder" | "conversation" | "deliverable" => {
                    Some(graph_node_id(source_kind, source_id))
                }
                _ => None,
            };
            if let Some(target) = target {
                edges.push(WorkGraphEdge {
                    id: graph_edge_id("memory_source", &memory.id, source_id),
                    source: graph_node_id("memory", &memory.id),
                    target: target.clone(),
                    kind: "memory_source".to_string(),
                    label: "source".to_string(),
                    properties: json!({
                        "kind": "memory_source",
                        "label": "source",
                        "source": graph_node_id("memory", &memory.id),
                        "target": target,
                    }),
                });
            }
        }
    }

    for deliverable in &visible_deliverables {
        let hidden_by_default = deliverable.state == "killed";
        let stakeholder_names = stakeholder_names(deliverable);
        let initiative_titles = deliverable
            .initiatives
            .iter()
            .map(|initiative| initiative.title.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        nodes.push(WorkGraphNode {
            id: graph_node_id("deliverable", &deliverable.id),
            entity_id: deliverable.id.clone(),
            kind: "deliverable".to_string(),
            label: deliverable.title.clone(),
            subtitle: clean_optional(Some(deliverable.claim.clone())),
            status: Some(deliverable.state.clone()),
            url: Some(format!("/deliverables/{}", deliverable.id)),
            updated_at: Some(deliverable.updated_at.clone()),
            hidden_by_default,
            weight: deliverable_weight(deliverable),
            context: format!(
                "Deliverable '{}' is {} {}. Stakeholders: {}. Initiatives: {}. Claim: {}{}{}",
                deliverable.title,
                deliverable.deliverable_type,
                deliverable.state,
                if stakeholder_names.is_empty() {
                    "None".to_string()
                } else {
                    stakeholder_names
                },
                if initiative_titles.is_empty() {
                    "None".to_string()
                } else {
                    initiative_titles
                },
                deliverable.claim,
                optional_date_context("deadline", deliverable.deadline.as_deref()),
                optional_context_fragment(
                    " blocker",
                    deliverable.blocker_reason.as_deref().unwrap_or("")
                )
            ),
            properties: json!({
                "id": &deliverable.id,
                "kind": "deliverable",
                "title": &deliverable.title,
                "type": &deliverable.deliverable_type,
                "state": &deliverable.state,
                "claim": &deliverable.claim,
                "deadline": &deliverable.deadline,
                "priority": &deliverable.priority,
                "stakeholders": &deliverable.stakeholders,
                "initiatives": &deliverable.initiatives,
                "blocker_reason": &deliverable.blocker_reason,
                "created_at": &deliverable.created_at,
                "updated_at": &deliverable.updated_at,
            }),
        });
    }

    for capture in &visible_captures {
        let hidden_by_default = capture.status == "dismissed";
        nodes.push(WorkGraphNode {
            id: graph_node_id("capture", &capture.id),
            entity_id: capture.id.clone(),
            kind: "capture".to_string(),
            label: truncate_graph_label(&capture.body),
            subtitle: Some(capture.body.clone()),
            status: Some(capture.status.clone()),
            url: Some(format!("/captures?selected={}", capture.id)),
            updated_at: Some(capture.updated_at.clone()),
            hidden_by_default,
            weight: if capture.status == "inbox" { 2 } else { 1 },
            context: format!(
                "Capture '{}' is {}. Body: {}",
                capture.kind, capture.status, capture.body
            ),
            properties: json!({
                "id": &capture.id,
                "kind": "capture",
                "capture_kind": &capture.kind,
                "status": &capture.status,
                "body": &capture.body,
                "promoted_deliverable_id": &capture.promoted_deliverable_id,
                "promoted_initiative_id": &capture.promoted_initiative_id,
                "promoted_conversation_id": &capture.promoted_conversation_id,
                "created_at": &capture.created_at,
                "updated_at": &capture.updated_at,
            }),
        });
    }

    let node_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();

    for deliverable in &visible_deliverables {
        let deliverable_node = graph_node_id("deliverable", &deliverable.id);
        for initiative in &deliverable.initiatives {
            push_graph_edge_if_visible(
                &mut edges,
                &node_ids,
                "initiative_deliverable",
                &graph_node_id("initiative", &initiative.id),
                &deliverable_node,
                "contains",
            );
        }

        for stakeholder in &deliverable.stakeholders {
            push_graph_edge_if_visible(
                &mut edges,
                &node_ids,
                "stakeholder_deliverable",
                &graph_node_id("stakeholder", &stakeholder.id),
                &deliverable_node,
                "target",
            );
        }

        if let Some(conversation_id) = &deliverable.conversation_id {
            push_graph_edge_if_visible(
                &mut edges,
                &node_ids,
                "conversation_deliverable",
                &graph_node_id("conversation", conversation_id),
                &deliverable_node,
                "produced",
            );
        }
    }

    for capture in &visible_captures {
        let capture_node = graph_node_id("capture", &capture.id);
        if let Some(deliverable_id) = &capture.promoted_deliverable_id {
            push_graph_edge_if_visible(
                &mut edges,
                &node_ids,
                "capture_deliverable",
                &capture_node,
                &graph_node_id("deliverable", deliverable_id),
                "promoted",
            );
        }
        if let Some(initiative_id) = &capture.promoted_initiative_id {
            push_graph_edge_if_visible(
                &mut edges,
                &node_ids,
                "capture_initiative",
                &capture_node,
                &graph_node_id("initiative", initiative_id),
                "promoted",
            );
        }
        if let Some(conversation_id) = &capture.promoted_conversation_id {
            push_graph_edge_if_visible(
                &mut edges,
                &node_ids,
                "capture_conversation",
                &capture_node,
                &graph_node_id("conversation", conversation_id),
                "promoted",
            );
        }
    }

    Ok(WorkGraph {
        generated_at: now_utc(),
        ai_context: graph_ai_context(
            &visible_deliverables,
            &visible_captures,
            &memories,
            edges.len(),
        ),
        nodes,
        edges,
    })
}

fn push_graph_edge_if_visible(
    edges: &mut Vec<WorkGraphEdge>,
    node_ids: &BTreeSet<String>,
    kind: &str,
    source: &str,
    target: &str,
    label: &str,
) {
    if !node_ids.contains(source) || !node_ids.contains(target) {
        return;
    }

    edges.push(WorkGraphEdge {
        id: format!("{kind}:{source}->{target}"),
        source: source.to_string(),
        target: target.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        properties: json!({
            "kind": kind,
            "label": label,
            "source": source,
            "target": target,
        }),
    });
}

fn graph_ai_context(
    deliverables: &[Deliverable],
    captures: &[Capture],
    memories: &[MemoryRecord],
    edge_count: usize,
) -> String {
    let active = deliverables
        .iter()
        .filter(|deliverable| matches!(deliverable.state.as_str(), "drafting" | "in_review"))
        .count();
    let shipped = deliverables
        .iter()
        .filter(|deliverable| deliverable.state == "shipped")
        .count();
    let focused = deliverables
        .iter()
        .find(|deliverable| deliverable.is_focused)
        .map(|deliverable| deliverable.title.as_str())
        .unwrap_or("None");
    let inbox_captures = captures
        .iter()
        .filter(|capture| capture.status == CaptureStatus::Inbox.as_str())
        .count();

    let mut lines = vec![
        format!(
            "Trace memory graph: {} deliverable(s), {} active, {} shipped, {} capture(s), {} durable memory item(s), {} edge(s).",
            deliverables.len(),
            active,
            shipped,
            captures.len(),
            memories.len(),
            edge_count
        ),
        format!("Current focus: {focused}. Inbox captures: {inbox_captures}."),
    ];

    for memory in memories.iter().take(12) {
        lines.push(format!(
            "- [memory {} {}] {} | confidence {:.2} | {}",
            memory.kind, memory.source, memory.title, memory.confidence, memory.body
        ));
    }

    for deliverable in deliverables.iter().take(12) {
        lines.push(format!(
            "- [{} {}] {} | stakeholders: {} | claim: {}",
            deliverable.deliverable_type,
            deliverable.state,
            deliverable.title,
            if stakeholder_names(deliverable).is_empty() {
                "None".to_string()
            } else {
                stakeholder_names(deliverable)
            },
            deliverable.claim
        ));
    }

    lines.join("\n")
}

fn deliverable_weight(deliverable: &Deliverable) -> i64 {
    let mut weight =
        1 + deliverable.initiatives.len() as i64 + deliverable.stakeholders.len() as i64;
    if deliverable.is_focused {
        weight += 3;
    }
    if deliverable.state == "in_review" {
        weight += 1;
    }
    if deliverable.blocker_reason.is_some() {
        weight += 1;
    }
    weight
}

fn stakeholder_names(deliverable: &Deliverable) -> String {
    if deliverable.stakeholders.is_empty() {
        return deliverable.stakeholder_name.clone().unwrap_or_default();
    }

    deliverable
        .stakeholders
        .iter()
        .map(|stakeholder| stakeholder.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn optional_context_fragment(label: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        String::new()
    } else {
        format!("{label}: {value}")
    }
}

fn optional_date_context(label: &str, value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!(" {label}: {value}."))
        .unwrap_or_default()
}

pub fn empty_as_placeholder(value: &str) -> &str {
    if value.trim().is_empty() {
        "None"
    } else {
        value
    }
}

fn graph_node_id(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

fn graph_edge_id(kind: &str, source: &str, target: &str) -> String {
    format!("{kind}:{source}->{target}")
}

fn truncate_graph_label(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= 80 {
        return trimmed.to_string();
    }

    let prefix = trimmed.chars().take(77).collect::<String>();
    format!("{prefix}...")
}

pub fn truncate_tray_title(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.chars().count() <= 24 {
        return trimmed.to_string();
    }

    let prefix = trimmed.chars().take(21).collect::<String>();
    format!("{prefix}...")
}

pub fn fts_query(query: &str) -> Option<String> {
    let tokens = query
        .split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .map(|token| format!("{token}*"))
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

pub fn clean_ids(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn clean_optional_string(value: &str) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn to_json_string<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}

pub fn has_ascii_whitespace(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_memory;
    use crate::{
        models::{CreateMemoryInput, MemoryKind, RetrieveMemoryInput},
        repo::{
            consolidate_memories, create_capture, create_deliverable_by_name, create_memory,
            promote_capture_to_initiative, retrieve_memories, search_deliverables,
            shipped_at_for_state, update_deliverable_state, CreateDeliverableByNameInput,
        },
    };

    #[tokio::test]
    async fn repo_preserves_initiative_crud() {
        let pool = connect_memory().await.expect("database");
        let created = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "  Knowledge Graph  ".to_string(),
                framing: "  Graph-backed reasoning. ".to_string(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("created");

        assert_eq!(created.title, "Knowledge Graph");

        let updated = update_initiative(
            &pool,
            &created.id,
            UpdateInitiativeInput {
                title: "Knowledge Graph".to_string(),
                framing: "Updated".to_string(),
                status: InitiativeStatus::Paused,
                ..Default::default()
            },
        )
        .await
        .expect("updated");

        assert_eq!(updated.status, "paused");
        delete_initiative(&pool, &created.id)
            .await
            .expect("deleted");
    }

    #[tokio::test]
    async fn name_resolution_is_strict_and_lists_options() {
        let pool = connect_memory().await.expect("database");
        let initiative = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Knowledge Graph".to_string(),
                framing: String::new(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");

        assert_eq!(
            resolve_initiative_title(&pool, "Knowledge Graph")
                .await
                .expect("resolve"),
            initiative.id
        );

        let error = resolve_initiative_title(&pool, "Knowledge Graph Initiative")
            .await
            .expect_err("strict name should fail");
        assert!(error.contains("Valid initiatives: Knowledge Graph"));
    }

    #[tokio::test]
    async fn memory_create_retrieve_consolidate_and_graph_work() {
        let pool = connect_memory().await.expect("database");
        let manual = create_memory(
            &pool,
            CreateMemoryInput {
                kind: MemoryKind::Procedural,
                title: "Architecture output style".to_string(),
                body: "The user prefers direct, production-focused architecture documents with explicit tradeoffs.".to_string(),
                scope: "global".to_string(),
                tags: vec!["architecture".to_string(), "style".to_string()],
                confidence: Some(0.95),
                importance: Some(0.9),
                sensitivity: None,
                pinned: None,
                expires_at: None,
            },
        )
        .await
        .expect("manual memory");

        let retrieved = retrieve_memories(
            &pool,
            RetrieveMemoryInput {
                query: "architecture tradeoffs".to_string(),
                limit: Some(8),
                kinds: Vec::new(),
                source_kind: None,
                source_id: None,
                task_type: None,
                include_pinned: Some(true),
            },
        )
        .await
        .expect("retrieve memory");
        assert!(retrieved
            .memories
            .iter()
            .any(|memory| memory.id == manual.id));
        assert!(retrieved.context.contains("Architecture output style"));

        let initiative = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Memory Platform".to_string(),
                framing: "Make durable work context first-class.".to_string(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");
        create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "Memory architecture".to_string(),
                deliverable_type: DeliverableType::DesignDoc,
                state: DeliverableState::Drafting,
                claim: "Specify a production memory system.".to_string(),
                artifact_url: None,
                conversation_id: None,
                stakeholder_id: None,
                stakeholder_ids: Vec::new(),
                initiative_ids: vec![initiative.id],
            },
        )
        .await
        .expect("deliverable");

        let consolidation = consolidate_memories(&pool).await.expect("consolidated");
        assert!(consolidation.created_count > 0);
        assert!(consolidation
            .memories
            .iter()
            .any(|memory| memory.title.contains("Memory Platform")));

        let graph = get_work_context_graph(&pool, WorkGraphFilters::default())
            .await
            .expect("graph");
        assert!(graph.nodes.iter().any(|node| node.kind == "memory"));
        assert!(graph.ai_context.contains("durable memory"));
    }

    #[tokio::test]
    async fn create_deliverable_by_name_creates_conversation_and_links() {
        let pool = connect_memory().await.expect("database");
        create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Content Quality".to_string(),
                framing: String::new(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");
        create_stakeholder(
            &pool,
            CreateStakeholderInput {
                name: "CEO".to_string(),
                email: String::new(),
                role: String::new(),
                notes: String::new(),
            },
        )
        .await
        .expect("stakeholder");

        let deliverable = create_deliverable_by_name(
            &pool,
            CreateDeliverableByNameInput {
                title: "Quality argument".to_string(),
                deliverable_type: DeliverableType::Analysis,
                claim: "This argues for better review loops.".to_string(),
                initiative_titles: vec!["Content Quality".to_string()],
                stakeholder_name: Some("CEO".to_string()),
                artifact_url: None,
                conversation_url: Some("https://www.claude.ai/chat/abc123".to_string()),
            },
        )
        .await
        .expect("deliverable");

        assert_eq!(deliverable.state, "drafting");
        assert_eq!(
            deliverable.conversation_url.as_deref(),
            Some("https://claude.ai/chat/abc123")
        );
        assert_eq!(deliverable.initiatives.len(), 1);
    }

    #[tokio::test]
    async fn search_filters_and_state_transition_work() {
        let pool = connect_memory().await.expect("database");
        let initiative = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Retention".to_string(),
                framing: String::new(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");

        let deliverable = create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "Retention brief".to_string(),
                deliverable_type: DeliverableType::Deck,
                state: DeliverableState::Drafting,
                claim: "This argues for a retention review.".to_string(),
                artifact_url: None,
                conversation_id: None,
                stakeholder_id: None,
                stakeholder_ids: Vec::new(),
                initiative_ids: vec![initiative.id],
            },
        )
        .await
        .expect("deliverable");

        let results = search_deliverables(&pool, "retention", DeliverableFilters::default(), 10)
            .await
            .expect("search");
        assert_eq!(results.len(), 1);

        let shipped = update_deliverable_state(&pool, &deliverable.id, DeliverableState::Shipped)
            .await
            .expect("shipped");
        assert_eq!(shipped.state, "shipped");
        assert!(shipped.shipped_at.is_some());

        let drafting = update_deliverable_state(&pool, &deliverable.id, DeliverableState::Drafting)
            .await
            .expect("drafting");
        assert_eq!(drafting.state, "drafting");
        assert!(drafting.shipped_at.is_none());
    }

    #[tokio::test]
    async fn priority_filtering_and_reorder_normalize_column_order() {
        let pool = connect_memory().await.expect("database");
        let first = create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "First".to_string(),
                deliverable_type: DeliverableType::Analysis,
                state: DeliverableState::Backlog,
                claim: "First claim".to_string(),
                artifact_url: None,
                conversation_id: None,
                stakeholder_id: None,
                stakeholder_ids: Vec::new(),
                initiative_ids: Vec::new(),
            },
        )
        .await
        .expect("first");
        let second = create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "Second".to_string(),
                deliverable_type: DeliverableType::Analysis,
                state: DeliverableState::Backlog,
                claim: "Second claim".to_string(),
                artifact_url: None,
                conversation_id: None,
                stakeholder_id: None,
                stakeholder_ids: Vec::new(),
                initiative_ids: Vec::new(),
            },
        )
        .await
        .expect("second");
        let third = create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "Third".to_string(),
                deliverable_type: DeliverableType::Analysis,
                state: DeliverableState::Backlog,
                claim: "Third claim".to_string(),
                artifact_url: None,
                conversation_id: None,
                stakeholder_id: None,
                stakeholder_ids: Vec::new(),
                initiative_ids: Vec::new(),
            },
        )
        .await
        .expect("third");

        update_deliverable_metadata(
            &pool,
            &first.id,
            crate::models::UpdateDeliverableMetadataInput {
                deadline: None,
                effort: None,
                impact: None,
                blocker_reason: None,
                priority: Some("p1".to_string()),
            },
        )
        .await
        .expect("priority");
        let priority_results = list_deliverables(
            &pool,
            DeliverableFilters {
                priority: Some("p1".to_string()),
                ..DeliverableFilters::default()
            },
        )
        .await
        .expect("priority filter");
        assert_eq!(priority_results.len(), 1);
        assert_eq!(priority_results[0].id, first.id);

        reorder_deliverable(&pool, &first.id, 0)
            .await
            .expect("first order");
        reorder_deliverable(&pool, &second.id, 1)
            .await
            .expect("second order");
        reorder_deliverable(&pool, &third.id, 2)
            .await
            .expect("third order");

        let reordered = reorder_deliverable_within_state(&pool, &second.id, "up")
            .await
            .expect("reorder");
        assert_eq!(
            reordered.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
            vec![second.id.as_str(), first.id.as_str(), third.id.as_str(),]
        );
        assert_eq!(
            reordered
                .iter()
                .map(|deliverable| deliverable.display_order)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[tokio::test]
    async fn work_intake_suggestions_are_reviewed_before_apply() {
        let pool = connect_memory().await.expect("database");
        let deliverable = create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "Launch brief".to_string(),
                deliverable_type: DeliverableType::Deck,
                state: DeliverableState::Backlog,
                claim: "Prepare a launch brief.".to_string(),
                artifact_url: None,
                conversation_id: None,
                stakeholder_id: None,
                stakeholder_ids: Vec::new(),
                initiative_ids: Vec::new(),
            },
        )
        .await
        .expect("deliverable");

        let suggestion = create_work_intake_suggestion(
            &pool,
            CreateWorkIntakeSuggestionInput {
                source_kind: "test".to_string(),
                source_id: Some("source-1".to_string()),
                source_title: "Test source".to_string(),
                source_route: None,
                item_kind: "task".to_string(),
                title: "Draft launch narrative".to_string(),
                body: "Needed before review.".to_string(),
                target_deliverable_id: Some(deliverable.id.clone()),
                target_initiative_id: None,
                due_date: Some("2026-05-15".to_string()),
                suggested_type: None,
                confidence: Some(0.8),
                rationale: "Detected action item.".to_string(),
                payload: serde_json::json!({ "source": "unit" }),
            },
        )
        .await
        .expect("suggestion");

        let pending = list_work_intake_suggestions(
            &pool,
            WorkIntakeFilters {
                status: Some("pending".to_string()),
                ..WorkIntakeFilters::default()
            },
        )
        .await
        .expect("pending");
        assert_eq!(pending.len(), 1);

        let applied = approve_work_intake_suggestion(
            &pool,
            crate::models::ApproveWorkIntakeInput {
                id: suggestion.id.clone(),
                target_deliverable_id: None,
                target_initiative_id: None,
                item_kind_override: None,
                title_override: None,
                body_override: None,
                due_date_override: None,
            },
        )
        .await
        .expect("approve");
        assert_eq!(applied.entity_kind, "task");

        let tasks = list_deliverable_tasks(&pool, &deliverable.id)
            .await
            .expect("tasks");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].due_date.as_deref(), Some("2026-05-15"));
        assert_eq!(
            get_work_intake_suggestion(&pool, &suggestion.id)
                .await
                .expect("resolved")
                .status,
            "approved"
        );
    }

    #[tokio::test]
    async fn uploaded_minutes_actions_are_pending_until_approved() {
        let pool = connect_memory().await.expect("database");
        let initiative = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Retention".to_string(),
                framing: String::new(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");
        let deliverable = create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "Retention brief".to_string(),
                deliverable_type: DeliverableType::Deck,
                state: DeliverableState::Drafting,
                claim: "This argues for a retention review.".to_string(),
                artifact_url: None,
                conversation_id: None,
                stakeholder_id: None,
                stakeholder_ids: Vec::new(),
                initiative_ids: vec![initiative.id],
            },
        )
        .await
        .expect("deliverable");
        let meeting = create_meeting(
            &pool,
            CreateMeetingInput {
                title: "Uploaded notes".to_string(),
                date: "2026-05-07".to_string(),
                stakeholder_ids: Vec::new(),
            },
        )
        .await
        .expect("meeting");

        let saved = save_minutes_summary(
            &pool,
            &meeting.id,
            &crate::models::MinutesProcessingResult {
                meeting_title: Some("Retention sync".to_string()),
                meeting_date: Some("2026-05-07".to_string()),
                summary: "Discussed retention brief next steps.".to_string(),
                actions: vec![
                    crate::models::MinutesAction {
                        kind: "deliverable_note".to_string(),
                        target_kind: Some("deliverable".to_string()),
                        target_id: Some(deliverable.id.clone()),
                        target: Some(deliverable.title.clone()),
                        detail: "Add retention appendix.".to_string(),
                        title: None,
                        due_date: None,
                        state: None,
                        deadline: None,
                        blocker_reason: None,
                    },
                    crate::models::MinutesAction {
                        kind: "task_created".to_string(),
                        target_kind: Some("deliverable".to_string()),
                        target_id: Some(deliverable.id.clone()),
                        target: Some(deliverable.title.clone()),
                        detail: "Draft appendix".to_string(),
                        title: Some("Draft appendix".to_string()),
                        due_date: Some("2026-05-15".to_string()),
                        state: None,
                        deadline: None,
                        blocker_reason: None,
                    },
                ],
                flagged: Vec::new(),
            },
        )
        .await
        .expect("save minutes");

        assert_eq!(saved.actions.len(), 2);
        assert!(saved.actions.iter().all(|action| !action.applied));
        assert!(saved.actions.iter().all(|action| action.payload.is_some()));
        let notes_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM deliverable_notes WHERE deliverable_id = ?")
                .bind(&deliverable.id)
                .fetch_one(&pool)
                .await
                .expect("note count");
        assert_eq!(notes_before, 0);

        for action in &saved.actions {
            apply_meeting_action(
                &pool,
                ApplyMeetingActionInput {
                    action_id: action.id.clone(),
                    target_id: None,
                },
            )
            .await
            .expect("approve action");
        }

        let notes_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM deliverable_notes WHERE deliverable_id = ?")
                .bind(&deliverable.id)
                .fetch_one(&pool)
                .await
                .expect("note count");
        let tasks_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM deliverable_tasks WHERE deliverable_id = ?")
                .bind(&deliverable.id)
                .fetch_one(&pool)
                .await
                .expect("task count");
        assert_eq!(notes_after, 1);
        assert_eq!(tasks_after, 1);
    }

    #[tokio::test]
    async fn initiative_state_and_briefing_are_generated() {
        let pool = connect_memory().await.expect("database");
        let initiative = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "GTM".to_string(),
                framing: "Market work".to_string(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");
        let stakeholder = create_stakeholder(
            &pool,
            CreateStakeholderInput {
                name: "Mentor".to_string(),
                email: String::new(),
                role: String::new(),
                notes: String::new(),
            },
        )
        .await
        .expect("stakeholder");
        create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "GTM note".to_string(),
                deliverable_type: DeliverableType::Analysis,
                state: DeliverableState::InReview,
                claim: "This argues for focused institutional positioning.".to_string(),
                artifact_url: None,
                conversation_id: None,
                stakeholder_id: Some(stakeholder.id),
                stakeholder_ids: Vec::new(),
                initiative_ids: vec![initiative.id],
            },
        )
        .await
        .expect("deliverable");

        let state = get_initiative_state_by_title(&pool, "GTM")
            .await
            .expect("initiative state");
        assert_eq!(state.in_review_count, 1);

        let briefing = deterministic_stakeholder_briefing(&pool, "Mentor", 30)
            .await
            .expect("briefing");
        assert!(
            briefing
                .sections
                .in_flight
                .iter()
                .any(|i| i.text.contains("GTM note"))
                || briefing
                    .sections
                    .recent_wins
                    .iter()
                    .any(|i| i.text.contains("GTM note"))
        );
    }

    #[tokio::test]
    async fn conversation_ingest_commits_conversation_and_accepted_deliverables() {
        let pool = connect_memory().await.expect("database");
        let initiative = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Trace Quality".to_string(),
                framing: String::new(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");

        let result = commit_conversation_ingest(
            &pool,
            CommitConversationIngestInput {
                chat_url: Some("https://www.claude.ai/chat/backfill123".to_string()),
                conversation: ExtractedConversation {
                    title: "Backfill plan".to_string(),
                    summary: "A conversation about the Trace backfill workflow.".to_string(),
                    occurred_at: None,
                },
                deliverables: vec![
                    CommitExtractedDeliverableInput {
                        accepted: true,
                        title: "Backfill queue".to_string(),
                        deliverable_type: DeliverableType::DesignDoc,
                        claim: "This defines the review queue before writes.".to_string(),
                        artifact_url: None,
                        stakeholder_id: None,
                        stakeholder_ids: Vec::new(),
                        initiative_ids: vec![initiative.id],
                    },
                    CommitExtractedDeliverableInput {
                        accepted: false,
                        title: "Ignored item".to_string(),
                        deliverable_type: DeliverableType::Analysis,
                        claim: "This should not be written.".to_string(),
                        artifact_url: None,
                        stakeholder_id: None,
                        stakeholder_ids: Vec::new(),
                        initiative_ids: Vec::new(),
                    },
                ],
            },
        )
        .await
        .expect("commit");

        assert_eq!(
            result.conversation.chat_url,
            "https://claude.ai/chat/backfill123"
        );
        assert_eq!(result.deliverables.len(), 1);
        assert_eq!(result.deliverables[0].state, "drafting");
        assert_eq!(
            result.deliverables[0].conversation_id.as_deref(),
            Some(result.conversation.id.as_str())
        );
    }

    #[tokio::test]
    async fn conversation_ingest_rejects_invalid_candidate_without_writes() {
        let pool = connect_memory().await.expect("database");
        let error = commit_conversation_ingest(
            &pool,
            CommitConversationIngestInput {
                chat_url: None,
                conversation: ExtractedConversation {
                    title: "Broken".to_string(),
                    summary: "Missing initiative".to_string(),
                    occurred_at: None,
                },
                deliverables: vec![CommitExtractedDeliverableInput {
                    accepted: true,
                    title: "No initiatives".to_string(),
                    deliverable_type: DeliverableType::Analysis,
                    claim: "This cannot be committed.".to_string(),
                    artifact_url: None,
                    stakeholder_id: None,
                    stakeholder_ids: Vec::new(),
                    initiative_ids: Vec::new(),
                }],
            },
        )
        .await
        .expect_err("invalid candidate");

        assert!(error.contains("at least one initiative"));
        let conversation_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversations")
            .fetch_one(&pool)
            .await
            .expect("conversation count");
        let deliverable_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deliverables")
            .fetch_one(&pool)
            .await
            .expect("deliverable count");
        assert_eq!(conversation_count, 0);
        assert_eq!(deliverable_count, 0);
    }

    #[tokio::test]
    async fn claude_capture_promotion_links_conversation() {
        let pool = connect_memory().await.expect("database");
        let initiative = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Ambient Work".to_string(),
                framing: String::new(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");
        let capture = create_capture(
            &pool,
            CreateCaptureInput {
                kind: CaptureKind::ClaudeLink,
                body: "https://claude.ai/chat/capture123".to_string(),
            },
        )
        .await
        .expect("capture");

        let result = promote_claude_capture_to_ingest(
            &pool,
            &capture.id,
            CommitConversationIngestInput {
                chat_url: None,
                conversation: ExtractedConversation {
                    title: "Capture ingest".to_string(),
                    summary: "A captured Claude link becomes a conversation.".to_string(),
                    occurred_at: None,
                },
                deliverables: vec![CommitExtractedDeliverableInput {
                    accepted: true,
                    title: "Capture candidate".to_string(),
                    deliverable_type: DeliverableType::Research,
                    claim: "This preserves provenance from the capture inbox.".to_string(),
                    artifact_url: None,
                    stakeholder_id: None,
                    stakeholder_ids: Vec::new(),
                    initiative_ids: vec![initiative.id],
                }],
            },
        )
        .await
        .expect("promoted");

        let promoted = get_capture(&pool, &capture.id).await.expect("capture");
        assert_eq!(promoted.status, "promoted");
        assert_eq!(
            promoted.promoted_conversation_id.as_deref(),
            Some(result.conversation.id.as_str())
        );
    }

    #[tokio::test]
    async fn thought_capture_promotes_to_initiative() {
        let pool = connect_memory().await.expect("database");
        let capture = create_capture(
            &pool,
            CreateCaptureInput {
                kind: CaptureKind::Thought,
                body: "Retention framing should become an initiative.".to_string(),
            },
        )
        .await
        .expect("capture");

        let initiative = promote_capture_to_initiative(
            &pool,
            &capture.id,
            CreateInitiativeInput {
                title: "Retention Framing".to_string(),
                framing: capture.body.clone(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");

        let promoted = get_capture(&pool, &capture.id).await.expect("capture");
        assert_eq!(promoted.status, "promoted");
        assert_eq!(
            promoted.promoted_initiative_id.as_deref(),
            Some(initiative.id.as_str())
        );
        assert_eq!(
            promoted.promoted_initiative_title.as_deref(),
            Some("Retention Framing")
        );
    }

    #[tokio::test]
    async fn work_context_graph_links_relational_entities() {
        let pool = connect_memory().await.expect("database");
        let initiative = create_initiative(
            &pool,
            CreateInitiativeInput {
                title: "Context Map".to_string(),
                framing: String::new(),
                status: InitiativeStatus::Live,
                ..Default::default()
            },
        )
        .await
        .expect("initiative");
        let stakeholder = create_stakeholder(
            &pool,
            CreateStakeholderInput {
                name: "CEO".to_string(),
                email: String::new(),
                role: String::new(),
                notes: String::new(),
            },
        )
        .await
        .expect("stakeholder");
        let conversation = create_conversation(
            &pool,
            CreateConversationInput {
                chat_url: "https://claude.ai/chat/graph123".to_string(),
                title: Some("Graph plan".to_string()),
                summary: Some("How context should connect.".to_string()),
                occurred_at: None,
            },
        )
        .await
        .expect("conversation");
        let deliverable = create_deliverable(
            &pool,
            CreateDeliverableInput {
                title: "Graph view".to_string(),
                deliverable_type: DeliverableType::Prototype,
                state: DeliverableState::Drafting,
                claim: "This makes work context inspectable.".to_string(),
                artifact_url: None,
                conversation_id: Some(conversation.id.clone()),
                stakeholder_id: Some(stakeholder.id.clone()),
                stakeholder_ids: Vec::new(),
                initiative_ids: vec![initiative.id.clone()],
            },
        )
        .await
        .expect("deliverable");

        let graph = get_work_context_graph(&pool, WorkGraphFilters::default())
            .await
            .expect("graph");

        assert!(graph
            .nodes
            .iter()
            .any(|node| node.id == graph_node_id("deliverable", &deliverable.id)));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == "initiative_deliverable"
                && edge.source == graph_node_id("initiative", &initiative.id)
                && edge.target == graph_node_id("deliverable", &deliverable.id)
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == "stakeholder_deliverable"
                && edge.source == graph_node_id("stakeholder", &stakeholder.id)
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == "conversation_deliverable"
                && edge.source == graph_node_id("conversation", &conversation.id)
        }));
    }

    #[test]
    fn capture_and_url_validation_match_stage_rules() {
        assert_eq!(
            validate_capture_input(CaptureKind::Thought, "  usable note  ".to_string())
                .expect("valid thought"),
            "usable note"
        );
        assert!(validate_capture_input(CaptureKind::Thought, " ".to_string()).is_err());
        assert_eq!(
            normalize_claude_link("https://www.claude.ai/chat/abc123").expect("claude link"),
            "https://claude.ai/chat/abc123"
        );
        assert!(validate_capture_input(
            CaptureKind::ClaudeLink,
            "https://example.com/chat/abc123".to_string(),
        )
        .is_err());
        assert!(validate_capture_input(
            CaptureKind::ArtifactLink,
            "https://example.com/doc".to_string(),
        )
        .is_ok());
        assert!(validate_capture_input(CaptureKind::ArtifactLink, "notaurl".to_string()).is_err());
    }

    #[test]
    fn fts_query_uses_prefix_matching() {
        assert_eq!(
            fts_query("content quality!"),
            Some("content* quality*".to_string())
        );
        assert_eq!(fts_query(" !!! "), None);
    }

    #[test]
    fn shipped_state_sets_and_clears_shipped_at() {
        assert_eq!(
            shipped_at_for_state(DeliverableState::Shipped, None, "now"),
            Some("now".to_string())
        );
        assert_eq!(
            shipped_at_for_state(DeliverableState::Shipped, Some("then".to_string()), "now"),
            Some("then".to_string())
        );
        assert_eq!(
            shipped_at_for_state(DeliverableState::Drafting, Some("then".to_string()), "now"),
            None
        );
    }

    #[test]
    fn tray_title_truncates_to_twenty_four_chars() {
        assert_eq!(truncate_tray_title("Short title"), "Short title");
        assert_eq!(
            truncate_tray_title("This deliverable title is very long"),
            "This deliverable titl..."
        );
    }
}

// ── Deliverable tasks ────────────────────────────────────────────────────────

pub async fn list_deliverable_tasks(
    pool: &SqlitePool,
    deliverable_id: &str,
) -> Result<Vec<crate::models::DeliverableTask>, String> {
    sqlx::query_as::<_, crate::models::DeliverableTask>(
        r#"
        SELECT id, deliverable_id, title, status, due_date, notes, url, display_order, created_at, updated_at
        FROM deliverable_tasks
        WHERE deliverable_id = ?
        ORDER BY display_order ASC, created_at ASC
        "#,
    )
    .bind(deliverable_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn create_deliverable_task(
    pool: &SqlitePool,
    input: crate::models::CreateTaskInput,
) -> Result<crate::models::DeliverableTask, String> {
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err("task title is required".to_string());
    }

    let id = Ulid::new().to_string();
    let now = now_utc();
    let due_date = input.due_date.and_then(|d| {
        let d = d.trim().to_string();
        if d.is_empty() {
            None
        } else {
            Some(d)
        }
    });

    let max_order: Option<i64> = sqlx::query_scalar(
        "SELECT MAX(display_order) FROM deliverable_tasks WHERE deliverable_id = ?",
    )
    .bind(&input.deliverable_id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    let display_order = max_order.unwrap_or(-1) + 1;

    let notes = input.notes.and_then(|n| {
        let n = n.trim().to_string();
        if n.is_empty() {
            None
        } else {
            Some(n)
        }
    });
    let url = input.url.and_then(|u| {
        let u = u.trim().to_string();
        if u.is_empty() {
            None
        } else {
            Some(u)
        }
    });

    sqlx::query(
        r#"
        INSERT INTO deliverable_tasks (id, deliverable_id, title, status, due_date, notes, url, display_order, created_at, updated_at)
        VALUES (?, ?, ?, 'todo', ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.deliverable_id)
    .bind(&title)
    .bind(&due_date)
    .bind(&notes)
    .bind(&url)
    .bind(display_order)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_deliverable_task(pool, &id).await
}

pub async fn get_deliverable_task(
    pool: &SqlitePool,
    id: &str,
) -> Result<crate::models::DeliverableTask, String> {
    sqlx::query_as::<_, crate::models::DeliverableTask>(
        r#"
        SELECT id, deliverable_id, title, status, due_date, notes, url, display_order, created_at, updated_at
        FROM deliverable_tasks
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "task not found".to_string())
}

pub async fn update_deliverable_task(
    pool: &SqlitePool,
    id: &str,
    input: crate::models::UpdateTaskInput,
) -> Result<crate::models::DeliverableTask, String> {
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err("task title is required".to_string());
    }

    let due_date = input.due_date.and_then(|d| {
        let d = d.trim().to_string();
        if d.is_empty() {
            None
        } else {
            Some(d)
        }
    });

    let notes = input.notes.and_then(|n| {
        let n = n.trim().to_string();
        if n.is_empty() {
            None
        } else {
            Some(n)
        }
    });
    let url = input.url.and_then(|u| {
        let u = u.trim().to_string();
        if u.is_empty() {
            None
        } else {
            Some(u)
        }
    });

    let now = now_utc();
    let result = sqlx::query(
        r#"
        UPDATE deliverable_tasks
        SET title = ?, status = ?, due_date = ?, notes = ?, url = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&title)
    .bind(input.status.as_str())
    .bind(&due_date)
    .bind(&notes)
    .bind(&url)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("task not found".to_string());
    }

    get_deliverable_task(pool, id).await
}

pub async fn delete_deliverable_task(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM deliverable_tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("task not found".to_string());
    }

    Ok(())
}

pub async fn apply_generated_deliverable_tasks(
    pool: &SqlitePool,
    input: ApplyGeneratedTasksInput,
) -> Result<Vec<crate::models::DeliverableTask>, String> {
    if input.tasks.is_empty() {
        return Err("no generated tasks to apply".to_string());
    }

    let deliverable = get_deliverable(pool, &input.deliverable_id).await?;
    let mut created = Vec::new();
    for task in input.tasks {
        let title = task.title.trim();
        if title.is_empty() {
            continue;
        }
        created.push(
            create_deliverable_task(
                pool,
                crate::models::CreateTaskInput {
                    deliverable_id: input.deliverable_id.clone(),
                    title: title.to_string(),
                    due_date: task
                        .due_date
                        .and_then(|date| clean_optional_string(date.as_str())),
                    notes: None,
                    url: None,
                },
            )
            .await?,
        );
    }

    if let Some(deadline) = input
        .suggested_deliverable_deadline
        .and_then(|date| clean_optional_string(date.as_str()))
    {
        update_deliverable_metadata(
            pool,
            &input.deliverable_id,
            crate::models::UpdateDeliverableMetadataInput {
                deadline: Some(deadline),
                effort: deliverable.effort,
                impact: deliverable.impact,
                blocker_reason: deliverable.blocker_reason,
                priority: deliverable.priority,
            },
        )
        .await?;
    }

    Ok(created)
}

pub async fn list_work_intake_suggestions(
    pool: &SqlitePool,
    filters: WorkIntakeFilters,
) -> Result<Vec<WorkIntakeSuggestion>, String> {
    let status = filters.status.or_else(|| Some("pending".to_string()));
    let limit = filters.limit.unwrap_or(50).clamp(1, 200);
    sqlx::query_as::<_, WorkIntakeSuggestion>(
        r#"
        SELECT id, source_kind, source_id, source_title, source_route, item_kind, title, body,
               target_deliverable_id, target_initiative_id, due_date, suggested_type,
               confidence, rationale, status, payload, created_at, updated_at, applied_at
        FROM work_intake_suggestions
        WHERE (? IS NULL OR status = ?)
          AND (? IS NULL OR source_kind = ?)
          AND (? IS NULL OR source_id = ?)
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(status.clone())
    .bind(status)
    .bind(filters.source_kind.clone())
    .bind(filters.source_kind)
    .bind(filters.source_id.clone())
    .bind(filters.source_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

#[derive(Debug)]
pub struct CreateWorkIntakeSuggestionInput {
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_title: String,
    pub source_route: Option<String>,
    pub item_kind: String,
    pub title: String,
    pub body: String,
    pub target_deliverable_id: Option<String>,
    pub target_initiative_id: Option<String>,
    pub due_date: Option<String>,
    pub suggested_type: Option<String>,
    pub confidence: Option<f64>,
    pub rationale: String,
    pub payload: serde_json::Value,
}

pub async fn create_work_intake_suggestion(
    pool: &SqlitePool,
    input: CreateWorkIntakeSuggestionInput,
) -> Result<WorkIntakeSuggestion, String> {
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err("work intake suggestion title is required".to_string());
    }
    let item_kind = match input.item_kind.as_str() {
        "task" | "deliverable" | "initiative" => input.item_kind,
        other => return Err(format!("unsupported work intake kind: {other}")),
    };
    let now = now_utc();
    if let Some(existing_id) = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM work_intake_suggestions
        WHERE source_kind = ?
          AND COALESCE(source_id, '') = COALESCE(?, '')
          AND item_kind = ?
          AND title = ?
          AND status = 'pending'
        LIMIT 1
        "#,
    )
    .bind(&input.source_kind)
    .bind(&input.source_id)
    .bind(&item_kind)
    .bind(&title)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    {
        return get_work_intake_suggestion(pool, &existing_id).await;
    }

    let id = Ulid::new().to_string();
    sqlx::query(
        r#"
        INSERT INTO work_intake_suggestions (
          id, source_kind, source_id, source_title, source_route, item_kind, title, body,
          target_deliverable_id, target_initiative_id, due_date, suggested_type,
          confidence, rationale, status, payload, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.source_kind)
    .bind(&input.source_id)
    .bind(input.source_title.trim())
    .bind(&input.source_route)
    .bind(&item_kind)
    .bind(&title)
    .bind(input.body.trim())
    .bind(&input.target_deliverable_id)
    .bind(&input.target_initiative_id)
    .bind(input.due_date.and_then(|date| clean_optional_string(&date)))
    .bind(
        input
            .suggested_type
            .and_then(|value| clean_optional_string(&value)),
    )
    .bind(input.confidence)
    .bind(input.rationale.trim())
    .bind(to_json_string(&input.payload))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_work_intake_suggestion(pool, &id).await
}

pub async fn get_work_intake_suggestion(
    pool: &SqlitePool,
    id: &str,
) -> Result<WorkIntakeSuggestion, String> {
    sqlx::query_as::<_, WorkIntakeSuggestion>(
        r#"
        SELECT id, source_kind, source_id, source_title, source_route, item_kind, title, body,
               target_deliverable_id, target_initiative_id, due_date, suggested_type,
               confidence, rationale, status, payload, created_at, updated_at, applied_at
        FROM work_intake_suggestions
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "work intake suggestion not found".to_string())
}

pub async fn record_work_intake_from_gmail_ai(
    pool: &SqlitePool,
    thread_id: &str,
    result: &crate::gmail::GmailAiResult,
) -> Result<Vec<WorkIntakeSuggestion>, String> {
    let detail = crate::gmail::get_local_thread(pool, thread_id).await?;
    let source_title = if detail.thread.subject.trim().is_empty() {
        "Email thread".to_string()
    } else {
        detail.thread.subject.clone()
    };
    let source_route = Some(format!("/email?thread={thread_id}"));
    let target_deliverable_id = detail
        .thread
        .linked_deliverables
        .first()
        .map(|deliverable| deliverable.id.clone());
    let target_initiative_id = detail
        .thread
        .linked_initiatives
        .first()
        .map(|initiative| initiative.id.clone());

    // Dismiss stale pending suggestions for this thread so re-runs give a clean set
    // instead of accumulating over multiple AI calls.
    let now = now_utc();
    sqlx::query(
        r#"UPDATE work_intake_suggestions
           SET status = 'dismissed', updated_at = ?
           WHERE source_kind = 'gmail'
             AND source_id = ?
             AND status = 'pending'"#,
    )
    .bind(&now)
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    let mut suggestions = Vec::new();
    for candidate in result.tasks.iter().chain(result.deadlines.iter()) {
        suggestions.push(
            create_work_intake_suggestion(
                pool,
                CreateWorkIntakeSuggestionInput {
                    source_kind: "gmail".to_string(),
                    source_id: Some(thread_id.to_string()),
                    source_title: source_title.clone(),
                    source_route: source_route.clone(),
                    item_kind: "task".to_string(),
                    title: candidate.title.clone(),
                    body: candidate.body.clone(),
                    target_deliverable_id: target_deliverable_id.clone(),
                    target_initiative_id: target_initiative_id.clone(),
                    due_date: candidate.due_date.clone(),
                    suggested_type: None,
                    confidence: candidate.confidence,
                    rationale: candidate.kind.clone(),
                    payload: serde_json::json!(candidate),
                },
            )
            .await?,
        );
    }
    for candidate in &result.deliverables {
        // Skip if a deliverable with this title already exists — avoids re-suggesting
        // items the user already approved in a prior analysis run.
        let existing_deliverable_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM deliverables WHERE LOWER(title) = LOWER(?) LIMIT 1")
                .bind(candidate.title.trim())
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?;
        if existing_deliverable_id.is_some() {
            continue;
        }

        suggestions.push(
            create_work_intake_suggestion(
                pool,
                CreateWorkIntakeSuggestionInput {
                    source_kind: "gmail".to_string(),
                    source_id: Some(thread_id.to_string()),
                    source_title: source_title.clone(),
                    source_route: source_route.clone(),
                    item_kind: "deliverable".to_string(),
                    title: candidate.title.clone(),
                    body: candidate.body.clone(),
                    target_deliverable_id: None,
                    target_initiative_id: target_initiative_id.clone(),
                    due_date: candidate.due_date.clone(),
                    suggested_type: Some(if candidate.kind.trim().is_empty() {
                        "email".to_string()
                    } else {
                        candidate.kind.clone()
                    }),
                    confidence: candidate.confidence,
                    rationale: "Suggested from email analysis.".to_string(),
                    payload: serde_json::json!(candidate),
                },
            )
            .await?,
        );
    }
    for candidate in &result.initiatives {
        // Skip if an initiative with this title already exists.
        let existing_initiative_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM initiatives WHERE LOWER(title) = LOWER(?) LIMIT 1")
                .bind(candidate.title.trim())
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?;
        if existing_initiative_id.is_some() {
            continue;
        }

        suggestions.push(
            create_work_intake_suggestion(
                pool,
                CreateWorkIntakeSuggestionInput {
                    source_kind: "gmail".to_string(),
                    source_id: Some(thread_id.to_string()),
                    source_title: source_title.clone(),
                    source_route: source_route.clone(),
                    item_kind: "initiative".to_string(),
                    title: candidate.title.clone(),
                    body: candidate.body.clone(),
                    target_deliverable_id: None,
                    target_initiative_id: None,
                    due_date: None,
                    suggested_type: None,
                    confidence: candidate.confidence,
                    rationale: "Suggested from email analysis.".to_string(),
                    payload: serde_json::json!(candidate),
                },
            )
            .await?,
        );
    }
    Ok(suggestions)
}

pub async fn generate_workspace_work_intake(
    pool: &SqlitePool,
) -> Result<Vec<WorkIntakeSuggestion>, String> {
    import_meeting_actions_to_work_intake(pool).await?;
    import_captures_to_work_intake(pool).await?;
    import_gmail_ai_suggestions_to_work_intake(pool).await?;
    list_work_intake_suggestions(
        pool,
        WorkIntakeFilters {
            status: Some("pending".to_string()),
            limit: Some(100),
            ..WorkIntakeFilters::default()
        },
    )
    .await
}

async fn import_meeting_actions_to_work_intake(pool: &SqlitePool) -> Result<(), String> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        Option<String>,
    )> = sqlx::query_as(
        r#"
            SELECT a.id, a.meeting_id, m.title, a.kind, a.target_id, a.body, a.payload
            FROM meeting_actions a
            INNER JOIN meetings m ON m.id = a.meeting_id
            WHERE a.applied = 0
              AND a.kind IN ('task_created', 'deadline_set', 'flagged')
            ORDER BY a.created_at DESC
            LIMIT 80
            "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (action_id, meeting_id, meeting_title, kind, target_id, body, payload) in rows {
        let payload_value = payload
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        let title = payload_value["title"]
            .as_str()
            .or_else(|| payload_value["detail"].as_str())
            .unwrap_or(body.as_str())
            .to_string();
        let item_kind = if kind == "task_created" || kind == "deadline_set" {
            "task"
        } else {
            "deliverable"
        };
        let _ = create_work_intake_suggestion(
            pool,
            CreateWorkIntakeSuggestionInput {
                source_kind: "meeting".to_string(),
                source_id: Some(action_id.clone()),
                source_title: meeting_title,
                source_route: Some(format!("/meetings/{meeting_id}")),
                item_kind: item_kind.to_string(),
                title,
                body,
                target_deliverable_id: target_id,
                target_initiative_id: None,
                due_date: payload_value["due_date"]
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| payload_value["deadline"].as_str().map(str::to_string)),
                suggested_type: payload_value["suggested_type"].as_str().map(str::to_string),
                confidence: Some(0.9),
                rationale: format!("Pending meeting action: {kind}"),
                payload: payload_value,
            },
        )
        .await;
    }
    Ok(())
}

async fn import_captures_to_work_intake(pool: &SqlitePool) -> Result<(), String> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, kind, body FROM captures WHERE status = 'inbox' ORDER BY created_at DESC LIMIT 40",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (capture_id, kind, body) in rows {
        let title = body
            .lines()
            .next()
            .unwrap_or("Captured work")
            .trim()
            .trim_start_matches("[CANDIDATE]")
            .trim()
            .to_string();
        let _ = create_work_intake_suggestion(
            pool,
            CreateWorkIntakeSuggestionInput {
                source_kind: "capture".to_string(),
                source_id: Some(capture_id),
                source_title: "Capture inbox".to_string(),
                source_route: Some("/captures".to_string()),
                item_kind: "deliverable".to_string(),
                title: if title.is_empty() {
                    "Captured work".to_string()
                } else {
                    title.chars().take(120).collect()
                },
                body,
                target_deliverable_id: None,
                target_initiative_id: None,
                due_date: None,
                suggested_type: Some(
                    if kind == "artifact_link" {
                        "other"
                    } else {
                        "analysis"
                    }
                    .to_string(),
                ),
                confidence: Some(0.55),
                rationale: "Capture inbox item may need promotion.".to_string(),
                payload: serde_json::json!({ "capture_kind": kind }),
            },
        )
        .await;
    }
    Ok(())
}

async fn import_gmail_ai_suggestions_to_work_intake(pool: &SqlitePool) -> Result<(), String> {
    let rows: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT s.id, s.thread_id, t.subject, s.kind, s.title, s.payload
        FROM gmail_ai_suggestions s
        INNER JOIN gmail_threads t ON t.thread_id = s.thread_id
        WHERE s.status = 'pending'
          AND s.kind IN ('task', 'deadline', 'deliverable', 'initiative')
        ORDER BY s.created_at DESC
        LIMIT 80
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for (suggestion_id, thread_id, subject, kind, title, payload) in rows {
        let payload_value = serde_json::from_str::<serde_json::Value>(&payload)
            .unwrap_or_else(|_| serde_json::json!({}));
        let item_kind = if kind == "deadline" {
            "task"
        } else {
            kind.as_str()
        };
        let _ = create_work_intake_suggestion(
            pool,
            CreateWorkIntakeSuggestionInput {
                source_kind: "gmail".to_string(),
                source_id: Some(thread_id.clone()),
                source_title: subject,
                source_route: Some(format!("/email?thread={thread_id}")),
                item_kind: item_kind.to_string(),
                title,
                body: payload_value["body"].as_str().unwrap_or("").to_string(),
                target_deliverable_id: None,
                target_initiative_id: None,
                due_date: payload_value["due_date"].as_str().map(str::to_string),
                suggested_type: payload_value["kind"].as_str().map(str::to_string),
                confidence: payload_value["confidence"].as_f64(),
                rationale: format!("Gmail AI suggestion: {kind}"),
                payload: serde_json::json!({
                    "gmail_ai_suggestion_id": suggestion_id,
                    "candidate": payload_value
                }),
            },
        )
        .await;
    }
    Ok(())
}

pub async fn approve_work_intake_suggestion(
    pool: &SqlitePool,
    input: crate::models::ApproveWorkIntakeInput,
) -> Result<WorkIntakeApplyResult, String> {
    let suggestion = get_work_intake_suggestion(pool, &input.id).await?;
    if suggestion.status != "pending" {
        return Err("work intake suggestion is already resolved".to_string());
    }
    let now = now_utc();

    // Caller may override the AI-suggested kind/title/body/due_date before approving.
    let effective_kind = input
        .item_kind_override
        .as_deref()
        .unwrap_or(&suggestion.item_kind);
    let effective_title = input
        .title_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&suggestion.title);
    let effective_due_date = input
        .due_date_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(suggestion.due_date.as_deref());

    let (entity_kind, entity_id, route) = match effective_kind {
        "task" => {
            let deliverable_id = input
                .target_deliverable_id
                .clone()
                .or(suggestion.target_deliverable_id.clone())
                .ok_or_else(|| "Choose a deliverable before approving this task.".to_string())?;
            let task = create_deliverable_task(
                pool,
                crate::models::CreateTaskInput {
                    deliverable_id: deliverable_id.clone(),
                    title: effective_title.to_string(),
                    due_date: effective_due_date.map(str::to_string),
                    notes: None,
                    url: None,
                },
            )
            .await?;
            if suggestion.source_kind == "gmail" {
                if let Some(thread_id) = suggestion.source_id.as_deref() {
                    let _ =
                        crate::gmail::link_thread_to_deliverable(pool, thread_id, &deliverable_id)
                            .await;
                }
            }
            (
                "task".to_string(),
                task.id,
                format!("/deliverables/{deliverable_id}"),
            )
        }
        "deliverable" => {
            let initiative_ids = input
                .target_initiative_id
                .clone()
                .or(suggestion.target_initiative_id.clone())
                .map(|id| vec![id])
                .unwrap_or_default();
            let effective_body = input
                .body_override
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(if suggestion.body.trim().is_empty() {
                    &suggestion.rationale
                } else {
                    &suggestion.body
                });
            let deliverable = create_deliverable(
                pool,
                CreateDeliverableInput {
                    title: effective_title.to_string(),
                    deliverable_type: parse_agentic_deliverable_type(
                        suggestion.suggested_type.as_deref().unwrap_or("other"),
                    ),
                    state: DeliverableState::Backlog,
                    claim: effective_body.to_string(),
                    artifact_url: None,
                    conversation_id: None,
                    stakeholder_id: None,
                    stakeholder_ids: Vec::new(),
                    initiative_ids,
                },
            )
            .await?;
            if suggestion.source_kind == "gmail" {
                if let Some(thread_id) = suggestion.source_id.as_deref() {
                    let _ =
                        crate::gmail::link_thread_to_deliverable(pool, thread_id, &deliverable.id)
                            .await;
                }
            }
            (
                "deliverable".to_string(),
                deliverable.id.clone(),
                format!("/deliverables/{}", deliverable.id),
            )
        }
        "initiative" => {
            let effective_framing = input
                .body_override
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(if suggestion.body.trim().is_empty() {
                    &suggestion.rationale
                } else {
                    &suggestion.body
                });
            let initiative = create_initiative(
                pool,
                CreateInitiativeInput {
                    title: effective_title.to_string(),
                    framing: effective_framing.to_string(),
                    status: InitiativeStatus::Live,
                    ..Default::default()
                },
            )
            .await?;
            if suggestion.source_kind == "gmail" {
                if let Some(thread_id) = suggestion.source_id.as_deref() {
                    let _ =
                        crate::gmail::link_thread_to_initiative(pool, thread_id, &initiative.id)
                            .await;
                }
            }
            (
                "initiative".to_string(),
                initiative.id.clone(),
                format!("/initiatives/{}", initiative.id),
            )
        }
        other => return Err(format!("unsupported work intake kind: {other}")),
    };

    sqlx::query(
        r#"
        UPDATE work_intake_suggestions
        SET status = 'approved',
            target_deliverable_id = COALESCE(?, target_deliverable_id),
            target_initiative_id = COALESCE(?, target_initiative_id),
            updated_at = ?,
            applied_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&input.target_deliverable_id)
    .bind(&input.target_initiative_id)
    .bind(&now)
    .bind(&now)
    .bind(&input.id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    Ok(WorkIntakeApplyResult {
        suggestion: get_work_intake_suggestion(pool, &input.id).await?,
        entity_kind,
        entity_id,
        route,
    })
}

pub async fn dismiss_work_intake_suggestion(
    pool: &SqlitePool,
    id: &str,
) -> Result<WorkIntakeSuggestion, String> {
    let now = now_utc();
    sqlx::query(
        "UPDATE work_intake_suggestions SET status = 'dismissed', updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    get_work_intake_suggestion(pool, id).await
}

// ── Deliverable notes ────────────────────────────────────────────────────────

pub async fn list_deliverable_notes(
    pool: &SqlitePool,
    deliverable_id: &str,
) -> Result<Vec<crate::models::DeliverableNote>, String> {
    sqlx::query_as::<_, crate::models::DeliverableNote>(
        r#"
        SELECT id, deliverable_id, body, created_at
        FROM deliverable_notes
        WHERE deliverable_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(deliverable_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn create_deliverable_note(
    pool: &SqlitePool,
    input: crate::models::CreateNoteInput,
) -> Result<crate::models::DeliverableNote, String> {
    let body = input.body.trim().to_string();
    if body.is_empty() {
        return Err("note body is required".to_string());
    }

    let id = Ulid::new().to_string();
    let now = now_utc();

    sqlx::query(
        r#"
        INSERT INTO deliverable_notes (id, deliverable_id, body, created_at)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&input.deliverable_id)
    .bind(&body)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query_as::<_, crate::models::DeliverableNote>(
        "SELECT id, deliverable_id, body, created_at FROM deliverable_notes WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}

pub async fn delete_deliverable_note(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM deliverable_notes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("note not found".to_string());
    }

    Ok(())
}

// ── Deliverable metadata (deadline, effort, impact, blocker) ─────────────────

pub async fn update_deliverable_metadata(
    pool: &SqlitePool,
    id: &str,
    input: crate::models::UpdateDeliverableMetadataInput,
) -> Result<Deliverable, String> {
    let deadline = input.deadline.and_then(|d| {
        let d = d.trim().to_string();
        if d.is_empty() {
            None
        } else {
            Some(d)
        }
    });
    let blocker_reason = input.blocker_reason.and_then(|r| {
        let r = r.trim().to_string();
        if r.is_empty() {
            None
        } else {
            Some(r)
        }
    });

    let priority = input.priority.and_then(|p| {
        let p = p.trim().to_string();
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    });

    let now = now_utc();
    let result = sqlx::query(
        r#"
        UPDATE deliverables
        SET deadline = ?, effort = ?, impact = ?, blocker_reason = ?, priority = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&deadline)
    .bind(input.effort)
    .bind(input.impact)
    .bind(&blocker_reason)
    .bind(&priority)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("deliverable not found".to_string());
    }

    get_deliverable(pool, id).await
}

// ── Focus (exclusive — only one deliverable focused at a time) ───────────────

pub async fn set_deliverable_focus(
    pool: &SqlitePool,
    id: &str,
    focused: bool,
) -> Result<Deliverable, String> {
    let now = now_utc();
    let mut tx = pool.begin().await.map_err(sql_error)?;

    if focused {
        sqlx::query("UPDATE deliverables SET is_focused = 0, updated_at = ?")
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(sql_error)?;
    }

    let result = sqlx::query("UPDATE deliverables SET is_focused = ?, updated_at = ? WHERE id = ?")
        .bind(focused as i64)
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("deliverable not found".to_string());
    }

    tx.commit().await.map_err(sql_error)?;
    get_deliverable(pool, id).await
}

// ── Week view ────────────────────────────────────────────────────────────────

pub async fn get_week_view(
    pool: &SqlitePool,
    week_start: &str,
    app_support_dir: &std::path::Path,
) -> Result<WeekView, String> {
    #[derive(sqlx::FromRow)]
    struct WeekPlanRow {
        day_index: i64,
        deliverable_id: Option<String>,
    }

    let rows = sqlx::query_as::<_, WeekPlanRow>(
        "SELECT day_index, deliverable_id FROM week_plans WHERE week_start = ? ORDER BY day_index",
    )
    .bind(week_start)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let meeting_config = get_meeting_config(pool).await?;
    let week_tasks = list_week_tasks(pool, week_start).await?;

    // Deliverables whose deadline falls within this week (auto-populated)
    let week_start_date = chrono::NaiveDate::parse_from_str(week_start, "%Y-%m-%d")
        .map_err(|_| "week_start must use YYYY-MM-DD".to_string())?;
    let week_end_date = week_start_date + Duration::days(4);
    let week_end = week_end_date.format("%Y-%m-%d").to_string();

    #[derive(sqlx::FromRow)]
    struct DeadlineRow {
        id: String,
        deadline: String,
    }
    let deadline_rows: Vec<DeadlineRow> = sqlx::query_as(
        "SELECT id, deadline FROM deliverables
         WHERE deadline >= ? AND deadline <= ?
           AND state NOT IN ('shipped', 'killed')",
    )
    .bind(week_start)
    .bind(&week_end)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    // GCal events cached for this week
    let gcal_connected = crate::google_calendar::calendar_connected(app_support_dir);
    let gcal_events_all = if gcal_connected {
        crate::google_calendar::get_cached_events(pool, week_start)
            .await
            .unwrap_or_default()
    } else {
        vec![]
    };

    let mut days: Vec<WeekDay> = Vec::with_capacity(5);
    for idx in 0..5i64 {
        let day_date = week_start_date + Duration::days(idx);
        let day_date_str = day_date.format("%Y-%m-%d").to_string();

        let deliverable_id = rows
            .iter()
            .find(|r| r.day_index == idx)
            .and_then(|r| r.deliverable_id.clone());

        let deliverable = if let Some(id) = deliverable_id {
            get_deliverable(pool, &id).await.ok()
        } else {
            None
        };

        // Deadline deliverables for this specific day (exclude the manually pinned one)
        let pinned_id = deliverable.as_ref().map(|d| d.id.clone());
        let mut deadline_deliverables = Vec::new();
        for row in deadline_rows.iter().filter(|r| r.deadline == day_date_str) {
            if pinned_id.as_deref() == Some(&row.id) {
                continue; // already shown as pinned
            }
            if let Ok(d) = get_deliverable(pool, &row.id).await {
                deadline_deliverables.push(d);
            }
        }

        let tasks = week_tasks
            .iter()
            .filter(|task| task.day_index == idx)
            .cloned()
            .collect();

        let gcal_events = gcal_events_all
            .iter()
            .filter(|e| e.start_date == day_date_str)
            .cloned()
            .collect();

        days.push(WeekDay {
            day_index: idx,
            deliverable,
            deadline_deliverables,
            tasks,
            gcal_events,
        });
    }

    Ok(WeekView {
        week_start: week_start.to_string(),
        days,
        meeting_date: meeting_config.next_meeting_date,
        gcal_connected,
    })
}

pub async fn list_week_tasks(pool: &SqlitePool, week_start: &str) -> Result<Vec<WeekTask>, String> {
    let start = chrono::NaiveDate::parse_from_str(week_start, "%Y-%m-%d")
        .map_err(|_| "week_start must use YYYY-MM-DD".to_string())?;
    let end = start + Duration::days(4);
    let week_start = start.format("%Y-%m-%d").to_string();
    let week_end = end.format("%Y-%m-%d").to_string();

    sqlx::query_as::<_, WeekTask>(
        r#"
        SELECT
          CAST(julianday(t.due_date) - julianday(?) AS INTEGER) AS day_index,
          t.id,
          t.deliverable_id,
          d.title AS deliverable_title,
          d.type AS deliverable_type,
          d.state AS deliverable_state,
          COALESCE(
            (
              SELECT GROUP_CONCAT(s.name, ', ')
              FROM deliverable_stakeholders ds
              INNER JOIN stakeholders s ON s.id = ds.stakeholder_id
              WHERE ds.deliverable_id = d.id
            ),
            s.name
          ) AS stakeholder_name,
          t.title,
          t.status,
          t.due_date
        FROM deliverable_tasks t
        INNER JOIN deliverables d ON d.id = t.deliverable_id
        LEFT JOIN stakeholders s ON s.id = d.stakeholder_id
        WHERE t.due_date BETWEEN ? AND ?
          AND d.state != 'killed'
        ORDER BY t.due_date ASC, t.status ASC, d.title ASC, t.display_order ASC
        "#,
    )
    .bind(&week_start)
    .bind(&week_start)
    .bind(&week_end)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn set_day_deliverable(
    pool: &SqlitePool,
    week_start: &str,
    day_index: i64,
    deliverable_id: Option<&str>,
    app_support_dir: &std::path::Path,
) -> Result<WeekView, String> {
    let now = now_utc();
    sqlx::query(
        r#"
        INSERT INTO week_plans (week_start, day_index, deliverable_id, updated_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(week_start, day_index) DO UPDATE SET deliverable_id = excluded.deliverable_id, updated_at = excluded.updated_at
        "#,
    )
    .bind(week_start)
    .bind(day_index)
    .bind(deliverable_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_week_view(pool, week_start, app_support_dir).await
}

pub async fn list_initiative_notes(
    pool: &SqlitePool,
    initiative_id: &str,
) -> Result<Vec<InitiativeNote>, String> {
    sqlx::query_as::<_, InitiativeNote>(
        "SELECT id, initiative_id, body, created_at FROM initiative_notes WHERE initiative_id = ? ORDER BY created_at DESC",
    )
    .bind(initiative_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn create_initiative_note(
    pool: &SqlitePool,
    initiative_id: &str,
    body: &str,
) -> Result<InitiativeNote, String> {
    let id = Ulid::new().to_string();
    let now = now_utc();
    sqlx::query(
        "INSERT INTO initiative_notes (id, initiative_id, body, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(initiative_id)
    .bind(body)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query_as::<_, InitiativeNote>(
        "SELECT id, initiative_id, body, created_at FROM initiative_notes WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}

pub async fn delete_initiative_note(pool: &SqlitePool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM initiative_notes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

// ── Gantt ──────────────────────────────────────────────────────────────────────

pub async fn get_initiative_gantt(
    pool: &SqlitePool,
    initiative_id: &str,
) -> Result<InitiativeGantt, String> {
    let initiative = sqlx::query_as::<_, (String, String, String, String, String, String, String, String)>(
        "SELECT id, title, framing, status, COALESCE(icon, 'target') AS icon, COALESCE(icon_color, '#6366f1') AS icon_color, created_at, updated_at FROM initiatives WHERE id = ?",
    )
    .bind(initiative_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "Initiative not found".to_string())?;

    let initiative = crate::models::Initiative {
        id: initiative.0,
        title: initiative.1,
        framing: initiative.2,
        status: initiative.3,
        icon: initiative.4,
        icon_color: initiative.5,
        created_at: initiative.6,
        updated_at: initiative.7,
    };

    let sections = sqlx::query_as::<_, InitiativeSection>(
        "SELECT id, initiative_id, title, position, created_at, updated_at FROM initiative_sections WHERE initiative_id = ? ORDER BY position ASC",
    )
    .bind(initiative_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let deliverables = sqlx::query_as::<_, GanttDeliverable>(
        r#"
        SELECT
            d.id,
            d.title,
            d.state,
            d.section_id,
            d.start_date,
            d.deadline,
            COUNT(t.id) AS task_count,
            SUM(CASE WHEN t.status = 'done' THEN 1 ELSE 0 END) AS done_task_count
        FROM deliverables d
        JOIN deliverable_initiatives di ON di.deliverable_id = d.id
        LEFT JOIN deliverable_tasks t ON t.deliverable_id = d.id
        WHERE di.initiative_id = ?
        GROUP BY d.id
        ORDER BY d.start_date ASC, d.created_at ASC
        "#,
    )
    .bind(initiative_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    Ok(InitiativeGantt {
        initiative,
        sections,
        deliverables,
    })
}

pub async fn list_initiative_sections(
    pool: &SqlitePool,
    initiative_id: &str,
) -> Result<Vec<InitiativeSection>, String> {
    sqlx::query_as::<_, InitiativeSection>(
        "SELECT id, initiative_id, title, position, created_at, updated_at FROM initiative_sections WHERE initiative_id = ? ORDER BY position ASC",
    )
    .bind(initiative_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn create_initiative_section(
    pool: &SqlitePool,
    input: CreateSectionInput,
) -> Result<InitiativeSection, String> {
    let id = Ulid::new().to_string();
    let now = now_utc();

    let max_pos: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) FROM initiative_sections WHERE initiative_id = ?",
    )
    .bind(&input.initiative_id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query(
        "INSERT INTO initiative_sections (id, initiative_id, title, position, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.initiative_id)
    .bind(&input.title)
    .bind(max_pos + 1)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query_as::<_, InitiativeSection>(
        "SELECT id, initiative_id, title, position, created_at, updated_at FROM initiative_sections WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}

pub async fn update_initiative_section(
    pool: &SqlitePool,
    id: &str,
    input: UpdateSectionInput,
) -> Result<InitiativeSection, String> {
    let now = now_utc();
    sqlx::query(
        "UPDATE initiative_sections SET title = ?, position = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&input.title)
    .bind(input.position)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query_as::<_, InitiativeSection>(
        "SELECT id, initiative_id, title, position, created_at, updated_at FROM initiative_sections WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}

pub async fn delete_initiative_section(pool: &SqlitePool, id: &str) -> Result<(), String> {
    // Unlink deliverables from this section
    sqlx::query("UPDATE deliverables SET section_id = NULL WHERE section_id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    sqlx::query("DELETE FROM initiative_sections WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn update_deliverable_gantt_dates(
    pool: &SqlitePool,
    id: &str,
    input: UpdateGanttDatesInput,
) -> Result<(), String> {
    let now = now_utc();
    sqlx::query(
        "UPDATE deliverables SET start_date = ?, deadline = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&input.start_date)
    .bind(&input.deadline)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn set_deliverable_section(
    pool: &SqlitePool,
    deliverable_id: &str,
    section_id: Option<&str>,
) -> Result<(), String> {
    let now = now_utc();
    sqlx::query("UPDATE deliverables SET section_id = ?, updated_at = ? WHERE id = ?")
        .bind(section_id)
        .bind(&now)
        .bind(deliverable_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

// ── Agentic search tools ──────────────────────────────────────────────────────
// Each function is a discrete tool Gemini can call during the agentic ask loop.

pub async fn reorder_deliverable_task(
    pool: &SqlitePool,
    id: &str,
    direction: &str,
) -> Result<Vec<crate::models::DeliverableTask>, String> {
    let task = get_deliverable_task(pool, id).await?;
    let tasks = list_deliverable_tasks(pool, &task.deliverable_id).await?;

    let pos = tasks
        .iter()
        .position(|t| t.id == id)
        .ok_or_else(|| "task not found in list".to_string())?;

    let swap_pos = match direction {
        "up" if pos > 0 => pos - 1,
        "down" if pos + 1 < tasks.len() => pos + 1,
        _ => return Ok(tasks),
    };

    let now = now_utc();
    sqlx::query("UPDATE deliverable_tasks SET display_order = ?, updated_at = ? WHERE id = ?")
        .bind(tasks[swap_pos].display_order)
        .bind(&now)
        .bind(&task.id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    sqlx::query("UPDATE deliverable_tasks SET display_order = ?, updated_at = ? WHERE id = ?")
        .bind(tasks[pos].display_order)
        .bind(&now)
        .bind(&tasks[swap_pos].id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    list_deliverable_tasks(pool, &task.deliverable_id).await
}

/// Partial metadata update — only fields present in the call are changed.

// ── Labels ─────────────────────────────────────────────────────────────────────

// ── Within-column reorder ──────────────────────────────────────────────────────

pub async fn reorder_deliverable(
    pool: &SqlitePool,
    id: &str,
    display_order: i64,
) -> Result<Deliverable, String> {
    let now = now_utc();
    let result =
        sqlx::query("UPDATE deliverables SET display_order = ?, updated_at = ? WHERE id = ?")
            .bind(display_order)
            .bind(&now)
            .bind(id)
            .execute(pool)
            .await
            .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("deliverable not found".to_string());
    }
    get_deliverable(pool, id).await
}

pub async fn reorder_deliverable_within_state(
    pool: &SqlitePool,
    id: &str,
    direction: &str,
) -> Result<Vec<Deliverable>, String> {
    let current: Option<(String,)> = sqlx::query_as("SELECT state FROM deliverables WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;
    let Some((state,)) = current else {
        return Err("deliverable not found".to_string());
    };

    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM deliverables
        WHERE state = ?
        ORDER BY COALESCE(display_order, 0) ASC, updated_at DESC, created_at ASC
        "#,
    )
    .bind(&state)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut ids = rows.into_iter().map(|row| row.0).collect::<Vec<_>>();
    let Some(index) = ids.iter().position(|candidate| candidate == id) else {
        return Err("deliverable not found in its state".to_string());
    };
    let next_index = match direction {
        "up" if index > 0 => index - 1,
        "down" if index + 1 < ids.len() => index + 1,
        "up" | "down" => index,
        other => return Err(format!("unknown reorder direction: {other}")),
    };
    if next_index != index {
        ids.swap(index, next_index);
    }

    let now = now_utc();
    let mut tx = pool.begin().await.map_err(sql_error)?;
    for (display_order, deliverable_id) in ids.iter().enumerate() {
        sqlx::query("UPDATE deliverables SET display_order = ?, updated_at = ? WHERE id = ?")
            .bind(display_order as i64)
            .bind(&now)
            .bind(deliverable_id)
            .execute(&mut *tx)
            .await
            .map_err(sql_error)?;
    }
    tx.commit().await.map_err(sql_error)?;

    list_deliverables(
        pool,
        DeliverableFilters {
            state: Some(DeliverableState::from_str(&state)?),
            ..DeliverableFilters::default()
        },
    )
    .await
}

// ── Apply flagged meeting item directly to backlog ─────────────────────────────

pub async fn apply_flagged_to_backlog(
    pool: &SqlitePool,
    input: crate::models::ApplyFlaggedToBacklogInput,
) -> Result<Deliverable, String> {
    let action = sqlx::query_as::<_, MeetingAction>(
        "SELECT id, meeting_id, kind, target_id, target_title, body, payload, applied, created_at FROM meeting_actions WHERE id = ?",
    )
    .bind(&input.action_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| format!("meeting action {} not found", input.action_id))?;

    if action.kind != "flagged" {
        return Err("only flagged actions can be promoted to backlog".to_string());
    }

    let payload = action_payload(&action)?;
    let title = action
        .target_title
        .clone()
        .unwrap_or_else(|| "Candidate deliverable".to_string());
    let claim = payload
        .get("claim")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| action.body.as_str())
        .to_string();
    let suggested_type = payload
        .get("suggested_type")
        .and_then(|v| v.as_str())
        .unwrap_or("other")
        .to_string();

    let deliverable_type = match suggested_type.as_str() {
        "deck" => DeliverableType::Deck,
        "design_doc" => DeliverableType::DesignDoc,
        "prototype" => DeliverableType::Prototype,
        "analysis" => DeliverableType::Analysis,
        "framework" => DeliverableType::Framework,
        "pitch" => DeliverableType::Pitch,
        "research" => DeliverableType::Research,
        "code" => DeliverableType::Code,
        "email" => DeliverableType::Email,
        "meeting_prep" => DeliverableType::MeetingPrep,
        _ => DeliverableType::Other,
    };

    let deliverable = create_deliverable(
        pool,
        CreateDeliverableInput {
            title,
            deliverable_type,
            state: DeliverableState::Backlog,
            claim: if claim.trim().is_empty() {
                "Flagged from meeting".to_string()
            } else {
                claim
            },
            artifact_url: None,
            conversation_id: None,
            stakeholder_id: None,
            stakeholder_ids: Vec::new(),
            initiative_ids: input.initiative_ids,
        },
    )
    .await?;

    // Mark action as applied
    sqlx::query("UPDATE meeting_actions SET applied = 1, target_id = ? WHERE id = ?")
        .bind(&deliverable.id)
        .bind(&input.action_id)
        .execute(pool)
        .await
        .ok();

    Ok(deliverable)
}

// ── Server-side Ask chat persistence ─────────────────────────────────────────

pub async fn list_ask_chats(
    pool: &SqlitePool,
    filters: ListAskChatsFilters,
) -> Result<Vec<AskChatRecord>, String> {
    let limit = filters.limit.unwrap_or(120).clamp(1, 500);
    let rows = if filters.include_archived {
        sqlx::query_as::<_, AskChatRecord>(
            r#"
            SELECT id, title, mode, summary, archived_at, created_at, updated_at
            FROM ask_chats
            ORDER BY updated_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?
    } else {
        sqlx::query_as::<_, AskChatRecord>(
            r#"
            SELECT id, title, mode, summary, archived_at, created_at, updated_at
            FROM ask_chats
            WHERE archived_at IS NULL
            ORDER BY updated_at DESC
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

pub async fn get_ask_chat(pool: &SqlitePool, chat_id: &str) -> Result<AskChatDetail, String> {
    let chat = sqlx::query_as::<_, AskChatRecord>(
        r#"
        SELECT id, title, mode, summary, archived_at, created_at, updated_at
        FROM ask_chats
        WHERE id = ?
        "#,
    )
    .bind(chat_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "chat not found".to_string())?;

    let turn_rows: Vec<AskTurnRow> = sqlx::query_as(
        r#"
        SELECT id, chat_id, parent_id, fork_of, mode, question, answer, reasoning,
               status, error, refs_json, questions_json, steps_json,
               scored_nodes_json, retrieval_query,
               created_at, updated_at
        FROM ask_turns
        WHERE chat_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(chat_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let attachments: Vec<AskTurnAttachmentRecord> = sqlx::query_as(
        r#"
        SELECT a.id, a.turn_id, a.mime_type, a.filename, a.data_b64, a.size_bytes, a.created_at
        FROM ask_turn_attachments a
        JOIN ask_turns t ON t.id = a.turn_id
        WHERE t.chat_id = ?
        ORDER BY a.created_at ASC
        "#,
    )
    .bind(chat_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut by_turn: BTreeMap<String, Vec<AskTurnAttachmentRecord>> = BTreeMap::new();
    for attachment in attachments {
        by_turn
            .entry(attachment.turn_id.clone())
            .or_default()
            .push(attachment);
    }

    let turns = turn_rows
        .into_iter()
        .map(|row| {
            let key = row.id.clone();
            row.into_record(by_turn.remove(&key).unwrap_or_default())
        })
        .collect();

    Ok(AskChatDetail { chat, turns })
}

pub async fn upsert_ask_chat(
    pool: &SqlitePool,
    input: UpsertAskChatInput,
) -> Result<AskChatRecord, String> {
    let id = input
        .id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Ulid::new().to_string());
    let title = input.title.trim();
    let title = if title.is_empty() { "New chat" } else { title };
    let mode = input.mode.trim();
    let mode = if mode.is_empty() { "ask" } else { mode };
    let summary = input.summary.unwrap_or_default();
    let now = now_utc();

    sqlx::query(
        r#"
        INSERT INTO ask_chats (id, title, mode, summary, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            mode = excluded.mode,
            summary = excluded.summary,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&id)
    .bind(title)
    .bind(mode)
    .bind(&summary)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query_as::<_, AskChatRecord>(
        r#"
        SELECT id, title, mode, summary, archived_at, created_at, updated_at
        FROM ask_chats
        WHERE id = ?
        "#,
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}

pub async fn delete_ask_chat(pool: &SqlitePool, chat_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM ask_chats WHERE id = ?")
        .bind(chat_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn archive_ask_chat(pool: &SqlitePool, chat_id: &str) -> Result<(), String> {
    let now = now_utc();
    sqlx::query("UPDATE ask_chats SET archived_at = ?, updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&now)
        .bind(chat_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn append_ask_turn(
    pool: &SqlitePool,
    input: AppendAskTurnInput,
) -> Result<AskTurnRecord, String> {
    let now = now_utc();
    let answer = input.answer.unwrap_or_default();
    let reasoning = input.reasoning.unwrap_or_default();
    let refs_json = serde_json::to_string(&input.refs).map_err(|e| format!("encode refs: {e}"))?;
    let questions_json =
        serde_json::to_string(&input.questions).map_err(|e| format!("encode questions: {e}"))?;
    let steps_json =
        serde_json::to_string(&input.steps).map_err(|e| format!("encode steps: {e}"))?;
    let scored_nodes_json = if input.scored_nodes.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&input.scored_nodes)
                .map_err(|e| format!("encode scored_nodes: {e}"))?,
        )
    };

    let mut tx = pool.begin().await.map_err(sql_error)?;

    sqlx::query(
        r#"
        INSERT INTO ask_turns (
            id, chat_id, parent_id, fork_of, mode, question, answer, reasoning,
            status, error, refs_json, questions_json, steps_json,
            scored_nodes_json, retrieval_query,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            answer = excluded.answer,
            reasoning = excluded.reasoning,
            status = excluded.status,
            error = excluded.error,
            refs_json = excluded.refs_json,
            questions_json = excluded.questions_json,
            steps_json = excluded.steps_json,
            scored_nodes_json = COALESCE(excluded.scored_nodes_json, ask_turns.scored_nodes_json),
            retrieval_query = COALESCE(excluded.retrieval_query, ask_turns.retrieval_query),
            updated_at = excluded.updated_at
        "#,
    )
    .bind(&input.turn_id)
    .bind(&input.chat_id)
    .bind(&input.parent_id)
    .bind(&input.fork_of)
    .bind(&input.mode)
    .bind(&input.question)
    .bind(&answer)
    .bind(&reasoning)
    .bind(&input.status)
    .bind(&input.error)
    .bind(&refs_json)
    .bind(&questions_json)
    .bind(&steps_json)
    .bind(&scored_nodes_json)
    .bind(&input.retrieval_query)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    // Replace attachments for this turn (idempotent on retry).
    sqlx::query("DELETE FROM ask_turn_attachments WHERE turn_id = ?")
        .bind(&input.turn_id)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

    for attachment in &input.attachments {
        let attachment_id = Ulid::new().to_string();
        sqlx::query(
            r#"
            INSERT INTO ask_turn_attachments (id, turn_id, mime_type, filename, data_b64, size_bytes, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&attachment_id)
        .bind(&input.turn_id)
        .bind(&attachment.mime_type)
        .bind(&attachment.filename)
        .bind(&attachment.data)
        .bind(attachment.size.unwrap_or(attachment.data.len() as i64))
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    }

    sqlx::query("UPDATE ask_chats SET updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&input.chat_id)
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

    tx.commit().await.map_err(sql_error)?;

    let turn_row: AskTurnRow = sqlx::query_as(
        r#"
        SELECT id, chat_id, parent_id, fork_of, mode, question, answer, reasoning,
               status, error, refs_json, questions_json, steps_json,
               scored_nodes_json, retrieval_query,
               created_at, updated_at
        FROM ask_turns
        WHERE id = ?
        "#,
    )
    .bind(&input.turn_id)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    let attachments: Vec<AskTurnAttachmentRecord> = sqlx::query_as(
        r#"
        SELECT id, turn_id, mime_type, filename, data_b64, size_bytes, created_at
        FROM ask_turn_attachments
        WHERE turn_id = ?
        ORDER BY created_at ASC
        "#,
    )
    .bind(&input.turn_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    Ok(turn_row.into_record(attachments))
}

pub async fn search_ask_chats(
    pool: &SqlitePool,
    query: &str,
    limit: Option<i64>,
) -> Result<Vec<AskChatSearchHit>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let fts = match fts_query(trimmed) {
        Some(value) => value,
        None => return Ok(Vec::new()),
    };
    let limit = limit.unwrap_or(40).clamp(1, 200);
    let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT s.chat_id, s.turn_id,
               snippet(ask_chat_search, 2, '[', ']', '…', 12) AS question_snippet,
               snippet(ask_chat_search, 3, '[', ']', '…', 16) AS answer_snippet,
               t.created_at
        FROM ask_chat_search s
        JOIN ask_turns t ON t.id = s.turn_id
        WHERE ask_chat_search MATCH ?
        ORDER BY bm25(ask_chat_search), t.created_at DESC
        LIMIT ?
        "#,
    )
    .bind(&fts)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let chat_ids: Vec<String> = rows.iter().map(|row| row.0.clone()).collect();
    let mut titles: BTreeMap<String, String> = BTreeMap::new();
    if !chat_ids.is_empty() {
        let placeholders = vec!["?"; chat_ids.len()].join(",");
        let sql = format!("SELECT id, title FROM ask_chats WHERE id IN ({placeholders})");
        let mut query_builder = sqlx::query_as::<_, (String, String)>(&sql);
        for id in &chat_ids {
            query_builder = query_builder.bind(id);
        }
        let title_rows = query_builder.fetch_all(pool).await.map_err(sql_error)?;
        for (id, title) in title_rows {
            titles.insert(id, title);
        }
    }

    Ok(rows
        .into_iter()
        .map(
            |(chat_id, turn_id, question_snippet, answer_snippet, created_at)| AskChatSearchHit {
                chat_title: titles
                    .get(&chat_id)
                    .cloned()
                    .unwrap_or_else(|| "Untitled".to_string()),
                chat_id,
                turn_id,
                question_snippet,
                answer_snippet,
                created_at,
            },
        )
        .collect())
}

#[derive(sqlx::FromRow)]
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
    questions_json: String,
    steps_json: String,
    /// Section 6.2 — JSON-encoded `Vec<ScoredBrainNode>`. NULL when the
    /// turn pre-dates the column or made no brain-context retrieval call.
    scored_nodes_json: Option<String>,
    retrieval_query: Option<String>,
    created_at: String,
    updated_at: String,
}

impl AskTurnRow {
    fn into_record(self, attachments: Vec<AskTurnAttachmentRecord>) -> AskTurnRecord {
        let scored_nodes = self
            .scored_nodes_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        AskTurnRecord {
            id: self.id,
            chat_id: self.chat_id,
            parent_id: self.parent_id,
            fork_of: self.fork_of,
            mode: self.mode,
            question: self.question,
            answer: self.answer,
            reasoning: self.reasoning,
            status: self.status,
            error: self.error,
            refs: serde_json::from_str(&self.refs_json).unwrap_or_default(),
            questions: serde_json::from_str(&self.questions_json).unwrap_or_default(),
            steps: serde_json::from_str(&self.steps_json).unwrap_or_default(),
            attachments,
            scored_nodes,
            retrieval_query: self.retrieval_query,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

// ── Calendar brain tools (read from local gcal_events cache) ─────────────────

pub async fn get_user_profile(pool: &SqlitePool) -> Result<UserProfile, String> {
    sqlx::query_as::<_, UserProfile>(
        "SELECT id, name, role, bio, avatar_url, email, updated_at FROM user_profile WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)
}

pub async fn update_user_profile(
    pool: &SqlitePool,
    input: UpdateUserProfileInput,
) -> Result<UserProfile, String> {
    sqlx::query(
        r#"
        UPDATE user_profile
        SET name = ?, role = ?, bio = ?, avatar_url = ?, email = ?, updated_at = ?
        WHERE id = 1
        "#,
    )
    .bind(input.name)
    .bind(input.role)
    .bind(input.bio)
    .bind(input.avatar_url)
    .bind(input.email)
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_user_profile(pool).await
}
