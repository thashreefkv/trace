//! Local thread storage and hydration: list/get local threads, row
//! hydration, bundle-size enrichment, and message-row loading.
//! Extracted from legacy.rs (Section 13-G12).

use serde_json::json;
use sqlx::SqlitePool;

use super::models::*;
use super::{
    labels_for_thread, linked_deliverables_for_thread, linked_initiatives_for_thread,
    linked_stakeholders_for_thread, links_for_thread_urls, list_attachments, list_links,
    parse_addresses_json, parse_string_vec, stakeholder_email, work_mail_needs_me_reason,
    work_mail_review_summary,
};

pub async fn list_local_threads(
    pool: &SqlitePool,
    filters: GmailThreadFilter,
) -> Result<Vec<GmailLocalThread>, String> {
    let limit = filters.limit.unwrap_or(50).clamp(1, 500);
    let orphan_only = filters.orphan_only.unwrap_or(false);
    let rows = if let Some(query) = filters.query.as_deref().filter(|q| !q.trim().is_empty()) {
        if let Some(fts) = crate::repo::fts_query(query) {
            sqlx::query_as::<_, GmailThreadRow>(
                r#"
                SELECT t.thread_id, t.subject, t.snippet, t.participants, t.first_message_at,
                       t.last_message_at, t.message_count, t.has_unread, t.is_sent_only,
                       t.last_from_name, t.last_from_email, t.ai_title,
                       t.summary, t.sentiment, t.urgency,
                       t.ai_category, t.ai_priority, t.ai_category_confidence,
                       t.ai_category_reasons, t.ai_triaged_at,
                       t.last_analyzed_message_at, t.graph_context_json,
                       t.effective_priority, t.priority_reasons_json,
                       t.last_sync_at,
                       t.intent, t.action_required, t.predicted_action,
                       t.thread_state, t.dimensions_confidence_json,
                       t.work_relevance, t.work_relevance_reasons_json,
                       t.work_relevance_confidence, t.attention_state,
                       t.attention_reasons_json, t.attention_confidence,
                       t.message_type, t.message_type_reasons_json,
                       t.message_type_confidence, t.work_mail_updated_at,
                       t.bundle_id, t.last_analysis_error
                FROM gmail_thread_search s
                JOIN gmail_threads t ON t.thread_id = s.thread_id
                WHERE gmail_thread_search MATCH ?
                  AND (? IS NULL OR t.ai_category = ?)
                  AND (? = 0 OR (
                    NOT EXISTS (SELECT 1 FROM gmail_thread_deliverables td WHERE td.thread_id = t.thread_id)
                    AND NOT EXISTS (SELECT 1 FROM gmail_thread_initiatives ti WHERE ti.thread_id = t.thread_id)
                    AND NOT EXISTS (SELECT 1 FROM gmail_thread_captures tc WHERE tc.thread_id = t.thread_id)
                    AND NOT EXISTS (SELECT 1 FROM gmail_thread_stakeholders ts WHERE ts.thread_id = t.thread_id)
                    AND NOT EXISTS (
                      SELECT 1
                      FROM gmail_thread_participants tp
                      JOIN stakeholders s ON lower(s.email) = lower(tp.email)
                      LEFT JOIN gmail_thread_stakeholder_excludes ex
                        ON ex.thread_id = t.thread_id AND ex.stakeholder_id = s.id
                      WHERE tp.thread_id = t.thread_id
                        AND s.email != ''
                        AND ex.stakeholder_id IS NULL
                    )
                  ))
                  AND NOT EXISTS (
                      SELECT 1 FROM gmail_thread_labels blocked
                      WHERE blocked.thread_id = t.thread_id
                        AND blocked.gmail_label_id IN ('SPAM', 'TRASH')
                  )
                ORDER BY bm25(gmail_thread_search), t.last_message_at DESC
                LIMIT ?
                "#,
            )
            .bind(fts)
            .bind(filters.category.clone())
            .bind(filters.category.clone())
            .bind(orphan_only)
            .bind(limit)
            .fetch_all(pool)
            .await
            .map_err(crate::db::sql_error)?
        } else {
            Vec::new()
        }
    } else {
        let label = filters.label_id.clone();
        let category = filters.category.clone();
        let stakeholder_id = filters.stakeholder_id.clone();
        let deliverable_id = filters.deliverable_id.clone();
        let initiative_id = filters.initiative_id.clone();
        let stakeholder_email = if let Some(id) = stakeholder_id.as_deref() {
            stakeholder_email(pool, id).await?
        } else {
            None
        };
        sqlx::query_as::<_, GmailThreadRow>(
            r#"
            SELECT t.thread_id, t.subject, t.snippet, t.participants, t.first_message_at,
                   t.last_message_at, t.message_count, t.has_unread, t.is_sent_only,
                   t.last_from_name, t.last_from_email, t.ai_title,
                   t.summary, t.sentiment, t.urgency,
                   t.ai_category, t.ai_priority, t.ai_category_confidence,
                   t.ai_category_reasons, t.ai_triaged_at,
                   t.last_analyzed_message_at, t.graph_context_json,
                   t.effective_priority, t.priority_reasons_json,
                   t.last_sync_at,
                   t.intent, t.action_required, t.predicted_action,
                   t.thread_state, t.dimensions_confidence_json,
                   t.work_relevance, t.work_relevance_reasons_json,
                   t.work_relevance_confidence, t.attention_state,
                   t.attention_reasons_json, t.attention_confidence,
                   t.message_type, t.message_type_reasons_json,
                   t.message_type_confidence, t.work_mail_updated_at,
                   t.bundle_id, t.last_analysis_error
            FROM gmail_threads t
            WHERE NOT EXISTS (
                SELECT 1 FROM gmail_thread_labels blocked
                WHERE blocked.thread_id = t.thread_id
                  AND blocked.gmail_label_id IN ('SPAM', 'TRASH')
            )
              AND (
                t.is_sent_only = 1
                OR EXISTS (
                  SELECT 1 FROM gmail_thread_labels inbox
                  WHERE inbox.thread_id = t.thread_id AND inbox.gmail_label_id = 'INBOX'
                )
              )
              AND (? IS NULL OR EXISTS (
                SELECT 1 FROM gmail_thread_labels tl
                WHERE tl.thread_id = t.thread_id AND tl.gmail_label_id = ?
            ))
              AND (? IS NULL OR t.ai_category = ?)
              AND (? IS NULL OR EXISTS (
                SELECT 1 FROM gmail_thread_participants tp
                WHERE tp.thread_id = t.thread_id AND lower(tp.email) = lower(?)
            ))
              AND (? IS NULL OR NOT EXISTS (
                SELECT 1 FROM gmail_thread_stakeholder_excludes ex
                WHERE ex.thread_id = t.thread_id AND ex.stakeholder_id = ?
            ))
              AND (? IS NULL OR EXISTS (
                SELECT 1 FROM gmail_thread_deliverables td
                WHERE td.thread_id = t.thread_id AND td.deliverable_id = ?
            ))
              AND (? IS NULL OR EXISTS (
                SELECT 1 FROM gmail_thread_initiatives ti
                WHERE ti.thread_id = t.thread_id AND ti.initiative_id = ?
            ))
              AND (? = 0 OR (
                NOT EXISTS (SELECT 1 FROM gmail_thread_deliverables td WHERE td.thread_id = t.thread_id)
                AND NOT EXISTS (SELECT 1 FROM gmail_thread_initiatives ti WHERE ti.thread_id = t.thread_id)
                AND NOT EXISTS (SELECT 1 FROM gmail_thread_captures tc WHERE tc.thread_id = t.thread_id)
                AND NOT EXISTS (SELECT 1 FROM gmail_thread_stakeholders ts WHERE ts.thread_id = t.thread_id)
                AND NOT EXISTS (
                  SELECT 1
                  FROM gmail_thread_participants tp
                  JOIN stakeholders s ON lower(s.email) = lower(tp.email)
                  LEFT JOIN gmail_thread_stakeholder_excludes ex
                    ON ex.thread_id = t.thread_id AND ex.stakeholder_id = s.id
                  WHERE tp.thread_id = t.thread_id
                    AND s.email != ''
                    AND ex.stakeholder_id IS NULL
                )
              ))
            ORDER BY t.last_message_at DESC
            LIMIT ?
            "#,
        )
        .bind(label.clone())
        .bind(label)
        .bind(category.clone())
        .bind(category)
        .bind(stakeholder_email.clone())
        .bind(stakeholder_email)
        .bind(stakeholder_id.clone())
        .bind(stakeholder_id)
        .bind(deliverable_id.clone())
        .bind(deliverable_id)
        .bind(initiative_id.clone())
        .bind(initiative_id)
        .bind(orphan_only)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(crate::db::sql_error)?
    };

    hydrate_thread_rows(pool, rows).await
}

pub async fn get_local_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<GmailThreadDetail, String> {
    let row = sqlx::query_as::<_, GmailThreadRow>(
        r#"
        SELECT thread_id, subject, snippet, participants, first_message_at,
               last_message_at, message_count, has_unread, is_sent_only,
               last_from_name, last_from_email, ai_title,
               summary, sentiment, urgency,
               ai_category, ai_priority, ai_category_confidence, ai_category_reasons,
               ai_triaged_at, last_analyzed_message_at, graph_context_json,
               effective_priority, priority_reasons_json, last_sync_at,
               intent, action_required, predicted_action,
               thread_state, dimensions_confidence_json,
               work_relevance, work_relevance_reasons_json,
               work_relevance_confidence, attention_state,
               attention_reasons_json, attention_confidence,
               message_type, message_type_reasons_json,
               message_type_confidence, work_mail_updated_at,
               bundle_id, last_analysis_error
        FROM gmail_threads
        WHERE thread_id = ?
        "#,
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::db::sql_error)?
    .ok_or_else(|| "gmail thread not found".to_string())?;

    let thread = hydrate_thread_rows(pool, vec![row])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| "gmail thread not found".to_string())?;
    let messages = load_message_rows(pool, thread_id).await?;
    let attachments = list_attachments(pool, thread_id).await?;
    let links = list_links(pool, thread_id).await?;

    Ok(GmailThreadDetail {
        thread,
        messages,
        attachments,
        links,
    })
}

async fn hydrate_thread_rows(
    pool: &SqlitePool,
    rows: Vec<GmailThreadRow>,
) -> Result<Vec<GmailLocalThread>, String> {
    let mut threads = Vec::with_capacity(rows.len());
    for row in rows {
        let labels = labels_for_thread(pool, &row.thread_id).await?;
        let linked_deliverables = linked_deliverables_for_thread(pool, &row.thread_id).await?;
        let linked_initiatives = linked_initiatives_for_thread(pool, &row.thread_id).await?;
        let linked_stakeholders = linked_stakeholders_for_thread(pool, &row.thread_id).await?;
        let artifact_urls = links_for_thread_urls(pool, &row.thread_id).await?;
        let graph_context =
            serde_json::from_str(&row.graph_context_json).unwrap_or_else(|_| json!({}));
        let effective = crate::gmail_intel::effective_for_thread(pool, &row.thread_id)
            .await
            .ok();
        let review = work_mail_review_summary(pool, &row.thread_id).await?;
        let mut thread = GmailLocalThread {
            thread_id: row.thread_id,
            subject: row.subject,
            snippet: row.snippet,
            participants: parse_addresses_json(&row.participants),
            first_message_at: row.first_message_at,
            last_message_at: row.last_message_at,
            message_count: row.message_count,
            has_unread: row.has_unread,
            gmail_read_state: if row.has_unread {
                "unread".to_string()
            } else {
                "read".to_string()
            },
            is_sent_only: row.is_sent_only,
            last_from_name: row.last_from_name,
            last_from_email: row.last_from_email,
            ai_title: row.ai_title,
            summary: row.summary,
            sentiment: row.sentiment,
            urgency: row.urgency,
            ai_category: row.ai_category,
            ai_priority: row.ai_priority,
            ai_category_confidence: row.ai_category_confidence,
            ai_category_reasons: parse_string_vec(&row.ai_category_reasons),
            ai_triaged_at: row.ai_triaged_at,
            labels,
            linked_deliverables,
            linked_initiatives,
            linked_stakeholders,
            artifact_urls,
            effective_priority: row.effective_priority,
            priority_reasons: parse_string_vec(&row.priority_reasons_json),
            graph_context,
            last_analyzed_message_at: row.last_analyzed_message_at,
            last_sync_at: row.last_sync_at,
            intent: row.intent,
            action_required: row.action_required != 0,
            predicted_action: row.predicted_action,
            thread_state: row.thread_state,
            dimensions_confidence: serde_json::from_str(&row.dimensions_confidence_json)
                .unwrap_or_else(|_| json!({})),
            work_relevance: effective
                .as_ref()
                .map(|classification| classification.work_relevance.clone())
                .unwrap_or(row.work_relevance),
            work_relevance_reasons: parse_string_vec(&row.work_relevance_reasons_json),
            work_relevance_confidence: row.work_relevance_confidence,
            attention_state: effective
                .as_ref()
                .map(|classification| classification.attention_state.clone())
                .unwrap_or(row.attention_state),
            attention_reasons: parse_string_vec(&row.attention_reasons_json),
            attention_confidence: row.attention_confidence,
            message_type: effective
                .as_ref()
                .map(|classification| classification.message_type.clone())
                .unwrap_or(row.message_type),
            message_type_reasons: parse_string_vec(&row.message_type_reasons_json),
            message_type_confidence: row.message_type_confidence,
            work_mail_updated_at: row.work_mail_updated_at,
            trace_seen_at: review.trace_seen_at,
            trace_review_state: review.review_state.as_str().to_string(),
            seen_through_message_id: review.seen.message_id,
            seen_through_message_at: review.seen.message_at,
            reviewed_through_message_id: review.reviewed_through_message_id,
            reviewed_through_message_at: review.reviewed_through_message_at,
            deferred_until: review.deferred_until,
            new_since_review: review.new_since_review,
            needs_me_reason: None,
            bundle_id: row.bundle_id,
            bundle_size: 0, // filled in by attach_bundle_sizes below
            last_analysis_error: row.last_analysis_error,
        };
        thread.needs_me_reason = work_mail_needs_me_reason(&thread);
        threads.push(thread);
    }
    attach_bundle_sizes(pool, &mut threads).await;
    Ok(threads)
}

/// One batch query: count threads per bundle_id present in the result set,
/// then enrich each thread with its bundle_size.
async fn attach_bundle_sizes(pool: &SqlitePool, threads: &mut [GmailLocalThread]) {
    let bundle_ids: Vec<String> = threads
        .iter()
        .filter_map(|t| t.bundle_id.clone())
        .collect();
    if bundle_ids.is_empty() {
        return;
    }
    let mut sql = String::from(
        "SELECT bundle_id, COUNT(*) FROM gmail_threads WHERE bundle_id IN (",
    );
    for (i, _) in bundle_ids.iter().enumerate() {
        if i > 0 {
            sql.push(',');
        }
        sql.push('?');
    }
    sql.push_str(") GROUP BY bundle_id");

    let mut query = sqlx::query_as::<_, (String, i64)>(&sql);
    for id in &bundle_ids {
        query = query.bind(id);
    }
    let Ok(rows) = query.fetch_all(pool).await else {
        return;
    };
    let counts: std::collections::HashMap<String, i64> = rows.into_iter().collect();
    for thread in threads.iter_mut() {
        if let Some(bid) = thread.bundle_id.as_deref() {
            thread.bundle_size = counts.get(bid).copied().unwrap_or(1);
        } else {
            thread.bundle_size = 1;
        }
    }
}

pub async fn load_message_rows(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<GmailMessageRecord>, String> {
    let rows = sqlx::query_as::<_, GmailMessageRow>(
        r#"
        SELECT message_id, thread_id, history_id, subject, snippet, from_name, from_email,
               to_json, cc_json, bcc_json, date_ts, internal_date_ts, plain_body, html_body,
               label_ids_json, is_sent, is_draft, is_unread, size_estimate,
               artifact_urls_json, synced_at
        FROM gmail_messages
        WHERE thread_id = ?
        ORDER BY COALESCE(internal_date_ts, date_ts, 0) ASC
        "#,
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;

    Ok(rows
        .into_iter()
        .map(|row| GmailMessageRecord {
            message_id: row.message_id,
            thread_id: row.thread_id,
            history_id: row.history_id,
            subject: row.subject,
            snippet: row.snippet,
            from_name: row.from_name,
            from_email: row.from_email,
            to: parse_addresses_json(&row.to_json),
            cc: parse_addresses_json(&row.cc_json),
            bcc: parse_addresses_json(&row.bcc_json),
            date_ts: row.date_ts,
            internal_date_ts: row.internal_date_ts,
            plain_body: row.plain_body,
            html_body: row.html_body,
            label_ids: parse_string_vec(&row.label_ids_json),
            is_sent: row.is_sent,
            is_draft: row.is_draft,
            is_unread: row.is_unread,
            size_estimate: row.size_estimate,
            artifact_urls: parse_string_vec(&row.artifact_urls_json),
            synced_at: row.synced_at,
        })
        .collect())
}
