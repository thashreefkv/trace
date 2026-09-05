use std::{collections::BTreeSet, path::Path};

use chrono::{NaiveDate, Utc};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use ulid::Ulid;

use super::models::*;
use super::{
    auto_analyze_relevant_threads, classify_url, email_domain, format_ts, is_blocked_spam_or_trash,
    linked_deliverables_for_thread, linked_initiatives_for_thread, linked_stakeholders_for_thread,
    links_for_thread_urls, list_local_threads, load_message_rows, load_relevance_context,
    local_message_is_low_signal, now_utc, refresh_work_mail_dimensions, strip_html, to_json_string,
};

pub async fn auto_link_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<GmailAutoLinkReport, String> {
    let messages = load_message_rows(pool, thread_id).await?;
    let haystack = thread_link_haystack(&messages);
    let artifact_urls = links_for_thread_urls(pool, thread_id)
        .await
        .unwrap_or_default();
    let participant_emails = participant_emails_for_thread(pool, thread_id).await?;
    let participant_domains = participant_emails
        .iter()
        .filter_map(|email| email_domain(email))
        .collect::<BTreeSet<_>>();

    let mut report = GmailAutoLinkReport {
        thread_id: thread_id.to_string(),
        linked_stakeholders: 0,
        linked_deliverables: 0,
        linked_initiatives: 0,
        suggestions_created: 0,
        orphan: false,
    };

    let now = now_utc();
    let exact_stakeholders: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT s.id, s.name, s.email
        FROM stakeholders s
        INNER JOIN gmail_thread_participants tp ON lower(tp.email) = lower(s.email)
        WHERE tp.thread_id = ?
          AND s.email != ''
          AND NOT EXISTS (
            SELECT 1 FROM gmail_thread_stakeholder_excludes ex
            WHERE ex.thread_id = tp.thread_id AND ex.stakeholder_id = s.id
          )
        "#,
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    for (stakeholder_id, name, email) in exact_stakeholders {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO gmail_thread_stakeholders (
              thread_id, stakeholder_id, linked_at, source, confidence, rationale
            )
            VALUES (?, ?, ?, 'auto', 0.98, ?)
            "#,
        )
        .bind(thread_id)
        .bind(&stakeholder_id)
        .bind(&now)
        .bind(format!(
            "Participant email {} exactly matches stakeholder {}.",
            email, name
        ))
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
        report.linked_stakeholders += result.rows_affected() as i64;
    }

    let stakeholder_candidates: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, name, email, role FROM stakeholders WHERE name != '' OR email != ''",
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    for (stakeholder_id, name, email, _role) in stakeholder_candidates {
        if email.trim().is_empty()
            || participant_emails
                .iter()
                .any(|participant| participant.eq_ignore_ascii_case(&email))
        {
            continue;
        }
        let mut score: f64 = 0.0;
        let mut reasons = Vec::new();
        if let Some(domain) = email_domain(&email) {
            if participant_domains.contains(&domain) && !is_public_email_domain(&domain) {
                score += 0.42;
                reasons.push(format!(
                    "Participant domain matches stakeholder domain {domain}."
                ));
            }
        }
        let name_tokens = meaningful_tokens(&name);
        if !name_tokens.is_empty()
            && name_tokens
                .iter()
                .filter(|token| haystack.contains(token.as_str()))
                .count()
                >= name_tokens.len().min(2)
        {
            score += 0.28;
            reasons.push("Stakeholder name appears in the thread text.".to_string());
        }
        if (0.55..0.98).contains(&score) {
            if upsert_link_suggestion(
                pool,
                thread_id,
                "stakeholder",
                &stakeholder_id,
                &name,
                score,
                &reasons.join(" "),
            )
            .await?
            {
                report.suggestions_created += 1;
            }
        }
    }

    let deliverables: Vec<(
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
    )> = sqlx::query_as(
        r#"
        SELECT d.id,
               d.title,
               d.claim,
               d.state,
               d.deadline,
               d.priority,
               d.blocker_reason,
               d.artifact_url,
               COALESCE(group_concat(DISTINCT lower(s.email)), '') AS stakeholder_emails
        FROM deliverables d
        LEFT JOIN deliverable_stakeholders ds ON ds.deliverable_id = d.id
        LEFT JOIN stakeholders s
          ON s.id = ds.stakeholder_id OR s.id = d.stakeholder_id
        WHERE d.state NOT IN ('shipped', 'killed')
        GROUP BY d.id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    for (
        deliverable_id,
        title,
        claim,
        _state,
        deadline,
        priority,
        blocker_reason,
        artifact_url,
        stakeholder_emails,
    ) in deliverables
    {
        let (score, reasons) = score_deliverable_link(
            &haystack,
            &artifact_urls,
            &participant_emails,
            &title,
            &claim,
            artifact_url.as_deref(),
            &stakeholder_emails,
            deadline.as_deref(),
            priority.as_deref(),
            blocker_reason.as_deref(),
        );
        if score >= 0.82 {
            let result = sqlx::query(
                r#"
                INSERT OR IGNORE INTO gmail_thread_deliverables (
                  thread_id, deliverable_id, linked_at, source, confidence, rationale
                )
                VALUES (?, ?, ?, 'auto', ?, ?)
                "#,
            )
            .bind(thread_id)
            .bind(&deliverable_id)
            .bind(&now)
            .bind(score)
            .bind(reasons.join(" "))
            .execute(pool)
            .await
            .map_err(crate::db::sql_error)?;
            report.linked_deliverables += result.rows_affected() as i64;
        } else if score >= 0.55
            && upsert_link_suggestion(
                pool,
                thread_id,
                "deliverable",
                &deliverable_id,
                &title,
                score,
                &reasons.join(" "),
            )
            .await?
        {
            report.suggestions_created += 1;
        }
    }

    let initiatives: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, title, framing, status FROM initiatives WHERE status IN ('live', 'paused')",
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    for (initiative_id, title, framing, _status) in initiatives {
        let (score, reasons) = score_initiative_link(&haystack, &title, &framing);
        if score >= 0.85 {
            let result = sqlx::query(
                r#"
                INSERT OR IGNORE INTO gmail_thread_initiatives (
                  thread_id, initiative_id, linked_at, source, confidence, rationale
                )
                VALUES (?, ?, ?, 'auto', ?, ?)
                "#,
            )
            .bind(thread_id)
            .bind(&initiative_id)
            .bind(&now)
            .bind(score)
            .bind(reasons.join(" "))
            .execute(pool)
            .await
            .map_err(crate::db::sql_error)?;
            report.linked_initiatives += result.rows_affected() as i64;
        } else if score >= 0.6
            && upsert_link_suggestion(
                pool,
                thread_id,
                "initiative",
                &initiative_id,
                &title,
                score,
                &reasons.join(" "),
            )
            .await?
        {
            report.suggestions_created += 1;
        }
    }

    refresh_thread_intelligence(pool, thread_id).await?;
    report.orphan = thread_is_orphan(pool, thread_id).await?;
    Ok(report)
}

pub async fn backfill_stakeholder_thread_links(
    pool: &SqlitePool,
    stakeholder_id: &str,
    stakeholder_email: &str,
) -> Result<i64, String> {
    if stakeholder_email.trim().is_empty() {
        return Ok(0);
    }
    let now = now_utc();
    let thread_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT tp.thread_id
        FROM gmail_thread_participants tp
        WHERE lower(tp.email) = lower(?)
          AND NOT EXISTS (
            SELECT 1 FROM gmail_thread_stakeholder_excludes ex
            WHERE ex.thread_id = tp.thread_id AND ex.stakeholder_id = ?
          )
        "#,
    )
    .bind(stakeholder_email)
    .bind(stakeholder_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;

    let mut linked = 0i64;
    for thread_id in thread_ids {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO gmail_thread_stakeholders (
              thread_id, stakeholder_id, linked_at, source, confidence, rationale
            )
            VALUES (?, ?, ?, 'auto', 0.98, 'Participant email exactly matches newly created stakeholder.')
            "#,
        )
        .bind(&thread_id)
        .bind(stakeholder_id)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
        linked += result.rows_affected() as i64;
        let _ = refresh_thread_intelligence(pool, &thread_id).await;
    }
    Ok(linked)
}

pub async fn list_orphan_threads(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<GmailLocalThread>, String> {
    list_local_threads(
        pool,
        GmailThreadFilter {
            query: None,
            label_id: None,
            category: None,
            stakeholder_id: None,
            deliverable_id: None,
            initiative_id: None,
            orphan_only: Some(true),
            limit: Some(limit),
        },
    )
    .await
}

pub async fn reanalyze_stale_threads(
    dir: &Path,
    pool: &SqlitePool,
    limit: i64,
) -> Result<GmailSyncReport, String> {
    let started_at = now_utc();
    let report = auto_analyze_relevant_threads(dir, pool, limit, None).await;
    let completed_at = now_utc();
    Ok(GmailSyncReport {
        synced_threads: 0,
        synced_messages: 0,
        backfilled_threads: 0,
        backfill_complete: false,
        skipped_spam_threads: 0,
        skipped_irrelevant_threads: 0,
        purged_threads: 0,
        ai_analyzed_threads: report.analyzed,
        auto_linked_threads: 0,
        analysis_refreshed_threads: report.refreshed,
        analysis_failed_threads: report.failed,
        orphan_threads: count_orphan_threads(pool).await.unwrap_or(0),
        new_messages: 0,
        new_threads: 0,
        synced_labels: 0,
        synced_drafts: 0,
        started_at,
        completed_at,
        account_email: None,
    })
}

async fn upsert_link_suggestion(
    pool: &SqlitePool,
    thread_id: &str,
    target_kind: &str,
    target_id: &str,
    target_title: &str,
    confidence: f64,
    rationale: &str,
) -> Result<bool, String> {
    let now = now_utc();
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO gmail_thread_link_suggestions (
          id, thread_id, target_kind, target_id, target_title, confidence,
          rationale, status, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)
        "#,
    )
    .bind(Ulid::new().to_string())
    .bind(thread_id)
    .bind(target_kind)
    .bind(target_id)
    .bind(target_title)
    .bind(confidence)
    .bind(rationale)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    if result.rows_affected() == 0 {
        sqlx::query(
            r#"
            UPDATE gmail_thread_link_suggestions
            SET confidence = CASE WHEN confidence < ? THEN ? ELSE confidence END,
                rationale = CASE WHEN confidence < ? THEN ? ELSE rationale END,
                target_title = ?,
                updated_at = ?
            WHERE thread_id = ?
              AND target_kind = ?
              AND target_id = ?
              AND status = 'pending'
            "#,
        )
        .bind(confidence)
        .bind(confidence)
        .bind(confidence)
        .bind(rationale)
        .bind(target_title)
        .bind(&now)
        .bind(thread_id)
        .bind(target_kind)
        .bind(target_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    }
    Ok(result.rows_affected() > 0)
}

pub async fn list_thread_link_suggestions(
    pool: &SqlitePool,
    thread_id: Option<&str>,
    limit: i64,
) -> Result<Vec<GmailThreadLinkSuggestion>, String> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        f64,
        String,
        String,
        String,
        String,
    )> = sqlx::query_as(
        r#"
            SELECT id, thread_id, target_kind, target_id, target_title,
                   confidence, rationale, status, created_at, updated_at
            FROM gmail_thread_link_suggestions
            WHERE (? IS NULL OR thread_id = ?)
              AND status = 'pending'
            ORDER BY confidence DESC, created_at DESC
            LIMIT ?
            "#,
    )
    .bind(thread_id)
    .bind(thread_id)
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                thread_id,
                target_kind,
                target_id,
                target_title,
                confidence,
                rationale,
                status,
                created_at,
                updated_at,
            )| GmailThreadLinkSuggestion {
                id,
                thread_id,
                target_kind,
                target_id,
                target_title,
                confidence,
                rationale,
                status,
                created_at,
                updated_at,
            },
        )
        .collect())
}

pub async fn accept_thread_link(pool: &SqlitePool, suggestion_id: &str) -> Result<(), String> {
    let suggestion: Option<(String, String, String, String, String, f64, String)> = sqlx::query_as(
        r#"
        SELECT id, thread_id, target_kind, target_id, target_title, confidence, rationale
        FROM gmail_thread_link_suggestions
        WHERE id = ? AND status = 'pending'
        "#,
    )
    .bind(suggestion_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::db::sql_error)?;
    let Some((id, thread_id, target_kind, target_id, target_title, confidence, rationale)) =
        suggestion
    else {
        return Err("thread link suggestion not found".to_string());
    };

    let now = now_utc();
    match target_kind.as_str() {
        "stakeholder" => {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO gmail_thread_stakeholders (
                  thread_id, stakeholder_id, linked_at, source, confidence, rationale
                )
                VALUES (?, ?, ?, 'accepted', ?, ?)
                "#,
            )
            .bind(&thread_id)
            .bind(&target_id)
            .bind(&now)
            .bind(confidence)
            .bind(&rationale)
            .execute(pool)
            .await
            .map_err(crate::db::sql_error)?;
        }
        "deliverable" => {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO gmail_thread_deliverables (
                  thread_id, deliverable_id, linked_at, source, confidence, rationale
                )
                VALUES (?, ?, ?, 'accepted', ?, ?)
                "#,
            )
            .bind(&thread_id)
            .bind(&target_id)
            .bind(&now)
            .bind(confidence)
            .bind(&rationale)
            .execute(pool)
            .await
            .map_err(crate::db::sql_error)?;
        }
        "initiative" => {
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO gmail_thread_initiatives (
                  thread_id, initiative_id, linked_at, source, confidence, rationale
                )
                VALUES (?, ?, ?, 'accepted', ?, ?)
                "#,
            )
            .bind(&thread_id)
            .bind(&target_id)
            .bind(&now)
            .bind(confidence)
            .bind(&rationale)
            .execute(pool)
            .await
            .map_err(crate::db::sql_error)?;
        }
        _ => return Err(format!("unsupported link target kind: {target_kind}")),
    }
    sqlx::query(
        "UPDATE gmail_thread_link_suggestions SET status = 'accepted', updated_at = ?, resolved_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    refresh_thread_intelligence(pool, &thread_id).await?;

    if target_kind == "deliverable" {
        sqlx::query(
            r#"
            UPDATE work_intake_suggestions
            SET target_deliverable_id = COALESCE(target_deliverable_id, ?),
                updated_at = ?
            WHERE source_kind = 'gmail_thread'
              AND source_id = ?
              AND status = 'pending'
            "#,
        )
        .bind(&target_id)
        .bind(&now)
        .bind(&thread_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    }

    let _ = target_title;
    Ok(())
}

pub async fn reject_thread_link(pool: &SqlitePool, suggestion_id: &str) -> Result<(), String> {
    let now = now_utc();
    let result = sqlx::query(
        "UPDATE gmail_thread_link_suggestions SET status = 'rejected', updated_at = ?, resolved_at = ? WHERE id = ? AND status = 'pending'",
    )
    .bind(&now)
    .bind(&now)
    .bind(suggestion_id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    if result.rows_affected() == 0 {
        return Err("thread link suggestion not found".to_string());
    }
    Ok(())
}

pub async fn refresh_thread_intelligence(pool: &SqlitePool, thread_id: &str) -> Result<(), String> {
    let context = build_graph_context_for_thread(pool, thread_id).await?;
    let rows: Option<(String, String, bool, Option<i64>, i64)> = sqlx::query_as(
        "SELECT ai_category, ai_priority, has_unread, last_message_at, message_count FROM gmail_threads WHERE thread_id = ?",
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::db::sql_error)?;
    let Some((ai_category, ai_priority, has_unread, _last_message_at, _message_count)) = rows
    else {
        return Ok(());
    };
    let (effective_priority, reasons, graph_action_signal) =
        compute_effective_priority(&context, &ai_category, &ai_priority, has_unread);
    let category = if graph_action_signal
        && matches!(
            ai_category.as_str(),
            "other" | "archive" | "newsletter" | "personal"
        ) {
        "action_required"
    } else {
        ai_category.as_str()
    };
    sqlx::query(
        r#"
        UPDATE gmail_threads
        SET graph_context_json = ?,
            effective_priority = ?,
            priority_reasons_json = ?,
            intelligence_updated_at = ?,
            ai_category = ?,
            ai_triaged_at = COALESCE(ai_triaged_at, ?)
        WHERE thread_id = ?
        "#,
    )
    .bind(context.to_string())
    .bind(effective_priority)
    .bind(to_json_string(&reasons))
    .bind(now_utc())
    .bind(category)
    .bind(now_utc())
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    refresh_work_mail_dimensions(pool, thread_id).await?;
    Ok(())
}

pub async fn build_graph_context_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Value, String> {
    let stakeholders = linked_stakeholders_for_thread(pool, thread_id).await?;
    let linked_deliverables = linked_deliverables_for_thread(pool, thread_id).await?;
    let linked_initiatives = linked_initiatives_for_thread(pool, thread_id).await?;
    let active_deliverables: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT DISTINCT d.id, d.title, d.state, d.deadline, d.priority,
               d.blocker_reason, d.artifact_url
        FROM deliverables d
        LEFT JOIN deliverable_stakeholders ds ON ds.deliverable_id = d.id
        LEFT JOIN stakeholders direct ON direct.id = d.stakeholder_id
        WHERE d.state NOT IN ('shipped', 'killed')
          AND (
            EXISTS (
              SELECT 1 FROM gmail_thread_deliverables td
              WHERE td.thread_id = ? AND td.deliverable_id = d.id
            )
            OR d.stakeholder_id IN (
              SELECT stakeholder_id FROM gmail_thread_stakeholders WHERE thread_id = ?
            )
            OR ds.stakeholder_id IN (
              SELECT stakeholder_id FROM gmail_thread_stakeholders WHERE thread_id = ?
            )
            OR lower(direct.email) IN (
              SELECT lower(email) FROM gmail_thread_participants WHERE thread_id = ?
            )
            OR ds.stakeholder_id IN (
              SELECT s.id
              FROM stakeholders s
              INNER JOIN gmail_thread_participants tp ON lower(tp.email) = lower(s.email)
              WHERE tp.thread_id = ?
            )
          )
        ORDER BY d.deadline IS NULL ASC, d.deadline ASC, d.updated_at DESC
        LIMIT 12
        "#,
    )
    .bind(thread_id)
    .bind(thread_id)
    .bind(thread_id)
    .bind(thread_id)
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    let recent_meetings: Vec<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT m.id, m.title, m.date
        FROM meetings m
        INNER JOIN meeting_stakeholders ms ON ms.meeting_id = m.id
        WHERE ms.stakeholder_id IN (
          SELECT stakeholder_id FROM gmail_thread_stakeholders WHERE thread_id = ?
          UNION
          SELECT s.id
          FROM stakeholders s
          INNER JOIN gmail_thread_participants tp ON lower(tp.email) = lower(s.email)
          WHERE tp.thread_id = ?
        )
        ORDER BY m.date DESC
        LIMIT 5
        "#,
    )
    .bind(thread_id)
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    Ok(json!({
        "stakeholders": stakeholders.iter().map(|stakeholder| json!({
            "id": stakeholder.id,
            "name": stakeholder.name,
            "email": stakeholder.email,
            "role": stakeholder.role,
            "source": stakeholder.source,
            "confidence": stakeholder.confidence,
            "rationale": stakeholder.rationale,
        })).collect::<Vec<_>>(),
        "linked_deliverables": linked_deliverables.iter().map(|deliverable| json!({
            "id": deliverable.id,
            "title": deliverable.title,
            "state": deliverable.state,
            "source": deliverable.source,
            "confidence": deliverable.confidence,
            "rationale": deliverable.rationale,
        })).collect::<Vec<_>>(),
        "active_deliverables": active_deliverables.iter().map(
            |(id, title, state, deadline, priority, blocker_reason, artifact_url)| json!({
                "id": id,
                "title": title,
                "state": state,
                "deadline": deadline,
                "priority": priority,
                "blocker_reason": blocker_reason,
                "artifact_url": artifact_url,
            })
        ).collect::<Vec<_>>(),
        "linked_initiatives": linked_initiatives.iter().map(|initiative| json!({
            "id": initiative.id,
            "title": initiative.title,
            "status": initiative.status,
            "source": initiative.source,
            "confidence": initiative.confidence,
            "rationale": initiative.rationale,
        })).collect::<Vec<_>>(),
        "recent_meetings": recent_meetings.iter().map(|(id, title, date)| json!({
            "id": id,
            "title": title,
            "date": date,
        })).collect::<Vec<_>>(),
    }))
}

pub(super) fn compute_effective_priority(
    context: &Value,
    ai_category: &str,
    ai_priority: &str,
    has_unread: bool,
) -> (String, Vec<String>, bool) {
    let mut score = match ai_priority {
        "urgent" => 85,
        "high" => 62,
        "medium" => 38,
        _ => 15,
    };
    let mut reasons = vec![format!("Content priority: {ai_priority}.")];
    if has_unread {
        score += 8;
        reasons.push("Thread has unread mail.".to_string());
    }
    if matches!(ai_category, "action_required" | "meeting_request") {
        score += 12;
        reasons.push("Email category is action-oriented.".to_string());
    }
    let stakeholder_count = context
        .get("stakeholders")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if stakeholder_count > 0 {
        score += 10;
        reasons.push("Thread is linked to a stakeholder.".to_string());
    }

    let mut graph_action_signal = false;
    if let Some(deliverables) = context.get("active_deliverables").and_then(Value::as_array) {
        if !deliverables.is_empty() {
            score += 10;
            graph_action_signal = true;
            reasons.push("Stakeholder has active deliverables.".to_string());
        }
        for deliverable in deliverables {
            let title = deliverable
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("deliverable");
            if let Some(priority) = deliverable.get("priority").and_then(Value::as_str) {
                if is_high_priority_value(priority) {
                    score += 15;
                    graph_action_signal = true;
                    reasons.push(format!("{title} is high priority."));
                }
            }
            if deliverable
                .get("blocker_reason")
                .and_then(Value::as_str)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
            {
                score += 18;
                graph_action_signal = true;
                reasons.push(format!("{title} is blocked."));
            }
            if let Some(deadline) = deliverable.get("deadline").and_then(Value::as_str) {
                if let Some(days) = days_until_deadline(deadline) {
                    if days <= 2 {
                        score += 30;
                        graph_action_signal = true;
                        reasons.push(format!("{title} is due within 48 hours."));
                    } else if days <= 7 {
                        score += 15;
                        graph_action_signal = true;
                        reasons.push(format!("{title} is due within a week."));
                    }
                }
            }
        }
    }

    (
        priority_label_from_score(score),
        dedupe_strings(reasons),
        graph_action_signal,
    )
}

pub(super) fn score_deliverable_link(
    haystack: &str,
    artifact_urls: &[String],
    participant_emails: &[String],
    title: &str,
    claim: &str,
    artifact_url: Option<&str>,
    stakeholder_emails: &str,
    deadline: Option<&str>,
    priority: Option<&str>,
    blocker_reason: Option<&str>,
) -> (f64, Vec<String>) {
    let mut score: f64 = 0.0;
    let mut reasons = Vec::new();
    let title_lower = title.trim().to_lowercase();
    if title_lower.len() >= 6 && haystack.contains(&title_lower) {
        score += 0.65;
        reasons.push("Deliverable title appears in the thread.".to_string());
    } else {
        let tokens = meaningful_tokens(title);
        let matches = tokens
            .iter()
            .filter(|token| haystack.contains(token.as_str()))
            .count();
        if matches > 0 {
            score += (matches as f64 * 0.12).min(0.36);
            reasons.push("Deliverable title tokens appear in the thread.".to_string());
        }
    }

    let claim_tokens = meaningful_tokens(claim);
    let claim_matches = claim_tokens
        .iter()
        .filter(|token| haystack.contains(token.as_str()))
        .count();
    if claim_matches >= 2 {
        score += 0.12;
        reasons.push("Deliverable claim overlaps with the thread text.".to_string());
    }

    if let Some(artifact_url) = artifact_url.filter(|url| !url.trim().is_empty()) {
        if artifact_urls.iter().any(|url| url == artifact_url) || haystack.contains(artifact_url) {
            score += 0.35;
            reasons.push("Artifact URL overlaps with the deliverable.".to_string());
        }
    }

    let stakeholder_set = stakeholder_emails
        .split(',')
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .collect::<BTreeSet<_>>();
    if participant_emails
        .iter()
        .any(|email| stakeholder_set.contains(email.as_str()))
    {
        score += 0.25;
        reasons.push("Thread participant is connected to the deliverable.".to_string());
    }
    if blocker_reason
        .map(|reason| !reason.trim().is_empty())
        .unwrap_or(false)
    {
        score += 0.08;
        reasons.push("Deliverable is currently blocked.".to_string());
    }
    if priority.map(is_high_priority_value).unwrap_or(false) {
        score += 0.08;
        reasons.push("Deliverable is high priority.".to_string());
    }
    if deadline
        .and_then(days_until_deadline)
        .is_some_and(|days| days <= 7)
    {
        score += 0.08;
        reasons.push("Deliverable has a near deadline.".to_string());
    }
    (score.min(1.0), dedupe_strings(reasons))
}

fn score_initiative_link(haystack: &str, title: &str, framing: &str) -> (f64, Vec<String>) {
    let mut score: f64 = 0.0;
    let mut reasons = Vec::new();
    let title_lower = title.trim().to_lowercase();
    if title_lower.len() >= 6 && haystack.contains(&title_lower) {
        score += 0.72;
        reasons.push("Initiative title appears in the thread.".to_string());
    } else {
        let matches = meaningful_tokens(title)
            .iter()
            .filter(|token| haystack.contains(token.as_str()))
            .count();
        if matches >= 2 {
            score += 0.42;
            reasons.push("Initiative title tokens appear in the thread.".to_string());
        }
    }
    let framing_matches = meaningful_tokens(framing)
        .iter()
        .filter(|token| haystack.contains(token.as_str()))
        .count();
    if framing_matches >= 3 {
        score += 0.22;
        reasons.push("Initiative framing overlaps with the thread text.".to_string());
    }
    (score.min(1.0), dedupe_strings(reasons))
}

fn thread_link_haystack(messages: &[GmailMessageRecord]) -> String {
    messages
        .iter()
        .map(|message| {
            format!(
                "{}\n{}\n{}\n{}\n{}",
                message.subject,
                message.snippet,
                message.from_name,
                message.plain_body,
                strip_html(&message.html_body)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase()
}

async fn participant_emails_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar(
        "SELECT DISTINCT lower(email) FROM gmail_thread_participants WHERE thread_id = ? AND email != ''",
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)
}

async fn thread_is_orphan(pool: &SqlitePool, thread_id: &str) -> Result<bool, String> {
    let linked: i64 = sqlx::query_scalar(
        r#"
        SELECT CASE WHEN
          EXISTS (SELECT 1 FROM gmail_thread_deliverables WHERE thread_id = ?)
          OR EXISTS (SELECT 1 FROM gmail_thread_initiatives WHERE thread_id = ?)
          OR EXISTS (SELECT 1 FROM gmail_thread_captures WHERE thread_id = ?)
          OR EXISTS (SELECT 1 FROM gmail_thread_stakeholders WHERE thread_id = ?)
          OR EXISTS (
            SELECT 1
            FROM gmail_thread_participants tp
            JOIN stakeholders s ON lower(s.email) = lower(tp.email)
            LEFT JOIN gmail_thread_stakeholder_excludes ex
              ON ex.thread_id = tp.thread_id AND ex.stakeholder_id = s.id
            WHERE tp.thread_id = ?
              AND s.email != ''
              AND ex.stakeholder_id IS NULL
          )
        THEN 1 ELSE 0 END
        "#,
    )
    .bind(thread_id)
    .bind(thread_id)
    .bind(thread_id)
    .bind(thread_id)
    .bind(thread_id)
    .fetch_one(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(linked == 0)
}

pub async fn count_orphan_threads(pool: &SqlitePool) -> Result<i64, String> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM gmail_threads t
        WHERE NOT EXISTS (SELECT 1 FROM gmail_thread_deliverables td WHERE td.thread_id = t.thread_id)
          AND NOT EXISTS (SELECT 1 FROM gmail_thread_initiatives ti WHERE ti.thread_id = t.thread_id)
          AND NOT EXISTS (SELECT 1 FROM gmail_thread_captures tc WHERE tc.thread_id = t.thread_id)
          AND NOT EXISTS (SELECT 1 FROM gmail_thread_stakeholders ts WHERE ts.thread_id = t.thread_id)
          AND NOT EXISTS (
            SELECT 1
            FROM gmail_thread_participants tp
            JOIN stakeholders s ON lower(s.email) = lower(tp.email)
            LEFT JOIN gmail_thread_stakeholder_excludes ex
              ON ex.thread_id = tp.thread_id AND ex.stakeholder_id = s.id
            WHERE tp.thread_id = t.thread_id
              AND s.email != ''
              AND ex.stakeholder_id IS NULL
          )
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(crate::db::sql_error)
}

fn meaningful_tokens(value: &str) -> Vec<String> {
    value
        .to_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::trim)
        .filter(|token| token.len() >= 4)
        .filter(|token| {
            !matches!(
                *token,
                "this"
                    | "that"
                    | "with"
                    | "from"
                    | "about"
                    | "into"
                    | "your"
                    | "have"
                    | "will"
                    | "deliverable"
                    | "project"
                    | "initiative"
            )
        })
        .map(str::to_string)
        .collect()
}

fn days_until_deadline(deadline: &str) -> Option<i64> {
    let trimmed = deadline.trim();
    let date_part = trimmed.get(0..10).unwrap_or(trimmed);
    let date = NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()?;
    Some((date - Utc::now().date_naive()).num_days())
}

fn priority_label_from_score(score: i32) -> String {
    match score {
        value if value >= 90 => "urgent",
        value if value >= 65 => "high",
        value if value >= 35 => "medium",
        _ => "low",
    }
    .to_string()
}

fn is_high_priority_value(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "urgent" | "high" | "p0" | "p1" | "critical"
    )
}

fn is_public_email_domain(domain: &str) -> bool {
    matches!(
        domain,
        "gmail.com"
            | "googlemail.com"
            | "yahoo.com"
            | "outlook.com"
            | "hotmail.com"
            | "icloud.com"
            | "me.com"
            | "live.com"
            | "proton.me"
            | "protonmail.com"
    )
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}
