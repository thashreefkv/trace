//! Brain templates: pre-canned graph queries that the Ask tool surface and
//! the Brain UI use to surface "what should I focus on right now?".
//!
//! Each template (`FocusToday`, `BlockedWork`, `EmailFollowups`, `StaleWork`,
//! `StakeholderContext`) reads the brain projection via
//! `super::legacy::get_brain_graph`, finds seed nodes that match the template's
//! intent, expands a small neighborhood, applies the learned ranking from the
//! RL bandit (`super::legacy::apply_learned_ranking`), and returns a
//! `BrainTemplateResult` with rows + the Cypher snippet the user could run
//! themselves.
//!
//! `payload_string` / `payload_bool` are `pub(super)` because the RL feature
//! extractor in `super::legacy::brain_rl_features` reads the same JSON payload
//! fields with the same conventions.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use chrono::{Datelike, Duration, Utc};
use serde_json::json;
use sqlx::SqlitePool;

use crate::models::{
    BrainGraphFilters, BrainTemplateInput, BrainTemplateKind, BrainTemplateResult, WorkGraph,
    WorkGraphNode,
};

use super::legacy::{expand_neighborhood, get_brain_graph};
use super::retrieval::graph_ai_context;
use super::rl::apply_learned_ranking;

pub async fn run_brain_template(
    pool: &SqlitePool,
    path: &Path,
    input: BrainTemplateInput,
) -> Result<BrainTemplateResult, String> {
    let limit = input.limit.unwrap_or(40).clamp(8, 120);
    let graph = get_brain_graph(pool, path, BrainGraphFilters::default()).await?;
    let mut result = match input.template {
        BrainTemplateKind::FocusToday => {
            build_focus_today_template(&graph, limit, input.focus_entity_id.as_deref())
        }
        BrainTemplateKind::BlockedWork => build_blocked_work_template(&graph, limit),
        BrainTemplateKind::EmailFollowups => build_email_followups_template(&graph, limit),
        BrainTemplateKind::StaleWork => build_stale_work_template(&graph, limit),
        BrainTemplateKind::StakeholderContext => {
            build_stakeholder_context_template(&graph, limit, input.focus_entity_id.as_deref())
        }
    };
    apply_learned_ranking(pool, &mut result).await?;
    Ok(result)
}

pub async fn tool_run_brain_template(
    pool: &SqlitePool,
    path: &Path,
    input: BrainTemplateInput,
) -> serde_json::Value {
    match run_brain_template(pool, path, input).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}
fn build_focus_today_template(
    graph: &WorkGraph,
    limit: usize,
    focus_entity_id: Option<&str>,
) -> BrainTemplateResult {
    let today = Utc::now().date_naive();
    let week_start = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let day_index = today.weekday().num_days_from_monday() as i64;
    let today_string = today.to_string();
    let mut seeds = BTreeSet::new();
    let mut rows = Vec::new();

    for node in &graph.nodes {
        if focus_entity_id
            .map(|focus| node.id == focus || node.entity_id == focus)
            .unwrap_or(false)
        {
            seeds.insert(node.id.clone());
            rows.push(template_row(node, "explicit focus"));
        }

        if node.kind == "calendar_event"
            && payload_string(node, "start_date") == Some(today_string.clone())
        {
            seeds.insert(node.id.clone());
            rows.push(template_row(node, "today calendar"));
        }
        if node.kind == "week_day"
            && payload_string(node, "week_start") == Some(week_start.to_string())
            && payload_i64(node, "day_index") == Some(day_index)
        {
            seeds.insert(node.id.clone());
            rows.push(template_row(node, "today week plan"));
        }
        if node.kind == "attention_signal"
            && matches!(
                node.status.as_deref(),
                Some("current_focus")
                    | Some("blocked")
                    | Some("task_due")
                    | Some("email_followup_due")
                    | Some("unread_email")
            )
        {
            seeds.insert(node.id.clone());
            rows.push(template_row(node, "attention signal"));
        }
    }

    for edge in &graph.edges {
        if edge.kind == "HAS_ATTENTION" && seeds.contains(&edge.target) {
            seeds.insert(edge.source.clone());
        }
        if edge.kind == "SCHEDULED_FOR" && seeds.contains(&edge.source) {
            seeds.insert(edge.target.clone());
        }
        if edge.kind == "SCHEDULED_FOR" && seeds.contains(&edge.target) {
            seeds.insert(edge.source.clone());
        }
    }

    let subgraph = subgraph_from_seed_ids(graph, seeds, 3, limit);
    let summary = format!(
        "Focus today uses week-plan, calendar, attention, open-loop, and related work paths; {} node(s), {} relation(s).",
        subgraph.nodes.len(),
        subgraph.edges.len()
    );
    BrainTemplateResult {
        template: BrainTemplateKind::FocusToday.as_str().to_string(),
        summary,
        cypher: focus_today_cypher(),
        rows: compact_template_rows(rows, &subgraph, limit),
        graph: subgraph,
    }
}

fn build_blocked_work_template(graph: &WorkGraph, limit: usize) -> BrainTemplateResult {
    let mut seeds = BTreeSet::new();
    let mut rows = Vec::new();
    for node in &graph.nodes {
        if node.kind == "blocker"
            || (node.kind == "open_loop" && node.status.as_deref() == Some("blocked"))
            || (node.kind == "attention_signal" && node.status.as_deref() == Some("blocked"))
        {
            seeds.insert(node.id.clone());
            rows.push(template_row(node, "blocked signal"));
        }
    }
    for edge in &graph.edges {
        if edge.kind == "BLOCKED_BY" || edge.kind == "WAITING_ON" {
            seeds.insert(edge.source.clone());
            seeds.insert(edge.target.clone());
        }
    }
    let subgraph = subgraph_from_seed_ids(graph, seeds, 4, limit);
    BrainTemplateResult {
        template: BrainTemplateKind::BlockedWork.as_str().to_string(),
        summary: format!(
            "Blocked work follows deliverable -> blocker/open loop -> stakeholder/email/meeting paths; {} node(s), {} relation(s).",
            subgraph.nodes.len(),
            subgraph.edges.len()
        ),
        cypher: blocked_work_cypher(),
        rows: compact_template_rows(rows, &subgraph, limit),
        graph: subgraph,
    }
}

fn build_email_followups_template(graph: &WorkGraph, limit: usize) -> BrainTemplateResult {
    let mut seeds = BTreeSet::new();
    let mut rows = Vec::new();
    for node in &graph.nodes {
        if node.kind == "email_followup"
            && matches!(node.status.as_deref(), Some("open") | Some("overdue"))
        {
            seeds.insert(node.id.clone());
            rows.push(template_row(node, "open email follow-up"));
        }
        if node.kind == "attention_signal" && node.status.as_deref() == Some("email_followup_due") {
            seeds.insert(node.id.clone());
            rows.push(template_row(node, "follow-up attention"));
        }
    }
    for edge in &graph.edges {
        if edge.kind == "HAS_FOLLOWUP" && seeds.contains(&edge.target) {
            seeds.insert(edge.source.clone());
        }
    }
    let subgraph = subgraph_from_seed_ids(graph, seeds, 4, limit);
    BrainTemplateResult {
        template: BrainTemplateKind::EmailFollowups.as_str().to_string(),
        summary: format!(
            "Email follow-ups use email thread -> follow-up -> open-loop plus deliverable/initiative/stakeholder neighborhoods; {} node(s), {} relation(s).",
            subgraph.nodes.len(),
            subgraph.edges.len()
        ),
        cypher: email_followups_cypher(),
        rows: compact_template_rows(rows, &subgraph, limit),
        graph: subgraph,
    }
}

fn build_stale_work_template(graph: &WorkGraph, limit: usize) -> BrainTemplateResult {
    let mut seeds = BTreeSet::new();
    let mut rows = Vec::new();
    for node in &graph.nodes {
        if node.kind == "attention_signal" && node.status.as_deref() == Some("stale_work") {
            seeds.insert(node.id.clone());
            rows.push(template_row(node, "stale important work"));
        }
    }
    for edge in &graph.edges {
        if edge.kind == "HAS_ATTENTION" && seeds.contains(&edge.target) {
            seeds.insert(edge.source.clone());
        }
    }
    let subgraph = subgraph_from_seed_ids(graph, seeds, 3, limit);
    BrainTemplateResult {
        template: BrainTemplateKind::StaleWork.as_str().to_string(),
        summary: format!(
            "Stale work follows important deliverables through attention and open task paths; {} node(s), {} relation(s).",
            subgraph.nodes.len(),
            subgraph.edges.len()
        ),
        cypher: stale_work_cypher(),
        rows: compact_template_rows(rows, &subgraph, limit),
        graph: subgraph,
    }
}

fn build_stakeholder_context_template(
    graph: &WorkGraph,
    limit: usize,
    focus_entity_id: Option<&str>,
) -> BrainTemplateResult {
    let mut seeds = BTreeSet::new();
    let focus_lower = focus_entity_id.map(|value| value.to_ascii_lowercase());
    for node in &graph.nodes {
        if matches!(node.kind.as_str(), "stakeholder" | "email_participant") {
            let is_focus = focus_entity_id
                .map(|focus| node.id == focus || node.entity_id == focus)
                .unwrap_or(false)
                || focus_lower
                    .as_ref()
                    .map(|focus| node.label.to_ascii_lowercase().contains(focus))
                    .unwrap_or(false);
            if is_focus || focus_entity_id.is_none() {
                seeds.insert(node.id.clone());
                if focus_entity_id.is_some() {
                    break;
                }
                if seeds.len() >= 5 {
                    break;
                }
            }
        }
    }
    let subgraph = subgraph_from_seed_ids(graph, seeds, 4, limit);
    let rows = subgraph
        .nodes
        .iter()
        .filter(|node| {
            matches!(
                node.kind.as_str(),
                "stakeholder"
                    | "deliverable"
                    | "email_thread"
                    | "meeting"
                    | "file"
                    | "trace_folder"
                    | "open_loop"
                    | "attention_signal"
            )
        })
        .map(|node| template_row(node, "stakeholder neighborhood"))
        .take(limit)
        .collect::<Vec<_>>();
    BrainTemplateResult {
        template: BrainTemplateKind::StakeholderContext.as_str().to_string(),
        summary: format!(
            "Stakeholder context expands stakeholder/email identity into deliverables, emails, meetings, files, and open loops; {} node(s), {} relation(s).",
            subgraph.nodes.len(),
            subgraph.edges.len()
        ),
        cypher: stakeholder_context_cypher(),
        rows,
        graph: subgraph,
    }
}

fn subgraph_from_seed_ids(
    graph: &WorkGraph,
    seed_ids: BTreeSet<String>,
    max_hops: usize,
    limit: usize,
) -> WorkGraph {
    let selected_ids = expand_neighborhood(&seed_ids, &graph.edges, max_hops, limit);
    let nodes = graph
        .nodes
        .iter()
        .filter(|node| selected_ids.contains(&node.id))
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let node_ids = nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let edges = graph
        .edges
        .iter()
        .filter(|edge| node_ids.contains(&edge.source) && node_ids.contains(&edge.target))
        .cloned()
        .collect::<Vec<_>>();
    WorkGraph {
        generated_at: graph.generated_at.clone(),
        ai_context: graph_ai_context(&nodes, &edges),
        nodes,
        edges,
    }
}

fn compact_template_rows(
    mut rows: Vec<serde_json::Value>,
    subgraph: &WorkGraph,
    limit: usize,
) -> Vec<serde_json::Value> {
    let mut seen = rows
        .iter()
        .filter_map(|row| {
            row.get("id")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect::<HashSet<_>>();
    for node in &subgraph.nodes {
        if seen.insert(node.id.clone()) {
            rows.push(template_row(node, "related by graph path"));
        }
        if rows.len() >= limit {
            break;
        }
    }
    rows.truncate(limit);
    rows
}
fn template_row(node: &WorkGraphNode, reason: &str) -> serde_json::Value {
    json!({
        "id": node.id,
        "entity_id": node.entity_id,
        "kind": node.kind,
        "title": node.label,
        "status": node.status,
        "summary": node.subtitle,
        "url": node.url,
        "weight": node.weight,
        "reason": reason,
    })
}

pub(super) fn payload_string(node: &WorkGraphNode, key: &str) -> Option<String> {
    node.properties
        .get("payload")
        .and_then(|payload| payload.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

pub(super) fn payload_bool(node: &WorkGraphNode, key: &str) -> bool {
    node.properties
        .get("payload")
        .and_then(|payload| payload.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn payload_i64(node: &WorkGraphNode, key: &str) -> Option<i64> {
    node.properties
        .get("payload")
        .and_then(|payload| payload.get(key))
        .and_then(|value| value.as_i64())
}

fn focus_today_cypher() -> String {
    r#"
MATCH (seed:Entity)-[path:Related*1..3]-(n:Entity)
WHERE seed.kind IN ['week_day','calendar_event','attention_signal']
  AND seed.status IN ['scheduled','current_focus','blocked','task_due','email_followup_due','unread_email','']
RETURN seed.kind AS seed_kind, seed.title AS seed, n.kind AS kind, n.title AS item, n.status AS status, path
ORDER BY n.importance DESC
"#
    .trim()
    .to_string()
}

fn blocked_work_cypher() -> String {
    r#"
MATCH (d:Entity)-[:Related {kind:'BLOCKED_BY'}]->(b:Entity)
OPTIONAL MATCH (b)-[:Related*1..3]-(ctx:Entity)
WHERE ctx.kind IN ['stakeholder','email_participant','email_thread','meeting','meeting_action','open_loop','attention_signal','file']
RETURN d.title AS deliverable, b.summary AS blocker, collect(ctx.title) AS context
ORDER BY d.importance DESC
"#
    .trim()
    .to_string()
}

fn email_followups_cypher() -> String {
    r#"
MATCH (thread:Entity)-[:Related {kind:'HAS_FOLLOWUP'}]->(followup:Entity)
WHERE followup.kind = 'email_followup' AND followup.status IN ['open','overdue']
OPTIONAL MATCH (thread)-[:Related*1..3]-(ctx:Entity)
WHERE ctx.kind IN ['deliverable','initiative','stakeholder','email_participant','open_loop','attention_signal']
RETURN followup.title AS followup, followup.status AS status, thread.title AS thread, collect(ctx.title) AS context
ORDER BY followup.importance DESC
"#
    .trim()
    .to_string()
}

fn stale_work_cypher() -> String {
    r#"
MATCH (work:Entity)-[:Related {kind:'HAS_ATTENTION'}]->(signal:Entity)
WHERE signal.kind = 'attention_signal' AND signal.status = 'stale_work'
OPTIONAL MATCH (work)-[:Related*1..3]-(ctx:Entity)
WHERE ctx.kind IN ['task','open_loop','email_thread','meeting','stakeholder','calendar_event']
RETURN work.title AS work, work.status AS state, signal.summary AS reason, collect(ctx.title) AS context
ORDER BY work.importance DESC
"#
    .trim()
    .to_string()
}

fn stakeholder_context_cypher() -> String {
    r#"
MATCH (s:Entity)-[:Related*0..4]-(ctx:Entity)
WHERE s.kind IN ['stakeholder','email_participant']
  AND ctx.kind IN ['deliverable','email_thread','meeting','file','trace_folder','open_loop','attention_signal','calendar_event']
RETURN s.title AS stakeholder, ctx.kind AS kind, ctx.title AS item, ctx.status AS status
ORDER BY ctx.importance DESC
"#
    .trim()
    .to_string()
}
