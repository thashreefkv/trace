//! Read-side joins: labels, linked deliverables/initiatives/stakeholders,
//! attachments, links, drafts, and category counts. Extracted from
//! legacy.rs (Section 13-G14).

use sqlx::SqlitePool;

use super::models::*;
use super::parse_addresses_json;

pub async fn list_gmail_labels(pool: &SqlitePool) -> Result<Vec<GmailLabelRecord>, String> {
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT gmail_label_id, name, type, color FROM gmail_labels ORDER BY type ASC, name ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(gmail_label_id, name, label_type, color)| GmailLabelRecord {
                gmail_label_id,
                name,
                label_type,
                color,
            },
        )
        .collect())
}

pub async fn labels_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<GmailLabelRecord>, String> {
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT l.gmail_label_id, l.name, l.type, l.color
        FROM gmail_labels l
        INNER JOIN gmail_thread_labels tl ON tl.gmail_label_id = l.gmail_label_id
        WHERE tl.thread_id = ?
        ORDER BY l.type ASC, l.name ASC
        "#,
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(gmail_label_id, name, label_type, color)| GmailLabelRecord {
                gmail_label_id,
                name,
                label_type,
                color,
            },
        )
        .collect())
}

pub async fn linked_deliverables_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<LinkedDeliverableRef>, String> {
    let rows: Vec<(String, String, String, String, String, Option<f64>, String)> = sqlx::query_as(
        r#"
        SELECT d.id, d.title, d.state, td.linked_at,
               td.source, td.confidence, td.rationale
        FROM deliverables d
        INNER JOIN gmail_thread_deliverables td ON td.deliverable_id = d.id
        WHERE td.thread_id = ?
        ORDER BY td.linked_at DESC
        "#,
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(id, title, state, linked_at, source, confidence, rationale)| LinkedDeliverableRef {
            id,
            title,
            state,
            linked_at,
            source,
            confidence,
            rationale,
        },
        )
        .collect())
}

pub async fn category_counts(pool: &SqlitePool) -> Result<Vec<GmailCategoryCount>, String> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT ai_category, COUNT(*) AS count
        FROM gmail_threads t
        WHERE NOT EXISTS (
            SELECT 1 FROM gmail_thread_labels blocked
            WHERE blocked.thread_id = t.thread_id
              AND blocked.gmail_label_id IN ('SPAM', 'TRASH')
        )
        GROUP BY ai_category
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;

    Ok(rows
        .into_iter()
        .map(|(category, count)| GmailCategoryCount { category, count })
        .collect())
}

pub async fn linked_initiatives_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<LinkedInitiativeRef>, String> {
    let rows: Vec<(String, String, String, String, String, Option<f64>, String)> = sqlx::query_as(
        r#"
        SELECT i.id, i.title, i.status, ti.linked_at,
               ti.source, ti.confidence, ti.rationale
        FROM initiatives i
        INNER JOIN gmail_thread_initiatives ti ON ti.initiative_id = i.id
        WHERE ti.thread_id = ?
        ORDER BY ti.linked_at DESC
        "#,
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(id, title, status, linked_at, source, confidence, rationale)| LinkedInitiativeRef {
            id,
            title,
            status,
            linked_at,
            source,
            confidence,
            rationale,
        },
        )
        .collect())
}

pub async fn linked_stakeholders_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<LinkedStakeholderRef>, String> {
    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        String,
        Option<f64>,
        String,
    )> = sqlx::query_as(
        r#"
        SELECT s.id, s.name, s.email, s.role, ts.linked_at,
               ts.source, ts.confidence, ts.rationale
        FROM stakeholders s
        INNER JOIN gmail_thread_stakeholders ts ON ts.stakeholder_id = s.id
        WHERE ts.thread_id = ?
          AND NOT EXISTS (
            SELECT 1 FROM gmail_thread_stakeholder_excludes ex
            WHERE ex.thread_id = ts.thread_id AND ex.stakeholder_id = ts.stakeholder_id
          )
        UNION ALL
        SELECT s.id, s.name, s.email, s.role,
               COALESCE(MAX(tp.last_seen_at), datetime('now')) AS linked_at,
               'participant_email' AS source,
               0.8 AS confidence,
               'Participant email matches a stakeholder profile.' AS rationale
        FROM stakeholders s
        INNER JOIN gmail_thread_participants tp ON lower(tp.email) = lower(s.email)
        WHERE tp.thread_id = ?
          AND s.email != ''
          AND NOT EXISTS (
            SELECT 1 FROM gmail_thread_stakeholders ts
            WHERE ts.thread_id = tp.thread_id AND ts.stakeholder_id = s.id
          )
          AND NOT EXISTS (
            SELECT 1 FROM gmail_thread_stakeholder_excludes ex
            WHERE ex.thread_id = tp.thread_id AND ex.stakeholder_id = s.id
          )
        GROUP BY s.id, s.name, s.email, s.role
        ORDER BY linked_at DESC
        "#,
    )
    .bind(thread_id)
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;

    Ok(rows
        .into_iter()
        .map(
            |(id, name, email, role, linked_at, source, confidence, rationale)| {
                LinkedStakeholderRef {
                    id,
                    name,
                    email,
                    role,
                    linked_at,
                    source,
                    confidence,
                    rationale,
                }
            },
        )
        .collect())
}

pub async fn list_attachments(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<GmailAttachmentRecord>, String> {
    let rows = sqlx::query_as::<_, GmailAttachmentRow>(
        r#"
        SELECT id, message_id, thread_id, attachment_id, filename, mime_type, size,
               shared_by_email, shared_with_json, created_at
        FROM gmail_attachments
        WHERE thread_id = ?
        ORDER BY created_at DESC, filename ASC
        "#,
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(|row| GmailAttachmentRecord {
            id: row.id,
            message_id: row.message_id,
            thread_id: row.thread_id,
            attachment_id: row.attachment_id,
            filename: row.filename,
            mime_type: row.mime_type,
            size: row.size,
            shared_by_email: row.shared_by_email,
            shared_with: parse_addresses_json(&row.shared_with_json),
            created_at: row.created_at,
        })
        .collect())
}

pub async fn list_links(pool: &SqlitePool, thread_id: &str) -> Result<Vec<GmailLinkRecord>, String> {
    let rows: Vec<(
        String,
        String,
        Option<String>,
        String,
        String,
        Option<String>,
        String,
    )> = sqlx::query_as(
        r#"
            SELECT id, thread_id, message_id, url, kind, title, created_at
            FROM gmail_links
            WHERE thread_id = ?
            ORDER BY created_at DESC
            "#,
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(id, thread_id, message_id, url, kind, title, created_at)| GmailLinkRecord {
                id,
                thread_id,
                message_id,
                url,
                kind,
                title,
                created_at,
            },
        )
        .collect())
}

pub async fn links_for_thread_urls(pool: &SqlitePool, thread_id: &str) -> Result<Vec<String>, String> {
    sqlx::query_scalar(
        "SELECT url FROM gmail_links WHERE thread_id = ? AND kind != 'url' ORDER BY created_at DESC",
    )
        .bind(thread_id)
        .fetch_all(pool)
        .await
        .map_err(crate::db::sql_error)
}

pub async fn list_drafts(pool: &SqlitePool) -> Result<Vec<GmailDraftRecord>, String> {
    let rows = sqlx::query_as::<_, GmailDraftRow>(
        r#"
        SELECT draft_id, message_id, thread_id, subject, to_json, cc_json, bcc_json,
               body_preview, updated_at, synced_at
        FROM gmail_drafts
        ORDER BY COALESCE(updated_at, synced_at) DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(|row| GmailDraftRecord {
            draft_id: row.draft_id,
            message_id: row.message_id,
            thread_id: row.thread_id,
            subject: row.subject,
            to: parse_addresses_json(&row.to_json),
            cc: parse_addresses_json(&row.cc_json),
            bcc: parse_addresses_json(&row.bcc_json),
            body_preview: row.body_preview,
            updated_at: row.updated_at,
            synced_at: row.synced_at,
        })
        .collect())
}

