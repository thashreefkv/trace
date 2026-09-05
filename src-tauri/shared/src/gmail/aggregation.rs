//! Post-sync rollups: thread aggregate rebuild, participant aggregation
//! (per-thread + global), thread search index, and follow-up tracking.
//! Extracted from legacy.rs (Section 13-G13).

use std::collections::{BTreeMap, BTreeSet};

use sqlx::SqlitePool;

use super::models::*;
use super::{
    format_ts, get_local_thread, infer_thread_category, load_message_rows, now_utc,
    refresh_thread_intelligence, refresh_work_mail_review_state, strip_html, to_json_string,
};

pub async fn rebuild_thread_aggregate(
    pool: &SqlitePool,
    thread_id: &str,
    now: &str,
) -> Result<(), String> {
    let messages = load_message_rows(pool, thread_id).await?;
    if messages.is_empty() {
        return Ok(());
    }
    let account_email: Option<String> =
        sqlx::query_scalar("SELECT account_email FROM gmail_sync_settings WHERE id = 1")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten();
    let first = messages.first().unwrap();
    let last = messages.last().unwrap();
    let participants = aggregate_participants(&messages);
    let participant_json = to_json_string(
        &participants
            .values()
            .map(|entry| EmailAddress {
                name: entry.name.clone(),
                email: entry.email.clone(),
            })
            .collect::<Vec<_>>(),
    );
    let has_unread = messages.iter().any(|message| message.is_unread);
    let is_sent_only = messages
        .iter()
        .all(|message| message.is_sent || message.is_draft);

    sqlx::query(
        r#"
        UPDATE gmail_threads
        SET subject = ?,
            snippet = ?,
            participants = ?,
            first_message_at = ?,
            last_message_at = ?,
            message_count = ?,
            has_unread = ?,
            is_sent_only = ?,
            last_from_name = ?,
            last_from_email = ?,
            last_sync_at = ?
        WHERE thread_id = ?
        "#,
    )
    .bind(&first.subject)
    .bind(&last.snippet)
    .bind(&participant_json)
    .bind(first.internal_date_ts.or(first.date_ts))
    .bind(last.internal_date_ts.or(last.date_ts))
    .bind(messages.len() as i64)
    .bind(has_unread)
    .bind(is_sent_only)
    .bind(&last.from_name)
    .bind(&last.from_email)
    .bind(now)
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    sqlx::query("DELETE FROM gmail_thread_labels WHERE thread_id = ?")
        .bind(thread_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    let mut label_ids = BTreeSet::new();
    for message in &messages {
        for label in &message.label_ids {
            label_ids.insert(label.clone());
        }
    }
    for label_id in label_ids {
        sqlx::query(
            "INSERT OR IGNORE INTO gmail_thread_labels (thread_id, gmail_label_id) VALUES (?, ?)",
        )
        .bind(thread_id)
        .bind(label_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    }

    let category = infer_thread_category(&messages, account_email.as_deref());
    sqlx::query(
        r#"
        UPDATE gmail_threads
        SET ai_category = ?,
            ai_priority = ?,
            ai_category_confidence = ?,
            ai_category_reasons = ?,
            ai_triaged_at = COALESCE(ai_triaged_at, ?)
        WHERE thread_id = ?
          AND (ai_triaged_at IS NULL OR ai_category IS NULL OR ai_category = '' OR ai_category = 'other')
        "#,
    )
    .bind(&category.category)
    .bind(&category.priority)
    .bind(category.confidence)
    .bind(to_json_string(&category.reasons))
    .bind(now)
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    rebuild_thread_participants(pool, thread_id, &messages).await?;
    rebuild_thread_search(pool, thread_id).await?;
    refresh_thread_intelligence(pool, thread_id).await?;
    refresh_work_mail_review_state(pool, thread_id).await?;
    Ok(())
}



#[derive(Debug)]
struct ParticipantAggregate {
    email: String,
    name: String,
    roles: BTreeSet<String>,
    count: i64,
    first_seen: String,
    last_seen: String,
}

fn aggregate_participants(
    messages: &[GmailMessageRecord],
) -> BTreeMap<String, ParticipantAggregate> {
    let mut aggregate: BTreeMap<String, ParticipantAggregate> = BTreeMap::new();
    for message in messages {
        let message_at =
            format_ts(message.internal_date_ts.or(message.date_ts)).unwrap_or_else(now_utc);
        let participants = std::iter::once((
            "from",
            EmailAddress {
                name: message.from_name.clone(),
                email: message.from_email.clone(),
            },
        ))
        .chain(message.to.iter().cloned().map(|address| ("to", address)))
        .chain(message.cc.iter().cloned().map(|address| ("cc", address)))
        .chain(message.bcc.iter().cloned().map(|address| ("bcc", address)));

        for (role, address) in participants {
            if address.email.trim().is_empty() {
                continue;
            }
            let key = address.email.to_lowercase();
            let entry = aggregate
                .entry(key.clone())
                .or_insert_with(|| ParticipantAggregate {
                    email: key,
                    name: address.name.clone(),
                    roles: BTreeSet::new(),
                    count: 0,
                    first_seen: message_at.clone(),
                    last_seen: message_at.clone(),
                });
            if entry.name.is_empty() && !address.name.is_empty() {
                entry.name = address.name;
            }
            entry.roles.insert(role.to_string());
            entry.count += 1;
            if message_at < entry.first_seen {
                entry.first_seen = message_at.clone();
            }
            if message_at > entry.last_seen {
                entry.last_seen = message_at.clone();
            }
        }
    }
    aggregate
}

async fn rebuild_thread_participants(
    pool: &SqlitePool,
    thread_id: &str,
    messages: &[GmailMessageRecord],
) -> Result<(), String> {
    sqlx::query("DELETE FROM gmail_thread_participants WHERE thread_id = ?")
        .bind(thread_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;

    let participants = aggregate_participants(messages);
    for participant in participants.values() {
        let role = participant
            .roles
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        sqlx::query(
            r#"
            INSERT INTO gmail_thread_participants (
              thread_id, email, name, role, message_count, first_seen_at, last_seen_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(thread_id)
        .bind(&participant.email)
        .bind(&participant.name)
        .bind(role)
        .bind(participant.count)
        .bind(&participant.first_seen)
        .bind(&participant.last_seen)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    }
    Ok(())
}

pub async fn rebuild_global_participants(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query("DELETE FROM gmail_participants")
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;

    sqlx::query(
        r#"
        INSERT INTO gmail_participants (
          email, name, first_seen_at, last_seen_at, sent_count, received_count, thread_count, last_thread_id
        )
        SELECT
          lower(p.email) AS email,
          COALESCE(NULLIF(MAX(p.name), ''), lower(p.email)) AS name,
          MIN(p.first_seen_at) AS first_seen_at,
          MAX(p.last_seen_at) AS last_seen_at,
          COALESCE(SUM(CASE WHEN instr(p.role, 'to') > 0 OR instr(p.role, 'cc') > 0 OR instr(p.role, 'bcc') > 0 THEN 1 ELSE 0 END), 0) AS sent_count,
          COALESCE(SUM(CASE WHEN instr(p.role, 'from') > 0 THEN 1 ELSE 0 END), 0) AS received_count,
          COUNT(DISTINCT p.thread_id) AS thread_count,
          (
            SELECT p2.thread_id
            FROM gmail_thread_participants p2
            WHERE lower(p2.email) = lower(p.email)
            ORDER BY p2.last_seen_at DESC
            LIMIT 1
          ) AS last_thread_id
        FROM gmail_thread_participants p
        WHERE p.email != ''
        GROUP BY lower(p.email)
        "#,
    )
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

async fn rebuild_thread_search(pool: &SqlitePool, thread_id: &str) -> Result<(), String> {
    let detail = get_local_thread(pool, thread_id).await?;
    let participants = detail
        .thread
        .participants
        .iter()
        .map(|participant| format!("{} {}", participant.name, participant.email))
        .collect::<Vec<_>>()
        .join(" ");
    let body = detail
        .messages
        .iter()
        .map(|message| {
            if message.plain_body.trim().is_empty() {
                strip_html(&message.html_body)
            } else {
                message.plain_body.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    sqlx::query("DELETE FROM gmail_thread_search WHERE thread_id = ?")
        .bind(thread_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    sqlx::query(
        "INSERT INTO gmail_thread_search (thread_id, subject, participants, body) VALUES (?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(&detail.thread.subject)
    .bind(participants)
    .bind(body)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

pub async fn update_followups(pool: &SqlitePool) -> Result<(), String> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT m.thread_id, m.message_id, COALESCE(m.internal_date_ts, m.date_ts, 0) AS sent_ts
        FROM gmail_messages m
        WHERE m.is_sent = 1
          AND m.is_draft = 0
          AND COALESCE(m.internal_date_ts, m.date_ts, 0) > 0
          AND NOT EXISTS (
            SELECT 1
            FROM gmail_messages reply
            WHERE reply.thread_id = m.thread_id
              AND reply.is_sent = 0
              AND COALESCE(reply.internal_date_ts, reply.date_ts, 0) > COALESCE(m.internal_date_ts, m.date_ts, 0)
          )
        ORDER BY sent_ts DESC
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;

    let now = now_utc();
    for (thread_id, message_id, sent_ts) in rows {
        let sent_at = format_ts(Some(sent_ts)).unwrap_or_else(now_utc);
        let due_at = format_ts(Some(sent_ts + 3 * 24 * 60 * 60)).unwrap_or_else(now_utc);
        sqlx::query(
            r#"
            INSERT INTO gmail_followups (
              id, thread_id, message_id, sent_at, expected_reply_after_days, due_at,
              status, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, 3, ?, 'open', ?, ?)
            ON CONFLICT(id) DO NOTHING
            "#,
        )
        .bind(format!("{}:{}", thread_id, message_id))
        .bind(thread_id)
        .bind(message_id)
        .bind(sent_at)
        .bind(due_at)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    }

    sqlx::query(
        r#"
        UPDATE gmail_followups
        SET status = 'resolved',
            resolved_at = ?,
            updated_at = ?
        WHERE status = 'open'
          AND EXISTS (
            SELECT 1
            FROM gmail_messages reply
            WHERE reply.thread_id = gmail_followups.thread_id
              AND reply.is_sent = 0
              AND COALESCE(reply.internal_date_ts, reply.date_ts, 0) >
                  COALESCE(strftime('%s', gmail_followups.sent_at), 0)
          )
        "#,
    )
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    Ok(())
}
