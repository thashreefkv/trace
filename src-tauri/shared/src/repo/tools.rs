//! Thin agentic tool wrappers that Gemini calls during the Ask agentic loop.
//!
//! Each function takes a SqlitePool plus the request arguments and returns a
//! serde_json::Value — never an error type. Failures become `{ "error": "..." }`.
//! The bulk of these wrappers delegate to one or two functions in the domain
//! submodules (deliverables, initiatives, stakeholders, captures, conversations,
//! meetings, memories, search) and shape the result into a tool-call response.

use chrono::Duration;
use sqlx::SqlitePool;

use crate::models::{
    CreateMemoryInput, DeliverableFilters, MemoryKind, RetrieveMemoryInput, WorkGraphFilters,
};

use super::{
    create_capture, create_deliverable, create_deliverable_note, create_deliverable_task,
    create_initiative_note, create_memory, fts_query, get_conversation, get_deliverable,
    get_deliverable_task, get_initiative, get_meeting, get_week_view, get_work_context_graph,
    list_deliverable_notes, list_deliverable_tasks, list_deliverables,
    list_deliverables_for_initiative, list_initiative_notes, list_initiatives,
    list_stakeholder_details, retrieve_memories, set_deliverable_focus, update_deliverable_metadata,
    update_deliverable_state, update_deliverable_task,
};

pub async fn tool_search_deliverables(
    pool: &SqlitePool,
    query: &str,
    state: Option<&str>,
) -> serde_json::Value {
    let like_q = format!("%{query}%");
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = if let Some(fts) = fts_query(query) {
        if let Some(s) = state {
            sqlx::query_as(
                r#"SELECT d.id, d.title, d.state, d.claim, d.blocker_reason, d.deadline
                   FROM deliverable_search ds JOIN deliverables d ON d.rowid = ds.rowid
                   WHERE deliverable_search MATCH ? AND d.state = ?
                   ORDER BY rank LIMIT 20"#,
            )
            .bind(&fts)
            .bind(s)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query_as(
                r#"SELECT d.id, d.title, d.state, d.claim, d.blocker_reason, d.deadline
                   FROM deliverable_search ds JOIN deliverables d ON d.rowid = ds.rowid
                   WHERE deliverable_search MATCH ?
                   ORDER BY rank LIMIT 20"#,
            )
            .bind(&fts)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        }
    } else if let Some(s) = state {
        sqlx::query_as(
            "SELECT id, title, state, claim, blocker_reason, deadline
                 FROM deliverables WHERE (title LIKE ? OR claim LIKE ?) AND state = ? LIMIT 20",
        )
        .bind(&like_q)
        .bind(&like_q)
        .bind(s)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as(
            "SELECT id, title, state, claim, blocker_reason, deadline
                 FROM deliverables WHERE title LIKE ? OR claim LIKE ? LIMIT 20",
        )
        .bind(&like_q)
        .bind(&like_q)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    serde_json::json!(rows
        .iter()
        .map(
            |(id, title, state, claim, blocker, deadline)| serde_json::json!({
                "id": id, "title": title, "state": state, "claim": claim,
                "blocker_reason": blocker, "deadline": deadline,
                "route": format!("/deliverables/{id}")
            })
        )
        .collect::<Vec<_>>())
}

pub async fn tool_get_deliverable_detail(pool: &SqlitePool, id: &str) -> serde_json::Value {
    let deliverable = match get_deliverable(pool, id).await {
        Ok(d) => d,
        Err(e) => return serde_json::json!({ "error": e }),
    };
    let tasks = list_deliverable_tasks(pool, id).await.unwrap_or_default();
    let notes = list_deliverable_notes(pool, id).await.unwrap_or_default();

    serde_json::json!({
        "id": &deliverable.id,
        "title": &deliverable.title,
        "type": &deliverable.deliverable_type,
        "state": &deliverable.state,
        "claim": &deliverable.claim,
        "artifact_url": deliverable.artifact_url,
        "deadline": &deliverable.deadline,
        "effort": deliverable.effort,
        "impact": deliverable.impact,
        "blocker_reason": &deliverable.blocker_reason,
        "is_focused": deliverable.is_focused,
        "stakeholders": &deliverable.stakeholders.iter().map(|s| serde_json::json!({
            "id": s.id, "name": s.name, "role": s.role
        })).collect::<Vec<_>>(),
        "initiatives": &deliverable.initiatives.iter().map(|i| serde_json::json!({
            "id": i.id, "title": i.title, "status": i.status
        })).collect::<Vec<_>>(),
        "tasks": tasks.iter().map(|t| serde_json::json!({
            "id": t.id, "title": t.title, "status": t.status,
            "due_date": t.due_date, "notes": t.notes, "url": t.url
        })).collect::<Vec<_>>(),
        "notes": notes.iter().map(|n| serde_json::json!({
            "body": n.body, "created_at": n.created_at
        })).collect::<Vec<_>>(),
        "created_at": &deliverable.created_at,
        "shipped_at": deliverable.shipped_at,
        "route": format!("/deliverables/{}", deliverable.id),
    })
}

pub async fn tool_list_initiatives(pool: &SqlitePool, query: Option<&str>) -> serde_json::Value {
    let initiatives = list_initiatives(pool).await.unwrap_or_default();
    let like_q = query.map(|q| format!("%{q}%"));
    let filtered: Vec<_> = initiatives
        .iter()
        .filter(|i| {
            like_q.as_deref().map_or(true, |lq| {
                let lq_lower = lq.to_lowercase();
                let lq_trimmed = lq_lower.trim_matches('%');
                i.title.to_lowercase().contains(lq_trimmed)
                    || i.framing.to_lowercase().contains(lq_trimmed)
            })
        })
        .collect();

    serde_json::json!(filtered
        .iter()
        .map(|i| serde_json::json!({
            "id": i.id, "title": i.title, "framing": i.framing,
            "status": i.status, "route": format!("/initiatives/{}", i.id)
        }))
        .collect::<Vec<_>>())
}

pub async fn tool_get_initiative_detail(pool: &SqlitePool, id: &str) -> serde_json::Value {
    let initiative = match get_initiative(pool, id).await {
        Ok(i) => i,
        Err(e) => return serde_json::json!({ "error": e }),
    };
    let deliverables = list_deliverables_for_initiative(pool, id)
        .await
        .unwrap_or_default();
    let notes = list_initiative_notes(pool, id).await.unwrap_or_default();

    serde_json::json!({
        "id": &initiative.id,
        "title": &initiative.title,
        "framing": &initiative.framing,
        "status": &initiative.status,
        "deliverables": deliverables.iter().map(|d| serde_json::json!({
            "id": d.id, "title": d.title, "state": d.state, "claim": d.claim,
            "deadline": d.deadline, "blocker_reason": d.blocker_reason,
            "stakeholder_name": d.stakeholder_name, "effort": d.effort, "impact": d.impact
        })).collect::<Vec<_>>(),
        "notes": notes.iter().map(|n| serde_json::json!({
            "body": n.body, "created_at": n.created_at
        })).collect::<Vec<_>>(),
        "route": format!("/initiatives/{id}"),
    })
}

pub async fn tool_search_meetings(pool: &SqlitePool, query: &str) -> serde_json::Value {
    let like_q = format!("%{query}%");
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, title, date, summary FROM meetings
         WHERE title LIKE ? OR summary LIKE ? OR transcript LIKE ?
         ORDER BY date DESC LIMIT 10",
    )
    .bind(&like_q)
    .bind(&like_q)
    .bind(&like_q)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    serde_json::json!(rows
        .iter()
        .map(|(id, title, date, summary)| serde_json::json!({
            "id": id, "title": title, "date": date, "summary": summary,
            "route": format!("/meetings/{id}")
        }))
        .collect::<Vec<_>>())
}

pub async fn tool_get_meeting_detail(pool: &SqlitePool, id: &str) -> serde_json::Value {
    match get_meeting(pool, id).await {
        Ok(m) => serde_json::json!({
            "id": m.meeting.id,
            "title": m.meeting.title,
            "date": m.meeting.date,
            "duration_secs": m.meeting.duration_secs,
            "transcript": m.meeting.transcript,
            "summary": m.meeting.summary,
            "key_decisions": m.meeting.key_decisions,
            "actions": m.actions.iter().map(|a| serde_json::json!({
                "kind": a.kind, "body": a.body,
                "target_title": a.target_title, "applied": a.applied
            })).collect::<Vec<_>>(),
            "route": format!("/meetings/{id}"),
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_get_stakeholders(pool: &SqlitePool) -> serde_json::Value {
    let details = list_stakeholder_details(pool).await.unwrap_or_default();
    serde_json::json!(details
        .iter()
        .map(|s| serde_json::json!({
            "id": s.stakeholder.id,
            "name": s.stakeholder.name,
            "role": s.stakeholder.role,
            "notes": s.stakeholder.notes,
            "deliverable_count": s.deliverable_count,
            "shipped_count": s.shipped_count,
            "in_flight_count": s.in_flight_count,
            "days_since_last_delivery": s.days_since_last_delivery,
        }))
        .collect::<Vec<_>>())
}

pub async fn tool_get_stakeholder_deliverables(
    pool: &SqlitePool,
    stakeholder_id: &str,
) -> serde_json::Value {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"SELECT DISTINCT d.id, d.title, d.state, d.claim, d.deadline, d.blocker_reason
           FROM deliverables d
           LEFT JOIN deliverable_stakeholders ds ON ds.deliverable_id = d.id
           WHERE d.stakeholder_id = ? OR ds.stakeholder_id = ?
           ORDER BY d.updated_at DESC LIMIT 30"#,
    )
    .bind(stakeholder_id)
    .bind(stakeholder_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    serde_json::json!(rows
        .iter()
        .map(
            |(id, title, state, claim, deadline, blocker)| serde_json::json!({
                "id": id, "title": title, "state": state, "claim": claim,
                "deadline": deadline, "blocker_reason": blocker,
                "route": format!("/deliverables/{id}")
            })
        )
        .collect::<Vec<_>>())
}

pub async fn tool_search_captures(pool: &SqlitePool, query: &str) -> serde_json::Value {
    let like_q = format!("%{query}%");
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, kind, body, status FROM captures
         WHERE body LIKE ? ORDER BY created_at DESC LIMIT 15",
    )
    .bind(&like_q)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    serde_json::json!(rows
        .iter()
        .map(|(id, kind, body, status)| serde_json::json!({
            "id": id, "kind": kind, "body": body, "status": status
        }))
        .collect::<Vec<_>>())
}

pub async fn tool_search_conversations(pool: &SqlitePool, query: &str) -> serde_json::Value {
    let like_q = format!("%{query}%");
    let rows: Vec<(
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, chat_url, title, summary, occurred_at, ingested_at
         FROM conversations
         WHERE title LIKE ? OR summary LIKE ? OR chat_url LIKE ?
         ORDER BY COALESCE(occurred_at, ingested_at) DESC
         LIMIT 12",
    )
    .bind(&like_q)
    .bind(&like_q)
    .bind(&like_q)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    serde_json::json!(rows
        .iter()
        .map(
            |(id, chat_url, title, summary, occurred_at, ingested_at)| serde_json::json!({
                "id": id,
                "title": title.as_deref().unwrap_or("Untitled conversation"),
                "summary": summary,
                "occurred_at": occurred_at,
                "ingested_at": ingested_at,
                "route": chat_url,
            })
        )
        .collect::<Vec<_>>())
}

pub async fn tool_get_conversation_detail(pool: &SqlitePool, id: &str) -> serde_json::Value {
    let conversation = match get_conversation(pool, id).await {
        Ok(conversation) => conversation,
        Err(e) => return serde_json::json!({ "error": e }),
    };
    let deliverables = list_deliverables(pool, DeliverableFilters::default())
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|deliverable| deliverable.conversation_id.as_deref() == Some(id))
        .map(|deliverable| {
            serde_json::json!({
                "id": &deliverable.id,
                "title": &deliverable.title,
                "state": &deliverable.state,
                "claim": &deliverable.claim,
                "deadline": &deliverable.deadline,
                "route": format!("/deliverables/{}", deliverable.id),
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "id": &conversation.id,
        "title": &conversation.title.unwrap_or_else(|| "Untitled conversation".to_string()),
        "summary": &conversation.summary,
        "occurred_at": conversation.occurred_at,
        "ingested_at": &conversation.ingested_at,
        "route": conversation.chat_url,
        "linked_deliverables": deliverables,
    })
}

pub async fn tool_get_work_graph_context(pool: &SqlitePool) -> serde_json::Value {
    match get_work_context_graph(pool, WorkGraphFilters::default()).await {
        Ok(graph) => serde_json::json!({
            "generated_at": graph.generated_at,
            "ai_context": graph.ai_context,
            "node_count": graph.nodes.len(),
            "edge_count": graph.edges.len(),
            "nodes": graph.nodes.iter().take(80).map(|node| serde_json::json!({
                "id": node.id,
                "kind": node.kind,
                "entity_id": node.entity_id,
                "label": node.label,
                "subtitle": node.subtitle,
                "status": node.status,
                "route": node.url,
            })).collect::<Vec<_>>(),
            "edges": graph.edges.iter().take(140).map(|edge| serde_json::json!({
                "kind": edge.kind,
                "source": edge.source,
                "target": edge.target,
                "label": edge.label,
            })).collect::<Vec<_>>(),
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_retrieve_memory(pool: &SqlitePool, query: &str) -> serde_json::Value {
    match retrieve_memories(
        pool,
        RetrieveMemoryInput {
            query: query.to_string(),
            limit: Some(16),
            kinds: Vec::new(),
            source_kind: Some("ask_tool".to_string()),
            source_id: None,
            task_type: None,
            include_pinned: Some(true),
        },
    )
    .await
    {
        Ok(result) => serde_json::json!({
            "context": result.context,
            "memories": result.memories.iter().map(|memory| serde_json::json!({
                "id": &memory.id,
                "kind": memory.kind,
                "title": &memory.title,
                "body": &memory.body,
                "source": &memory.source,
                "confidence": memory.confidence,
                "importance": memory.importance,
                "updated_at": &memory.updated_at
            })).collect::<Vec<_>>()
        }),
        Err(error) => serde_json::json!({ "error": error }),
    }
}

pub async fn tool_save_memory(
    pool: &SqlitePool,
    kind: &str,
    title: &str,
    body: &str,
    tags: &[String],
) -> serde_json::Value {
    let kind = match kind {
        "episodic" => MemoryKind::Episodic,
        "procedural" => MemoryKind::Procedural,
        _ => MemoryKind::Semantic,
    };
    match create_memory(
        pool,
        CreateMemoryInput {
            kind,
            title: title.to_string(),
            body: body.to_string(),
            scope: "global".to_string(),
            tags: tags.to_vec(),
            confidence: Some(0.95),
            importance: Some(0.85),
            sensitivity: None,
            pinned: None,
            expires_at: None,
        },
    )
    .await
    {
        Ok(memory) => serde_json::json!({
            "saved": true,
            "memory": memory
        }),
        Err(error) => serde_json::json!({ "saved": false, "error": error }),
    }
}

pub async fn tool_get_blocked_deliverables(pool: &SqlitePool) -> serde_json::Value {
    let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, title, state, claim, blocker_reason FROM deliverables
         WHERE blocker_reason IS NOT NULL AND blocker_reason != ''
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    serde_json::json!(rows
        .iter()
        .map(|(id, title, state, claim, blocker)| serde_json::json!({
            "id": id, "title": title, "state": state, "claim": claim,
            "blocker_reason": blocker, "route": format!("/deliverables/{id}")
        }))
        .collect::<Vec<_>>())
}

pub async fn tool_get_deliverables_by_state(pool: &SqlitePool, state: &str) -> serde_json::Value {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
    )> = sqlx::query_as(
        "SELECT id, title, state, claim, deadline, blocker_reason, effort, impact
             FROM deliverables WHERE state = ? ORDER BY updated_at DESC LIMIT 50",
    )
    .bind(state)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    serde_json::json!(rows
        .iter()
        .map(
            |(id, title, state, claim, deadline, blocker, effort, impact)| serde_json::json!({
                "id": id, "title": title, "state": state, "claim": claim,
                "deadline": deadline, "blocker_reason": blocker,
                "effort": effort, "impact": impact,
                "route": format!("/deliverables/{id}")
            })
        )
        .collect::<Vec<_>>())
}

pub async fn tool_get_high_priority_deliverables(pool: &SqlitePool) -> serde_json::Value {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        Option<i64>,
        Option<i64>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"SELECT id, title, state, claim, impact, effort, deadline, blocker_reason
               FROM deliverables
               WHERE state IN ('drafting', 'in_review')
               ORDER BY
                 CASE WHEN is_focused THEN 0 ELSE 1 END,
                 CASE WHEN blocker_reason IS NOT NULL AND blocker_reason != '' THEN 1 ELSE 0 END,
                 COALESCE(impact, 0) DESC,
                 COALESCE(effort, 5) ASC,
                 COALESCE(deadline, '9999-99-99') ASC
               LIMIT 20"#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    serde_json::json!(rows
        .iter()
        .map(
            |(id, title, state, claim, impact, effort, deadline, blocker)| serde_json::json!({
                "id": id, "title": title, "state": state, "claim": claim,
                "impact": impact, "effort": effort, "deadline": deadline,
                "blocker_reason": blocker, "route": format!("/deliverables/{id}")
            })
        )
        .collect::<Vec<_>>())
}

pub async fn tool_get_current_week(pool: &SqlitePool) -> serde_json::Value {
    use chrono::{Datelike, Utc};
    let today = Utc::now().date_naive();
    let days_from_monday = today.weekday().num_days_from_monday() as i64;
    let monday = today - chrono::Duration::days(days_from_monday);
    let week_start = monday.format("%Y-%m-%d").to_string();

    match get_week_view(pool, &week_start, std::path::Path::new("")).await {
        Ok(week) => serde_json::json!({
            "week_start": week.week_start,
            "meeting_date": week.meeting_date,
            "days": week.days.iter().map(|d| serde_json::json!({
                "day_index": d.day_index,
                "deliverable": d.deliverable.as_ref().map(|del| serde_json::json!({
                    "id": del.id, "title": del.title,
                    "state": del.state, "claim": del.claim,
                    "deadline": del.deadline
                })),
                "tasks": d.tasks.iter().map(|t| serde_json::json!({
                    "title": t.title, "status": t.status,
                    "deliverable_title": t.deliverable_title
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_get_recent_activity(pool: &SqlitePool) -> serde_json::Value {
    let recent_deliverables: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, title, state, updated_at FROM deliverables
         ORDER BY updated_at DESC LIMIT 10",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let recent_meetings: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, title, date FROM meetings ORDER BY date DESC LIMIT 5")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    let recent_captures: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, body, created_at FROM captures ORDER BY created_at DESC LIMIT 8",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    serde_json::json!({
        "recent_deliverables": recent_deliverables.iter().map(|(id, title, state, updated_at)| serde_json::json!({
            "id": id, "title": title, "state": state, "updated_at": updated_at,
            "route": format!("/deliverables/{id}")
        })).collect::<Vec<_>>(),
        "recent_meetings": recent_meetings.iter().map(|(id, title, date)| serde_json::json!({
            "id": id, "title": title, "date": date,
            "route": format!("/meetings/{id}")
        })).collect::<Vec<_>>(),
        "recent_captures": recent_captures.iter().map(|(id, body, created_at)| serde_json::json!({
            "id": id, "body": body, "created_at": created_at
        })).collect::<Vec<_>>(),
    })
}

pub async fn tool_get_workspace_summary(pool: &SqlitePool) -> serde_json::Value {
    let counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT COUNT(*) FROM initiatives WHERE status = 'live') as live_initiatives,
             (SELECT COUNT(*) FROM deliverables WHERE state = 'drafting') as drafting,
             (SELECT COUNT(*) FROM deliverables WHERE state = 'in_review') as in_review,
             (SELECT COUNT(*) FROM deliverables WHERE state = 'shipped'
              AND shipped_at >= date('now', '-30 days')) as shipped_30d,
             (SELECT COUNT(*) FROM captures WHERE status = 'inbox') as inbox_captures"#,
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0, 0, 0, 0, 0));

    let blocked_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM deliverables
         WHERE blocker_reason IS NOT NULL AND blocker_reason != ''
         AND state IN ('drafting', 'in_review')",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let today = chrono::Utc::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let today_events = tool_get_calendar_events(pool, &today).await;

    serde_json::json!({
        "live_initiatives": counts.0,
        "drafting_deliverables": counts.1,
        "in_review_deliverables": counts.2,
        "shipped_last_30_days": counts.3,
        "inbox_captures": counts.4,
        "blocked_deliverables": blocked_count,
        "today_calendar": today_events,
    })
}

// ── Agentic write tools ───────────────────────────────────────────────────────

pub async fn tool_add_deliverable_note(
    pool: &SqlitePool,
    deliverable_id: &str,
    body: &str,
) -> serde_json::Value {
    let input = crate::models::CreateNoteInput {
        deliverable_id: deliverable_id.to_string(),
        body: body.to_string(),
    };
    match create_deliverable_note(pool, input).await {
        Ok(note) => serde_json::json!({
            "success": true,
            "id": note.id,
            "deliverable_id": note.deliverable_id,
            "body": note.body,
            "created_at": note.created_at,
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_add_initiative_note(
    pool: &SqlitePool,
    initiative_id: &str,
    body: &str,
) -> serde_json::Value {
    match create_initiative_note(pool, initiative_id, body).await {
        Ok(note) => serde_json::json!({
            "success": true,
            "id": note.id,
            "initiative_id": note.initiative_id,
            "body": note.body,
            "created_at": note.created_at,
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_create_capture(pool: &SqlitePool, body: &str) -> serde_json::Value {
    let input = crate::models::CreateCaptureInput {
        kind: crate::models::CaptureKind::Thought,
        body: body.to_string(),
    };
    match create_capture(pool, input).await {
        Ok(cap) => serde_json::json!({
            "success": true,
            "id": cap.id,
            "body": cap.body,
            "status": cap.status,
            "created_at": cap.created_at,
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_update_deliverable_state(
    pool: &SqlitePool,
    id: &str,
    state_str: &str,
) -> serde_json::Value {
    let state = match state_str {
        "drafting" => crate::models::DeliverableState::Drafting,
        "in_review" => crate::models::DeliverableState::InReview,
        "shipped" => crate::models::DeliverableState::Shipped,
        "killed" => crate::models::DeliverableState::Killed,
        other => return serde_json::json!({ "error": format!("Unknown state: {other}") }),
    };
    match update_deliverable_state(pool, id, state).await {
        Ok(d) => serde_json::json!({
            "success": true,
            "id": d.id,
            "title": d.title,
            "state": d.state,
            "shipped_at": d.shipped_at,
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_set_deliverable_focus(
    pool: &SqlitePool,
    id: &str,
    focused: bool,
) -> serde_json::Value {
    match set_deliverable_focus(pool, id, focused).await {
        Ok(d) => serde_json::json!({
            "success": true,
            "id": d.id,
            "title": d.title,
            "is_focused": d.is_focused,
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_add_deliverable_task(
    pool: &SqlitePool,
    deliverable_id: &str,
    title: &str,
    due_date: Option<&str>,
    notes: Option<&str>,
    url: Option<&str>,
) -> serde_json::Value {
    let input = crate::models::CreateTaskInput {
        deliverable_id: deliverable_id.to_string(),
        title: title.to_string(),
        due_date: due_date.map(|s| s.to_string()),
        notes: notes.map(|s| s.to_string()),
        url: url.map(|s| s.to_string()),
    };
    match create_deliverable_task(pool, input).await {
        Ok(task) => serde_json::json!({
            "success": true,
            "id": task.id,
            "deliverable_id": task.deliverable_id,
            "title": task.title,
            "status": task.status,
            "due_date": task.due_date,
            "notes": task.notes,
            "url": task.url,
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_update_task_status(
    pool: &SqlitePool,
    task_id: &str,
    status_str: &str,
) -> serde_json::Value {
    let status = match status_str {
        "todo" => crate::models::TaskStatus::Todo,
        "doing" => crate::models::TaskStatus::Doing,
        "done" => crate::models::TaskStatus::Done,
        other => return serde_json::json!({ "error": format!("Unknown status: {other}") }),
    };

    let current = match get_deliverable_task(pool, task_id).await {
        Ok(t) => t,
        Err(_) => return serde_json::json!({ "error": "task not found" }),
    };

    let input = crate::models::UpdateTaskInput {
        title: current.title,
        status,
        due_date: current.due_date,
        notes: current.notes,
        url: current.url,
    };
    match update_deliverable_task(pool, task_id, input).await {
        Ok(task) => serde_json::json!({
            "success": true,
            "id": task.id,
            "title": task.title,
            "status": task.status,
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_list_pending_tasks(pool: &SqlitePool) -> serde_json::Value {
    let rows: Vec<(String, String, String, String, Option<String>, String)> =
        sqlx::query_as(
            r#"
            SELECT t.id, t.title, t.status, d.title AS deliverable_title, t.due_date, d.id AS deliverable_id
            FROM deliverable_tasks t
            JOIN deliverables d ON d.id = t.deliverable_id
            WHERE t.status IN ('todo', 'doing')
              AND d.state NOT IN ('shipped', 'dropped')
            ORDER BY
              CASE WHEN t.status = 'doing' THEN 0 ELSE 1 END,
              COALESCE(t.due_date, '9999-99-99') ASC,
              t.display_order ASC
            LIMIT 50
            "#,
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    serde_json::json!(rows
        .iter()
        .map(
            |(id, title, status, deliverable_title, due_date, deliverable_id)| serde_json::json!({
                "id": id,
                "title": title,
                "status": status,
                "due_date": due_date,
                "deliverable_title": deliverable_title,
                "deliverable_id": deliverable_id,
                "route": format!("/deliverables/{deliverable_id}")
            })
        )
        .collect::<Vec<_>>())
}
pub async fn tool_update_deliverable_metadata(
    pool: &SqlitePool,
    id: &str,
    new_deadline: Option<&str>,
    new_effort: Option<i64>,
    new_impact: Option<i64>,
    new_blocker: Option<&str>,
) -> serde_json::Value {
    // Fetch current to enable partial updates (DB function overwrites all fields)
    let current = match get_deliverable(pool, id).await {
        Ok(d) => d,
        Err(e) => return serde_json::json!({ "error": e }),
    };

    let deadline = new_deadline
        .map(|d| {
            let d = d.trim().to_string();
            if d.is_empty() {
                None
            } else {
                Some(d)
            }
        })
        .unwrap_or(current.deadline);

    let effort = new_effort.or(current.effort);
    let impact = new_impact.or(current.impact);

    let blocker_reason = new_blocker
        .map(|r| {
            let r = r.trim().to_string();
            if r.is_empty() {
                None
            } else {
                Some(r)
            }
        })
        .unwrap_or(current.blocker_reason);

    let input = crate::models::UpdateDeliverableMetadataInput {
        deadline,
        effort,
        impact,
        blocker_reason,
        priority: None,
    };
    match update_deliverable_metadata(pool, id, input).await {
        Ok(d) => serde_json::json!({
            "success": true,
            "id": d.id,
            "title": d.title,
            "deadline": d.deadline,
            "effort": d.effort,
            "impact": d.impact,
            "blocker_reason": d.blocker_reason,
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_flag_new_deliverable(
    pool: &sqlx::SqlitePool,
    title: &str,
    claim: &str,
    suggested_type: &str,
    suggested_initiative: &str,
) -> serde_json::Value {
    let body = format!(
        "[CANDIDATE] {}\nClaim: {}\nType: {}\nInitiative hint: {}",
        title,
        claim,
        if suggested_type.is_empty() {
            "other"
        } else {
            suggested_type
        },
        if suggested_initiative.is_empty() {
            "none"
        } else {
            suggested_initiative
        }
    );
    let _ = tool_create_capture(pool, &body).await;
    serde_json::json!({ "flagged": true, "title": title })
}

pub async fn tool_create_deliverable_from_email(
    pool: &SqlitePool,
    thread_id: &str,
    title: &str,
    claim: &str,
    deliverable_type: &str,
    initiative_id: Option<&str>,
) -> serde_json::Value {
    let detail = match crate::gmail::get_local_thread(pool, thread_id).await {
        Ok(detail) => detail,
        Err(e) => return serde_json::json!({ "error": e }),
    };
    let deliverable_type = parse_agentic_deliverable_type(deliverable_type);
    let initiative_ids = initiative_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| vec![id.to_string()])
        .unwrap_or_else(|| {
            detail
                .thread
                .linked_initiatives
                .iter()
                .map(|initiative| initiative.id.clone())
                .collect()
        });
    let artifact_url = detail.thread.artifact_urls.first().cloned();
    let input = crate::models::CreateDeliverableInput {
        title: if title.trim().is_empty() {
            detail.thread.subject.clone()
        } else {
            title.trim().to_string()
        },
        deliverable_type,
        state: crate::models::DeliverableState::Backlog,
        claim: if claim.trim().is_empty() {
            let inferred = detail
                .thread
                .summary
                .clone()
                .unwrap_or_else(|| detail.thread.snippet.clone());
            if inferred.trim().is_empty() {
                "Created from Gmail thread.".to_string()
            } else {
                inferred
            }
        } else {
            claim.trim().to_string()
        },
        artifact_url,
        conversation_id: None,
        stakeholder_id: None,
        stakeholder_ids: Vec::new(),
        initiative_ids,
    };
    match create_deliverable(pool, input).await {
        Ok(deliverable) => {
            let _ =
                crate::gmail::link_thread_to_deliverable(pool, thread_id, &deliverable.id).await;
            serde_json::json!({
                "success": true,
                "id": &deliverable.id,
                "title": &deliverable.title,
                "state": &deliverable.state,
                "route": format!("/deliverables/{}", deliverable.id),
                "thread_id": thread_id,
            })
        }
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_link_email_thread_to_deliverable(
    pool: &SqlitePool,
    thread_id: &str,
    deliverable_id: &str,
) -> serde_json::Value {
    match crate::gmail::link_thread_to_deliverable(pool, thread_id, deliverable_id).await {
        Ok(()) => serde_json::json!({
            "success": true,
            "thread_id": thread_id,
            "deliverable_id": deliverable_id,
            "route": format!("/email?thread={thread_id}")
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_link_email_thread_to_initiative(
    pool: &SqlitePool,
    thread_id: &str,
    initiative_id: &str,
) -> serde_json::Value {
    match crate::gmail::link_thread_to_initiative(pool, thread_id, initiative_id).await {
        Ok(()) => serde_json::json!({
            "success": true,
            "thread_id": thread_id,
            "initiative_id": initiative_id,
            "route": format!("/email?thread={thread_id}")
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_capture_email_thread(pool: &SqlitePool, thread_id: &str) -> serde_json::Value {
    match crate::gmail::create_capture_from_thread(pool, thread_id).await {
        Ok(capture) => serde_json::json!({
            "success": true,
            "id": &capture.id,
            "body": &capture.body,
            "route": "/captures"
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub fn parse_agentic_deliverable_type(value: &str) -> crate::models::DeliverableType {
    match value {
        "deck" => crate::models::DeliverableType::Deck,
        "design_doc" => crate::models::DeliverableType::DesignDoc,
        "prototype" => crate::models::DeliverableType::Prototype,
        "analysis" => crate::models::DeliverableType::Analysis,
        "framework" => crate::models::DeliverableType::Framework,
        "pitch" => crate::models::DeliverableType::Pitch,
        "research" => crate::models::DeliverableType::Research,
        "code" => crate::models::DeliverableType::Code,
        "email" => crate::models::DeliverableType::Email,
        "meeting_prep" => crate::models::DeliverableType::MeetingPrep,
        _ => crate::models::DeliverableType::Other,
    }
}

pub async fn tool_search_email_threads(
    pool: &SqlitePool,
    query: &str,
    category: Option<&str>,
    limit: Option<i64>,
) -> serde_json::Value {
    match crate::gmail::list_local_threads(
        pool,
        crate::gmail::GmailThreadFilter {
            query: if query.is_empty() {
                None
            } else {
                Some(query.to_string())
            },
            category: category.map(|c| c.to_string()),
            limit: Some(limit.unwrap_or(12).min(50)),
            ..crate::gmail::GmailThreadFilter::default()
        },
    )
    .await
    {
        Ok(threads) => serde_json::json!(threads
            .iter()
            .map(|thread| serde_json::json!({
                "id": thread.thread_id,
                "subject": thread.subject,
                "snippet": thread.snippet,
                "participants": thread.participants,
                "last_message_at": thread.last_message_at,
                "ai_category": thread.ai_category,
                "ai_priority": thread.ai_priority,
                "summary": thread.summary,
                "sentiment": thread.sentiment,
                "urgency": thread.urgency,
                "route": format!("/email?thread={}", thread.thread_id)
            }))
            .collect::<Vec<_>>()),
        Err(e) => serde_json::json!({ "error": e }),
    }
}

pub async fn tool_get_email_category_summary(pool: &SqlitePool) -> serde_json::Value {
    let counts = match crate::gmail::category_counts(pool).await {
        Ok(c) => c,
        Err(e) => return serde_json::json!({ "error": e }),
    };
    let account_email: Option<String> =
        sqlx::query_scalar("SELECT account_email FROM gmail_sync_settings WHERE id = 1")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten();

    serde_json::json!({
        "account_email": account_email,
        "note": "Use account_email to identify emails addressed TO you. Categories: work, personal, newsletter, notification, other.",
        "categories": counts.iter().map(|c| serde_json::json!({
            "category": c.category,
            "thread_count": c.count
        })).collect::<Vec<_>>()
    })
}

pub async fn tool_get_email_thread(pool: &SqlitePool, thread_id: &str) -> serde_json::Value {
    match crate::gmail::get_local_thread(pool, thread_id).await {
        Ok(detail) => serde_json::json!({
            "id": detail.thread.thread_id,
            "subject": detail.thread.subject,
            "snippet": detail.thread.snippet,
            "participants": detail.thread.participants,
            "labels": detail.thread.labels,
            "linked_deliverables": detail.thread.linked_deliverables,
            "linked_initiatives": detail.thread.linked_initiatives,
            "attachments": detail.attachments,
            "links": detail.links,
            "messages": detail.messages.iter().map(|message| serde_json::json!({
                "from": {"name": message.from_name, "email": message.from_email},
                "to": message.to,
                "cc": message.cc,
                "date_ts": message.internal_date_ts.or(message.date_ts),
                "is_sent": message.is_sent,
                "body": if message.plain_body.trim().is_empty() {
                    message.snippet.clone()
                } else {
                    message.plain_body.chars().take(4000).collect::<String>()
                }
            })).collect::<Vec<_>>(),
            "route": format!("/email?thread={thread_id}")
        }),
        Err(e) => serde_json::json!({ "error": e }),
    }
}
fn parse_json_col(s: Option<&str>) -> Option<serde_json::Value> {
    s.and_then(|v| serde_json::from_str(v).ok())
}

pub async fn tool_get_calendar_events(pool: &SqlitePool, date: &str) -> serde_json::Value {
    #[derive(sqlx::FromRow)]
    struct Row {
        title: String,
        gcal_event_id: String,
        start_date: String,
        start_datetime: Option<String>,
        end_datetime: Option<String>,
        is_all_day: bool,
        description: Option<String>,
        location: Option<String>,
        attendees: Option<String>,
        conference_data: Option<String>,
        organizer: Option<String>,
        recurring_event_id: Option<String>,
        color_id: Option<String>,
        transparency: Option<String>,
        attachments: Option<String>,
        reminders: Option<String>,
        visibility: Option<String>,
        html_link: Option<String>,
    }
    match sqlx::query_as::<_, Row>(
        "SELECT title, gcal_event_id, start_date, start_datetime, end_datetime, is_all_day,
                description, location, attendees, conference_data, organizer, recurring_event_id,
                color_id, transparency, attachments, reminders, visibility, html_link
         FROM gcal_events WHERE start_date = ? ORDER BY is_all_day DESC, start_datetime ASC",
    )
    .bind(date)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => serde_json::json!({
            "date": date,
            "event_count": rows.len(),
            "events": rows.iter().map(|r| serde_json::json!({
                "gcal_event_id": r.gcal_event_id,
                "title": r.title,
                "all_day": r.is_all_day,
                "start_time": r.start_datetime.as_deref().map(|dt| &dt[11..16]),
                "end_time": r.end_datetime.as_deref().map(|dt| &dt[11..16]),
                "description": r.description,
                "location": r.location,
                "attendees": parse_json_col(r.attendees.as_deref()),
                "conference": parse_json_col(r.conference_data.as_deref()),
                "organizer": parse_json_col(r.organizer.as_deref()),
                "recurring": r.recurring_event_id.is_some(),
                "color_id": r.color_id,
                "transparency": r.transparency,
                "attachments": parse_json_col(r.attachments.as_deref()),
                "reminders": parse_json_col(r.reminders.as_deref()),
                "visibility": r.visibility,
                "link": r.html_link
            })).collect::<Vec<_>>()
        }),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
}

pub async fn tool_get_calendar_week(pool: &SqlitePool, week_start: &str) -> serde_json::Value {
    let start = match chrono::NaiveDate::parse_from_str(week_start, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return serde_json::json!({ "error": "Invalid week_start, use YYYY-MM-DD" }),
    };
    let week_end = (start + Duration::days(6)).format("%Y-%m-%d").to_string();

    #[derive(sqlx::FromRow)]
    struct Row {
        title: String,
        gcal_event_id: String,
        start_date: String,
        start_datetime: Option<String>,
        end_datetime: Option<String>,
        is_all_day: bool,
        description: Option<String>,
        location: Option<String>,
        attendees: Option<String>,
        conference_data: Option<String>,
        organizer: Option<String>,
        recurring_event_id: Option<String>,
        color_id: Option<String>,
        transparency: Option<String>,
    }
    match sqlx::query_as::<_, Row>(
        "SELECT title, gcal_event_id, start_date, start_datetime, end_datetime, is_all_day,
                description, location, attendees, conference_data, organizer, recurring_event_id,
                color_id, transparency
         FROM gcal_events WHERE start_date >= ? AND start_date <= ?
         ORDER BY start_date ASC, is_all_day DESC, start_datetime ASC",
    )
    .bind(week_start)
    .bind(&week_end)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => {
            // Group by date
            let mut by_date: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
                std::collections::BTreeMap::new();
            for r in &rows {
                by_date
                    .entry(r.start_date.clone())
                    .or_default()
                    .push(serde_json::json!({
                        "gcal_event_id": r.gcal_event_id,
                        "title": r.title,
                        "all_day": r.is_all_day,
                        "start_time": r.start_datetime.as_deref().map(|dt| &dt[11..16]),
                        "end_time": r.end_datetime.as_deref().map(|dt| &dt[11..16]),
                        "description": r.description,
                        "location": r.location,
                        "attendees": parse_json_col(r.attendees.as_deref()),
                        "conference": parse_json_col(r.conference_data.as_deref()),
                        "organizer": parse_json_col(r.organizer.as_deref()),
                        "recurring": r.recurring_event_id.is_some(),
                        "color_id": r.color_id,
                        "transparency": r.transparency,
                    }));
            }
            serde_json::json!({
                "week_start": week_start,
                "week_end": week_end,
                "total_events": rows.len(),
                "days": by_date
            })
        }
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
}

pub async fn tool_get_upcoming_events(pool: &SqlitePool, days: i64) -> serde_json::Value {
    let today = chrono::Utc::now().date_naive();
    let end = today + Duration::days(days.clamp(1, 90));
    let today_str = today.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();

    #[derive(sqlx::FromRow)]
    struct Row {
        title: String,
        gcal_event_id: String,
        start_date: String,
        start_datetime: Option<String>,
        end_datetime: Option<String>,
        is_all_day: bool,
        description: Option<String>,
        location: Option<String>,
        attendees: Option<String>,
        conference_data: Option<String>,
        organizer: Option<String>,
        recurring_event_id: Option<String>,
        color_id: Option<String>,
        transparency: Option<String>,
    }
    match sqlx::query_as::<_, Row>(
        "SELECT title, gcal_event_id, start_date, start_datetime, end_datetime, is_all_day,
                description, location, attendees, conference_data, organizer, recurring_event_id,
                color_id, transparency
         FROM gcal_events WHERE start_date >= ? AND start_date <= ?
         ORDER BY start_date ASC, is_all_day DESC, start_datetime ASC LIMIT 50",
    )
    .bind(&today_str)
    .bind(&end_str)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => serde_json::json!({
            "from": today_str,
            "to": end_str,
            "event_count": rows.len(),
            "events": rows.iter().map(|r| serde_json::json!({
                "gcal_event_id": r.gcal_event_id,
                "date": r.start_date,
                "title": r.title,
                "all_day": r.is_all_day,
                "start_time": r.start_datetime.as_deref().map(|dt| &dt[11..16]),
                "end_time": r.end_datetime.as_deref().map(|dt| &dt[11..16]),
                "description": r.description,
                "location": r.location,
                "attendees": parse_json_col(r.attendees.as_deref()),
                "conference": parse_json_col(r.conference_data.as_deref()),
                "organizer": parse_json_col(r.organizer.as_deref()),
                "recurring": r.recurring_event_id.is_some(),
                "color_id": r.color_id,
                "transparency": r.transparency,
            })).collect::<Vec<_>>()
        }),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
}

pub async fn tool_search_calendar_events(pool: &SqlitePool, query: &str) -> serde_json::Value {
    let pattern = format!("%{query}%");
    #[derive(sqlx::FromRow)]
    struct Row {
        title: String,
        gcal_event_id: String,
        start_date: String,
        start_datetime: Option<String>,
        end_datetime: Option<String>,
        is_all_day: bool,
        description: Option<String>,
        location: Option<String>,
        attendees: Option<String>,
        conference_data: Option<String>,
        organizer: Option<String>,
        recurring_event_id: Option<String>,
    }
    match sqlx::query_as::<_, Row>(
        "SELECT title, gcal_event_id, start_date, start_datetime, end_datetime, is_all_day,
                description, location, attendees, conference_data, organizer, recurring_event_id
         FROM gcal_events WHERE title LIKE ? OR description LIKE ?
         ORDER BY start_date ASC LIMIT 30",
    )
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => serde_json::json!({
            "query": query,
            "result_count": rows.len(),
            "events": rows.iter().map(|r| serde_json::json!({
                "gcal_event_id": r.gcal_event_id,
                "date": r.start_date,
                "title": r.title,
                "all_day": r.is_all_day,
                "start_time": r.start_datetime.as_deref().map(|dt| &dt[11..16]),
                "end_time": r.end_datetime.as_deref().map(|dt| &dt[11..16]),
                "description": r.description,
                "location": r.location,
                "attendees": parse_json_col(r.attendees.as_deref()),
                "conference": parse_json_col(r.conference_data.as_deref()),
                "organizer": parse_json_col(r.organizer.as_deref()),
                "recurring": r.recurring_event_id.is_some(),
            })).collect::<Vec<_>>()
        }),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
}

pub async fn tool_find_free_slots(pool: &SqlitePool, date: &str) -> serde_json::Value {
    #[derive(sqlx::FromRow)]
    struct Row {
        start_datetime: Option<String>,
        end_datetime: Option<String>,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT start_datetime, end_datetime FROM gcal_events
         WHERE start_date = ? AND is_all_day = 0
         ORDER BY start_datetime ASC",
    )
    .bind(date)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Build occupied intervals as (start_minutes, end_minutes) since midnight
    let occupied: Vec<(i32, i32)> = rows
        .iter()
        .filter_map(|r| {
            let s = r.start_datetime.as_deref()?;
            let e = r.end_datetime.as_deref()?;
            let sm = time_to_minutes(&s[11..16])?;
            let em = time_to_minutes(&e[11..16])?;
            Some((sm, em))
        })
        .collect();

    let work_start = 9 * 60; // 09:00
    let work_end = 18 * 60; // 18:00
    let slot_len = 30; // 30-minute minimum slot

    let mut free: Vec<serde_json::Value> = Vec::new();
    let mut cursor = work_start;
    for (busy_start, busy_end) in &occupied {
        if cursor + slot_len <= *busy_start {
            free.push(serde_json::json!({
                "start": minutes_to_time(cursor),
                "end": minutes_to_time(*busy_start),
                "duration_minutes": busy_start - cursor
            }));
        }
        if *busy_end > cursor {
            cursor = *busy_end;
        }
    }
    if cursor + slot_len <= work_end {
        free.push(serde_json::json!({
            "start": minutes_to_time(cursor),
            "end": minutes_to_time(work_end),
            "duration_minutes": work_end - cursor
        }));
    }

    serde_json::json!({
        "date": date,
        "work_hours": "09:00–18:00",
        "free_slots": free,
        "busy_count": occupied.len()
    })
}


fn time_to_minutes(hhmm: &str) -> Option<i32> {
    let mut parts = hhmm.splitn(2, ':');
    let h: i32 = parts.next()?.parse().ok()?;
    let m: i32 = parts.next()?.parse().ok()?;
    Some(h * 60 + m)
}

fn minutes_to_time(mins: i32) -> String {
    format!("{:02}:{:02}", mins / 60, mins % 60)
}
