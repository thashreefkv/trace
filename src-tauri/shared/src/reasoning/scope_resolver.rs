use std::collections::HashSet;

use sqlx::SqlitePool;

use crate::db::sql_error;
use crate::models::ReasoningScope;

use super::sources::content_hash;

/// Result of expanding a [`ReasoningScope`] into the explicit set of entities
/// that count as "in scope" for a strict report. Source-kind/source-id pairs in
/// [`Self::source_unit_membership`] are the only ones a strict report may cite.
///
/// Captures, ask_turns, memories, and conversations are intentionally **not**
/// surfaced — they're process artifacts, not work.
#[derive(Debug, Clone, Default)]
pub struct ResolvedScope {
    pub initiative_ids: Vec<String>,
    pub deliverable_ids: Vec<String>,
    pub stakeholder_ids: Vec<String>,
    pub email_thread_ids: Vec<String>,
    pub email_message_ids: Vec<String>,
    pub meeting_ids: Vec<String>,
    pub file_ids: Vec<String>,
    pub calendar_event_ids: Vec<String>,
    pub initiative_titles: Vec<String>,
    /// Strict allow-list: only source units whose (kind, id) appears here are
    /// considered when the scope is targeted.
    pub source_unit_membership: HashSet<(String, String)>,
    /// Source-unit ids the user explicitly excluded for this report run.
    pub excluded_source_unit_ids: HashSet<String>,
    /// True when scope names at least one initiative/stakeholder/deliverable.
    /// When false the caller falls back to the previous (untargeted) behaviour.
    pub is_targeted: bool,
}

/// Walk the relationship graph rooted at the scope's initiative_ids /
/// stakeholder_ids / deliverable_ids and return the explicit allow-list of
/// entities (and their source-unit pairs) that belong in a strict report.
pub async fn resolve_scope(
    pool: &SqlitePool,
    scope: &ReasoningScope,
    exclusions: &[String],
) -> Result<ResolvedScope, String> {
    let mut resolved = ResolvedScope {
        excluded_source_unit_ids: exclusions.iter().cloned().collect(),
        ..ResolvedScope::default()
    };

    let is_targeted = !scope.initiative_ids.is_empty()
        || !scope.stakeholder_ids.is_empty()
        || !scope.deliverable_ids.is_empty();
    resolved.is_targeted = is_targeted;
    if !is_targeted {
        return Ok(resolved);
    }

    let initiative_ids: HashSet<String> = scope.initiative_ids.iter().cloned().collect();
    let mut deliverable_ids: HashSet<String> = scope.deliverable_ids.iter().cloned().collect();
    let mut stakeholder_ids: HashSet<String> = scope.stakeholder_ids.iter().cloned().collect();

    // initiative -> deliverables
    if !initiative_ids.is_empty() {
        let ids: Vec<String> = initiative_ids.iter().cloned().collect();
        for row in fetch_ids(
            pool,
            "SELECT deliverable_id FROM deliverable_initiatives WHERE initiative_id",
            &ids,
        )
        .await?
        {
            deliverable_ids.insert(row);
        }
    }

    // stakeholder -> deliverables
    if !stakeholder_ids.is_empty() {
        let ids: Vec<String> = stakeholder_ids.iter().cloned().collect();
        for row in fetch_ids(
            pool,
            "SELECT deliverable_id FROM deliverable_stakeholders WHERE stakeholder_id",
            &ids,
        )
        .await?
        {
            deliverable_ids.insert(row);
        }
    }

    // deliverables -> stakeholders (closure step)
    if !deliverable_ids.is_empty() {
        let ids: Vec<String> = deliverable_ids.iter().cloned().collect();
        for row in fetch_ids(
            pool,
            "SELECT stakeholder_id FROM deliverable_stakeholders WHERE deliverable_id",
            &ids,
        )
        .await?
        {
            stakeholder_ids.insert(row);
        }
    }

    // initiatives -> email threads (explicit junction)
    let mut email_thread_ids: HashSet<String> = HashSet::new();
    if !initiative_ids.is_empty() {
        let ids: Vec<String> = initiative_ids.iter().cloned().collect();
        for row in fetch_ids(
            pool,
            "SELECT thread_id FROM gmail_thread_initiatives WHERE initiative_id",
            &ids,
        )
        .await?
        {
            email_thread_ids.insert(row);
        }
    }

    // email threads -> individual messages (so message-level source units pass too)
    let mut email_message_ids: HashSet<String> = HashSet::new();
    if !email_thread_ids.is_empty() {
        let ids: Vec<String> = email_thread_ids.iter().cloned().collect();
        for row in fetch_ids(
            pool,
            "SELECT message_id FROM gmail_messages WHERE thread_id",
            &ids,
        )
        .await?
        {
            email_message_ids.insert(row);
        }
    }

    // stakeholders -> meetings (only direct relationship that exists today)
    let mut meeting_ids: HashSet<String> = HashSet::new();
    if !stakeholder_ids.is_empty() {
        let ids: Vec<String> = stakeholder_ids.iter().cloned().collect();
        for row in fetch_ids(
            pool,
            "SELECT meeting_id FROM meeting_stakeholders WHERE stakeholder_id",
            &ids,
        )
        .await?
        {
            meeting_ids.insert(row);
        }
    }

    // initiatives -> files (added in migration 0058; gated to avoid breaking
    // older databases that haven't migrated yet).
    let mut file_ids: HashSet<String> = HashSet::new();
    if !initiative_ids.is_empty() && table_exists(pool, "file_initiatives").await {
        let ids: Vec<String> = initiative_ids.iter().cloned().collect();
        for row in fetch_ids(
            pool,
            "SELECT file_id FROM file_initiatives WHERE initiative_id",
            &ids,
        )
        .await?
        {
            file_ids.insert(row);
        }
    }

    // initiatives -> calendar events (added in migration 0058)
    let mut event_ids: HashSet<String> = HashSet::new();
    if !initiative_ids.is_empty() && table_exists(pool, "gcal_event_initiatives").await {
        let ids: Vec<String> = initiative_ids.iter().cloned().collect();
        for row in fetch_ids(
            pool,
            "SELECT event_id FROM gcal_event_initiatives WHERE initiative_id",
            &ids,
        )
        .await?
        {
            event_ids.insert(row);
        }
    }

    let initiative_titles = if initiative_ids.is_empty() {
        Vec::new()
    } else {
        let ids: Vec<String> = initiative_ids.iter().cloned().collect();
        fetch_ids(pool, "SELECT title FROM initiatives WHERE id", &ids).await?
    };

    // Build strict allow-list. Each entity maps to its source-unit kind+id.
    let mut membership: HashSet<(String, String)> = HashSet::new();
    for id in &initiative_ids {
        membership.insert(("initiative".to_string(), id.clone()));
    }
    for id in &deliverable_ids {
        membership.insert(("deliverable".to_string(), id.clone()));
    }
    for id in &stakeholder_ids {
        membership.insert(("stakeholder".to_string(), id.clone()));
    }
    for id in &email_thread_ids {
        membership.insert(("email_thread".to_string(), id.clone()));
    }
    for id in &email_message_ids {
        membership.insert(("email_message".to_string(), id.clone()));
    }
    for id in &meeting_ids {
        membership.insert(("meeting".to_string(), id.clone()));
    }
    for id in &file_ids {
        membership.insert(("file".to_string(), id.clone()));
    }
    for id in &event_ids {
        membership.insert(("calendar_event".to_string(), id.clone()));
    }

    resolved.initiative_ids = initiative_ids.into_iter().collect();
    resolved.deliverable_ids = deliverable_ids.into_iter().collect();
    resolved.stakeholder_ids = stakeholder_ids.into_iter().collect();
    resolved.email_thread_ids = email_thread_ids.into_iter().collect();
    resolved.email_message_ids = email_message_ids.into_iter().collect();
    resolved.meeting_ids = meeting_ids.into_iter().collect();
    resolved.file_ids = file_ids.into_iter().collect();
    resolved.calendar_event_ids = event_ids.into_iter().collect();
    resolved.initiative_titles = initiative_titles;
    resolved.source_unit_membership = membership;

    Ok(resolved)
}

/// Stable fingerprint of a resolved scope, suitable for inclusion in cache keys.
/// Changes whenever the membership set or the user's exclusion list changes,
/// so a re-run after an edit doesn't return a stale cached synthesis.
pub fn scope_hash(resolved: &ResolvedScope) -> String {
    let mut pairs: Vec<String> = resolved
        .source_unit_membership
        .iter()
        .map(|(kind, id)| format!("{kind}:{id}"))
        .collect();
    pairs.sort();
    let mut excl: Vec<String> = resolved.excluded_source_unit_ids.iter().cloned().collect();
    excl.sort();
    pairs.push(format!("excl={}", excl.join(",")));
    content_hash(&pairs.join("|"))
}

async fn table_exists(pool: &SqlitePool, name: &str) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap_or(0)
        > 0
}

async fn fetch_ids(
    pool: &SqlitePool,
    sql_prefix: &str,
    ids: &[String],
) -> Result<Vec<String>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("{sql_prefix} IN ({placeholders})");
    let mut query = sqlx::query_scalar::<_, String>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    query.fetch_all(pool).await.map_err(sql_error)
}
