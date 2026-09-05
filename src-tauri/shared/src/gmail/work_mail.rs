use std::collections::BTreeSet;

use chrono::Utc;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use ulid::Ulid;

use super::models::*;
use super::{
    delete_local_thread, email_domain, format_ts, get_local_thread, is_blocked_spam_or_trash,
    list_local_threads, load_message_rows, local_message_is_low_signal, max_history, now_utc,
    parse_addresses_json, parse_string_vec, refresh_thread_intelligence, to_json_string,
};

pub fn infer_thread_category(
    messages: &[GmailMessageRecord],
    _account_email: Option<&str>,
) -> ThreadCategory {
    let labels = messages
        .iter()
        .flat_map(|message| message.label_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let subject = messages
        .last()
        .map(|message| message.subject.to_lowercase())
        .unwrap_or_default();
    let sender = messages
        .last()
        .map(|message| message.from_email.to_lowercase())
        .unwrap_or_default();
    let has_sent = messages.iter().any(|message| message.is_sent);
    let inbound = messages
        .iter()
        .any(|message| !message.is_sent && !message.is_draft);
    let low_signal = messages.iter().all(local_message_is_low_signal);
    let important = labels.contains("IMPORTANT") || labels.contains("STARRED");
    // Deterministic bulk filters run before the legacy heuristic cascade.

    if labels.contains("SPAM") || labels.contains("TRASH") {
        return ThreadCategory {
            category: "spam".to_string(),
            priority: "low".to_string(),
            confidence: 0.97,
            reasons: vec!["Gmail marked this thread as spam or trash.".to_string()],
        };
    }

    if subject.contains("receipt")
        || subject.contains("invoice")
        || subject.contains("order")
        || subject.contains("payment")
        || subject.contains("otp")
        || subject.contains("verification")
        || subject.contains("security alert")
    {
        return ThreadCategory {
            category: "receipt".to_string(),
            priority: "low".to_string(),
            confidence: 0.82,
            reasons: vec!["Subject looks transactional or account-related.".to_string()],
        };
    }

    if labels.contains("CATEGORY_PROMOTIONS")
        || labels.contains("CATEGORY_SOCIAL")
        || labels.contains("CATEGORY_FORUMS")
        || sender.contains("newsletter")
        || subject.contains("newsletter")
        || subject.contains("unsubscribe")
        || subject.contains("digest")
    {
        return ThreadCategory {
            category: "newsletter".to_string(),
            priority: "low".to_string(),
            confidence: 0.85,
            reasons: vec!["Gmail/category signals indicate bulk or newsletter mail.".to_string()],
        };
    }

    // Work relevance is owned by Work Mail dimensions, not by legacy category.

    let mut category = "other";
    let mut priority = "low";
    let mut confidence = 0.60;
    let mut reasons = Vec::new();

    if subject.contains("meeting")
        || subject.contains("calendar")
        || subject.contains("invite")
        || subject.contains("agenda")
    {
        category = "meeting";
        priority = if important { "high" } else { "medium" };
        confidence = 0.74;
        reasons.push("Subject appears to be meeting-related.".to_string());
    } else if important
        || subject.contains("urgent")
        || subject.contains("asap")
        || subject.contains("deadline")
        || subject.contains("blocked")
    {
        category = "action_required";
        priority = if important { "high" } else { "medium" };
        confidence = 0.76;
        reasons.push("Priority or action language suggests attention is needed.".to_string());
    } else if has_sent {
        category = "work";
        priority = "medium";
        confidence = 0.66;
        reasons.push("Thread includes sent mail — likely active work context.".to_string());
    } else if inbound && !low_signal {
        category = "other";
        confidence = 0.58;
        reasons.push("Direct inbound from outside known work domains.".to_string());
    } else if low_signal {
        category = "archive";
        confidence = 0.66;
        reasons.push("Automated or low-signal content is likely safe to archive.".to_string());
    }

    ThreadCategory {
        category: category.to_string(),
        priority: priority.to_string(),
        confidence,
        reasons,
    }
}

pub async fn refresh_work_mail_dimensions(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<(), String> {
    let messages = load_message_rows(pool, thread_id).await?;
    if messages.is_empty() {
        return Ok(());
    }
    let work_domains = enabled_work_mail_domain_set(pool).await?;
    let linked_work_context = thread_has_work_object_links(pool, thread_id).await?;
    let attachment_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM gmail_attachments WHERE thread_id = ?")
            .bind(thread_id)
            .fetch_one(pool)
            .await
            .map_err(crate::db::sql_error)?;
    let row: Option<(String, i64, Option<String>, String)> = sqlx::query_as(
        "SELECT COALESCE(ai_category, 'other'), COALESCE(action_required, 0),
                thread_state, COALESCE(work_relevance, 'unknown')
           FROM gmail_threads WHERE thread_id = ?",
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::db::sql_error)?;
    let Some((category, action_required, thread_state, prior_relevance)) = row else {
        return Ok(());
    };
    let dimensions = infer_work_mail_dimensions(
        &messages,
        &work_domains,
        linked_work_context,
        attachment_count > 0,
        &category,
        action_required != 0,
        thread_state.as_deref(),
    );
    sqlx::query(
        r#"
        UPDATE gmail_threads
        SET work_relevance = ?,
            work_relevance_reasons_json = ?,
            work_relevance_confidence = ?,
            attention_state = ?,
            attention_reasons_json = ?,
            attention_confidence = ?,
            message_type = ?,
            message_type_reasons_json = ?,
            message_type_confidence = ?,
            work_mail_updated_at = ?
        WHERE thread_id = ?
        "#,
    )
    .bind(&dimensions.work_relevance)
    .bind(to_json_string(&dimensions.work_relevance_reasons))
    .bind(dimensions.work_relevance_confidence)
    .bind(&dimensions.attention_state)
    .bind(to_json_string(&dimensions.attention_reasons))
    .bind(dimensions.attention_confidence)
    .bind(&dimensions.message_type)
    .bind(to_json_string(&dimensions.message_type_reasons))
    .bind(dimensions.message_type_confidence)
    .bind(now_utc())
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    if prior_relevance != dimensions.work_relevance {
        record_work_mail_agent_event(
            pool,
            Some(thread_id),
            "placement",
            "trace",
            &format!(
                "Placed thread in {} Work Mail scope.",
                dimensions.work_relevance.replace('_', " ")
            ),
            json!(dimensions.work_relevance_reasons),
            json!({
                "from": prior_relevance,
                "to": dimensions.work_relevance,
                "attention_state": dimensions.attention_state,
                "message_type": dimensions.message_type
            }),
            None,
        )
        .await?;
    }
    Ok(())
}

pub(super) fn infer_work_mail_dimensions(
    messages: &[GmailMessageRecord],
    work_domains: &BTreeSet<String>,
    linked_work_context: bool,
    has_attachment: bool,
    legacy_category: &str,
    action_required: bool,
    thread_state: Option<&str>,
) -> WorkMailDimensions {
    let labels = messages
        .iter()
        .flat_map(|message| message.label_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let subject = messages
        .last()
        .map(|message| message.subject.to_lowercase())
        .unwrap_or_default();
    let sender = messages
        .last()
        .map(|message| message.from_email.to_lowercase())
        .unwrap_or_default();
    let has_artifact = has_attachment
        || messages
            .iter()
            .any(|message| !message.artifact_urls.is_empty());
    let from_work_domain = messages.iter().any(|message| {
        !message.is_sent
            && !message.is_draft
            && email_domain(&message.from_email)
                .map(|domain| work_domains.contains(&domain))
                .unwrap_or(false)
    });
    let is_promotion = labels.contains("CATEGORY_PROMOTIONS")
        || subject.contains("promotion")
        || subject.contains("offer")
        || subject.contains("sale");
    let is_newsletter = labels.contains("CATEGORY_SOCIAL")
        || labels.contains("CATEGORY_FORUMS")
        || sender.contains("newsletter")
        || subject.contains("newsletter")
        || subject.contains("unsubscribe")
        || subject.contains("digest")
        || legacy_category == "newsletter";
    let low_signal = messages.iter().all(local_message_is_low_signal);
    let receipt_like = legacy_category == "receipt"
        || subject.contains("receipt")
        || subject.contains("invoice")
        || subject.contains("order")
        || subject.contains("payment");

    let (message_type, message_type_reasons, message_type_confidence) = if is_promotion {
        (
            "promotion",
            vec!["Promotional category or subject signal.".to_string()],
            0.93,
        )
    } else if is_newsletter {
        (
            "newsletter",
            vec!["Newsletter or bulk digest signal.".to_string()],
            0.91,
        )
    } else if receipt_like {
        (
            "receipt",
            vec!["Transactional receipt or invoice signal.".to_string()],
            0.86,
        )
    } else if subject.contains("meeting")
        || subject.contains("calendar")
        || subject.contains("invite")
        || subject.contains("agenda")
        || legacy_category == "meeting"
    {
        (
            "meeting",
            vec!["Meeting or calendar signal.".to_string()],
            0.82,
        )
    } else if has_artifact {
        (
            "file_share",
            vec!["Thread carries a file attachment or work artifact URL.".to_string()],
            0.78,
        )
    } else if low_signal {
        (
            "notification",
            vec!["Automated sender or bulk header signal.".to_string()],
            0.72,
        )
    } else {
        (
            "conversation",
            vec!["Direct thread without bulk or transactional signals.".to_string()],
            0.64,
        )
    };

    let (work_relevance, work_relevance_reasons, work_relevance_confidence) = if linked_work_context
    {
        if from_work_domain {
            (
                "work",
                vec!["Accepted Trace work link and enabled work-domain sender.".to_string()],
                0.99,
            )
        } else {
            (
                "linked_external",
                vec!["External thread has accepted Trace work context.".to_string()],
                0.98,
            )
        }
    } else if matches!(message_type, "promotion" | "newsletter") || low_signal {
        (
            "excluded",
            vec![
                "Recoverable bulk or low-signal mail is excluded from Work Mail by default."
                    .to_string(),
            ],
            0.92,
        )
    } else if from_work_domain {
        (
            "work",
            vec!["Inbound sender matches an enabled Work Mail domain.".to_string()],
            0.97,
        )
    } else if receipt_like {
        (
            "non_work",
            vec!["Transactional external mail has no accepted work link.".to_string()],
            0.80,
        )
    } else {
        (
            "unknown",
            vec!["External mail has no accepted Trace work link or manual promotion.".to_string()],
            0.58,
        )
    };

    let (attention_state, attention_reasons, attention_confidence) =
        if thread_state == Some("resolved") {
            (
                "resolved",
                vec!["Existing thread state is resolved.".to_string()],
                0.92,
            )
        } else if thread_state == Some("waiting_on_them") {
            (
                "waiting",
                vec!["Existing thread state is waiting on another party.".to_string()],
                0.88,
            )
        } else if thread_state == Some("waiting_on_you")
            || action_required
            || legacy_category == "action_required"
        {
            (
                "needs_me",
                vec!["Action or waiting-on-you signal needs review.".to_string()],
                0.89,
            )
        } else if message_type == "meeting" {
            (
                "scheduled",
                vec!["Meeting-related thread is scheduled context.".to_string()],
                0.78,
            )
        } else if messages.iter().any(|message| message.is_unread) {
            (
                "review",
                vec!["Unread work mail should be scanned.".to_string()],
                0.70,
            )
        } else {
            (
                "fyi",
                vec!["No action-bearing signal detected.".to_string()],
                0.65,
            )
        };

    WorkMailDimensions {
        work_relevance: work_relevance.to_string(),
        work_relevance_reasons,
        work_relevance_confidence,
        attention_state: attention_state.to_string(),
        attention_reasons,
        attention_confidence,
        message_type: message_type.to_string(),
        message_type_reasons,
        message_type_confidence,
    }
}

async fn enabled_work_mail_domain_set(pool: &SqlitePool) -> Result<BTreeSet<String>, String> {
    let domains: Vec<String> =
        sqlx::query_scalar("SELECT lower(domain) FROM gmail_work_domains WHERE enabled = 1")
            .fetch_all(pool)
            .await
            .map_err(crate::db::sql_error)?;
    Ok(domains.into_iter().collect())
}

async fn thread_has_work_object_links(pool: &SqlitePool, thread_id: &str) -> Result<bool, String> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT
          (SELECT COUNT(*) FROM gmail_thread_deliverables WHERE thread_id = ?)
          + (SELECT COUNT(*) FROM gmail_thread_initiatives WHERE thread_id = ?)
          + (SELECT COUNT(*) FROM gmail_thread_stakeholders WHERE thread_id = ?)
        "#,
    )
    .bind(thread_id)
    .bind(thread_id)
    .bind(thread_id)
    .fetch_one(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(count > 0)
}

pub async fn record_work_mail_agent_event(
    pool: &SqlitePool,
    thread_id: Option<&str>,
    event_kind: &str,
    actor: &str,
    summary: &str,
    reason: Value,
    payload: Value,
    undo_payload: Option<Value>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO gmail_work_mail_agent_events
          (id, thread_id, event_kind, actor, summary, reason_json, payload_json,
           undo_payload_json, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(Ulid::new().to_string())
    .bind(thread_id)
    .bind(event_kind)
    .bind(actor)
    .bind(summary)
    .bind(reason.to_string())
    .bind(payload.to_string())
    .bind(undo_payload.map(|value| value.to_string()))
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

fn checkpoint_for_messages<'a, F>(
    messages: &'a [GmailMessageRecord],
    keep: F,
) -> Option<(&'a str, i64)>
where
    F: Fn(&GmailMessageRecord) -> bool,
{
    messages
        .iter()
        .filter(|message| keep(message))
        .filter_map(|message| {
            message
                .internal_date_ts
                .or(message.date_ts)
                .map(|message_at| (message.message_id.as_str(), message_at))
        })
        .max_by_key(|(_, message_at)| *message_at)
}

async fn work_mail_review_row(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Option<WorkMailReviewRow>, String> {
    sqlx::query_as::<_, WorkMailReviewRow>(
        r#"
        SELECT thread_id, review_state, trace_seen_at, seen_through_message_id,
               seen_through_message_at, reviewed_through_message_id,
               reviewed_through_message_at, handled_at, deferred_until,
               reopened_at, updated_at
        FROM gmail_work_mail_thread_reviews
        WHERE thread_id = ?
        "#,
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::db::sql_error)
}

async fn ensure_work_mail_review_row(pool: &SqlitePool, thread_id: &str) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO gmail_work_mail_thread_reviews
          (thread_id, review_state, updated_at)
        VALUES (?, 'unreviewed', ?)
        "#,
    )
    .bind(thread_id)
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

pub async fn work_mail_review_summary(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<WorkMailReviewSummary, String> {
    let row = work_mail_review_row(pool, thread_id).await?;
    let latest_inbound =
        checkpoint_for_messages(&load_message_rows(pool, thread_id).await?, |message| {
            !message.is_sent && !message.is_draft
        })
        .map(|(message_id, message_at)| (message_id.to_string(), message_at));
    let checkpoint_at = row.as_ref().and_then(|value| {
        value
            .reviewed_through_message_at
            .or(value.seen_through_message_at)
    });
    let new_since_review = match (latest_inbound.as_ref(), checkpoint_at) {
        (Some((_, latest_at)), Some(checkpoint_at)) => *latest_at > checkpoint_at,
        _ => false,
    };

    Ok(match row {
        Some(row) => WorkMailReviewSummary {
            thread_id: row.thread_id,
            review_state: WorkMailReviewState::from_db(&row.review_state),
            trace_seen_at: row.trace_seen_at.clone(),
            seen: WorkMailSeenCheckpoint {
                message_id: row.seen_through_message_id,
                message_at: row.seen_through_message_at,
                seen_at: row.trace_seen_at,
            },
            reviewed_through_message_id: row.reviewed_through_message_id,
            reviewed_through_message_at: row.reviewed_through_message_at,
            handled_at: row.handled_at,
            deferred_until: row.deferred_until,
            reopened_at: row.reopened_at,
            updated_at: Some(row.updated_at),
            new_since_review,
        },
        None => WorkMailReviewSummary {
            thread_id: thread_id.to_string(),
            review_state: WorkMailReviewState::Unreviewed,
            trace_seen_at: None,
            seen: WorkMailSeenCheckpoint {
                message_id: None,
                message_at: None,
                seen_at: None,
            },
            reviewed_through_message_id: None,
            reviewed_through_message_at: None,
            handled_at: None,
            deferred_until: None,
            reopened_at: None,
            updated_at: None,
            new_since_review,
        },
    })
}

pub async fn mark_work_mail_thread_seen(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<WorkMailReviewSummary, String> {
    let messages = load_message_rows(pool, thread_id).await?;
    let latest = checkpoint_for_messages(&messages, |message| !message.is_draft)
        .map(|(message_id, message_at)| (message_id.to_string(), message_at));
    ensure_work_mail_review_row(pool, thread_id).await?;
    let current = work_mail_review_row(pool, thread_id)
        .await?
        .ok_or_else(|| "work mail review row not found".to_string())?;
    let should_advance = latest.as_ref().is_some_and(|(message_id, message_at)| {
        current.seen_through_message_id.as_deref() != Some(message_id.as_str())
            || current.seen_through_message_at != Some(*message_at)
    }) || current.trace_seen_at.is_none();
    if should_advance {
        let now = now_utc();
        sqlx::query(
            r#"
            UPDATE gmail_work_mail_thread_reviews
            SET trace_seen_at = ?,
                seen_through_message_id = ?,
                seen_through_message_at = ?,
                updated_at = ?
            WHERE thread_id = ?
            "#,
        )
        .bind(&now)
        .bind(latest.as_ref().map(|(message_id, _)| message_id))
        .bind(latest.as_ref().map(|(_, message_at)| *message_at))
        .bind(&now)
        .bind(thread_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
        record_work_mail_agent_event(
            pool,
            Some(thread_id),
            "seen",
            "user",
            "Opened thread in Trace.",
            json!(["Thread body opened in Work Mail detail."]),
            json!({
                "seen_through_message_id": latest.as_ref().map(|(message_id, _)| message_id),
                "seen_through_message_at": latest.as_ref().map(|(_, message_at)| *message_at),
            }),
            None,
        )
        .await?;
    }
    work_mail_review_summary(pool, thread_id).await
}

pub async fn set_work_mail_review_state(
    pool: &SqlitePool,
    thread_id: &str,
    update: WorkMailReviewUpdate,
) -> Result<WorkMailReviewSummary, String> {
    let messages = load_message_rows(pool, thread_id).await?;
    if messages.is_empty() {
        return Err("gmail thread not found".to_string());
    }
    let reviewed_checkpoint =
        checkpoint_for_messages(&messages, |message| !message.is_sent && !message.is_draft)
            .or_else(|| checkpoint_for_messages(&messages, |message| !message.is_draft))
            .map(|(message_id, message_at)| (message_id.to_string(), message_at));
    ensure_work_mail_review_row(pool, thread_id).await?;

    let now = now_utc();
    let handled = update.state.is_handled();
    let deferred_until = if update.state == WorkMailReviewState::Deferred {
        update.deferred_until.clone()
    } else {
        None
    };
    sqlx::query(
        r#"
        UPDATE gmail_work_mail_thread_reviews
        SET review_state = ?,
            reviewed_through_message_id = CASE WHEN ? THEN ? ELSE reviewed_through_message_id END,
            reviewed_through_message_at = CASE WHEN ? THEN ? ELSE reviewed_through_message_at END,
            handled_at = CASE WHEN ? THEN ? ELSE NULL END,
            deferred_until = ?,
            reopened_at = CASE WHEN ? THEN ? ELSE reopened_at END,
            updated_at = ?
        WHERE thread_id = ?
        "#,
    )
    .bind(update.state.as_str())
    .bind(handled)
    .bind(
        reviewed_checkpoint
            .as_ref()
            .map(|(message_id, _)| message_id),
    )
    .bind(handled)
    .bind(
        reviewed_checkpoint
            .as_ref()
            .map(|(_, message_at)| *message_at),
    )
    .bind(handled)
    .bind(&now)
    .bind(&deferred_until)
    .bind(update.state == WorkMailReviewState::Unreviewed)
    .bind(&now)
    .bind(&now)
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    record_work_mail_agent_event(
        pool,
        Some(thread_id),
        update.state.as_str(),
        "user",
        &format!("Set Work Mail review state to {}.", update.state.as_str()),
        json!(["Explicit Work Mail review action."]),
        json!({
            "review_state": update.state.as_str(),
            "reviewed_through_message_id": reviewed_checkpoint.as_ref().map(|(message_id, _)| message_id),
            "reviewed_through_message_at": reviewed_checkpoint.as_ref().map(|(_, message_at)| *message_at),
            "deferred_until": deferred_until,
        }),
        None,
    )
    .await?;
    work_mail_review_summary(pool, thread_id).await
}

pub async fn defer_work_mail_thread(
    pool: &SqlitePool,
    thread_id: &str,
    deferred_until: Option<String>,
) -> Result<WorkMailReviewSummary, String> {
    set_work_mail_review_state(
        pool,
        thread_id,
        WorkMailReviewUpdate {
            state: WorkMailReviewState::Deferred,
            deferred_until,
        },
    )
    .await
}

pub async fn reopen_work_mail_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<WorkMailReviewSummary, String> {
    set_work_mail_review_state(
        pool,
        thread_id,
        WorkMailReviewUpdate {
            state: WorkMailReviewState::Unreviewed,
            deferred_until: None,
        },
    )
    .await
}

pub async fn refresh_work_mail_review_state(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<(), String> {
    let current = work_mail_review_row(pool, thread_id).await?;
    let messages = load_message_rows(pool, thread_id).await?;
    let latest_inbound =
        checkpoint_for_messages(&messages, |message| !message.is_sent && !message.is_draft)
            .map(|(message_id, message_at)| (message_id.to_string(), message_at));
    let latest_sent = checkpoint_for_messages(&messages, |message| message.is_sent)
        .map(|(message_id, message_at)| (message_id.to_string(), message_at));
    let sent_reply_after_inbound = match (latest_sent.as_ref(), latest_inbound.as_ref()) {
        (Some((_, sent_at)), Some((_, inbound_at))) => sent_at > inbound_at,
        _ => false,
    };

    let Some(current) = current else {
        if sent_reply_after_inbound {
            set_work_mail_review_state(
                pool,
                thread_id,
                WorkMailReviewUpdate {
                    state: WorkMailReviewState::Replied,
                    deferred_until: None,
                },
            )
            .await?;
        }
        return Ok(());
    };
    let current_state = WorkMailReviewState::from_db(&current.review_state);

    if current_state.is_handled()
        && latest_inbound.as_ref().is_some_and(|(_, latest_at)| {
            current
                .reviewed_through_message_at
                .map_or(true, |checkpoint_at| *latest_at > checkpoint_at)
        })
    {
        let now = now_utc();
        sqlx::query(
            r#"
            UPDATE gmail_work_mail_thread_reviews
            SET review_state = 'unreviewed',
                handled_at = NULL,
                deferred_until = NULL,
                reopened_at = ?,
                updated_at = ?
            WHERE thread_id = ?
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(thread_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
        record_work_mail_agent_event(
            pool,
            Some(thread_id),
            "reopened",
            "trace",
            "New inbound mail reopened a handled thread.",
            json!(["Inbound message arrived after the review checkpoint."]),
            json!({
                "previous_review_state": current.review_state,
                "latest_inbound_message_id": latest_inbound.as_ref().map(|(message_id, _)| message_id),
                "latest_inbound_message_at": latest_inbound.as_ref().map(|(_, message_at)| *message_at),
            }),
            None,
        )
        .await?;
        return Ok(());
    }

    if sent_reply_after_inbound && current_state != WorkMailReviewState::Replied {
        set_work_mail_review_state(
            pool,
            thread_id,
            WorkMailReviewUpdate {
                state: WorkMailReviewState::Replied,
                deferred_until: None,
            },
        )
        .await?;
    }
    Ok(())
}
pub async fn list_work_mail_threads(
    pool: &SqlitePool,
    query: WorkMailQuery,
) -> Result<Vec<GmailLocalThread>, String> {
    backfill_work_mail_dimensions(pool, 250).await?;
    if query.view == WorkMailViewId::AgentActivity {
        return Ok(Vec::new());
    }
    let limit = query.limit.unwrap_or(80).clamp(1, 200);
    let candidates = list_local_threads(
        pool,
        GmailThreadFilter {
            query: query.query.clone(),
            stakeholder_id: query.stakeholder_id.clone(),
            deliverable_id: query.deliverable_id.clone(),
            initiative_id: query.initiative_id.clone(),
            limit: Some((limit * 5).clamp(80, 500)),
            ..GmailThreadFilter::default()
        },
    )
    .await?;
    Ok(candidates
        .into_iter()
        .filter(|thread| work_mail_thread_matches(thread, &query))
        .take(limit as usize)
        .collect())
}

pub async fn work_mail_view_counts(pool: &SqlitePool) -> Result<WorkMailViewCounts, String> {
    backfill_work_mail_dimensions(pool, 500).await?;
    let candidates = list_local_threads(
        pool,
        GmailThreadFilter {
            limit: Some(500),
            ..GmailThreadFilter::default()
        },
    )
    .await?;
    let mut counts = WorkMailViewId::all_thread_views()
        .into_iter()
        .map(|view| WorkMailViewCount { view, count: 0 })
        .collect::<Vec<_>>();
    for thread in &candidates {
        for count in &mut counts {
            if work_mail_thread_matches(
                thread,
                &WorkMailQuery {
                    view: count.view,
                    ..WorkMailQuery::default()
                },
            ) {
                count.count += 1;
            }
        }
    }
    let activity_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM gmail_work_mail_agent_events")
            .fetch_one(pool)
            .await
            .map_err(crate::db::sql_error)?;
    counts.push(WorkMailViewCount {
        view: WorkMailViewId::AgentActivity,
        count: activity_count,
    });
    Ok(WorkMailViewCounts { counts })
}

pub async fn work_mail_brief(pool: &SqlitePool) -> Result<WorkMailBrief, String> {
    let counts = work_mail_view_counts(pool).await?;
    let read_count = |view| {
        counts
            .counts
            .iter()
            .find(|count| count.view == view)
            .map(|count| count.count)
            .unwrap_or(0)
    };
    let all_work = read_count(WorkMailViewId::AllWork);
    let needs_you = read_count(WorkMailViewId::NeedsMe);
    let candidates = list_local_threads(
        pool,
        GmailThreadFilter {
            limit: Some(500),
            ..GmailThreadFilter::default()
        },
    )
    .await?;
    let in_work_scope = candidates
        .iter()
        .filter(|thread| work_mail_is_work_scope(&thread.work_relevance))
        .collect::<Vec<_>>();
    let handled = in_work_scope
        .iter()
        .filter(|thread| work_mail_review_is_handled(&thread.trace_review_state))
        .count() as i64;
    let pending_links: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gmail_thread_link_suggestions WHERE status = 'pending'",
    )
    .fetch_one(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(WorkMailBrief {
        needs_you,
        handled_by_trace: handled.max((all_work - needs_you).max(0)),
        unread_work_mail: in_work_scope
            .iter()
            .filter(|thread| thread.has_unread)
            .count() as i64,
        unseen_in_trace: in_work_scope
            .iter()
            .filter(|thread| thread.trace_seen_at.is_none())
            .count() as i64,
        seen_unreviewed: in_work_scope
            .iter()
            .filter(|thread| {
                thread.trace_seen_at.is_some() && thread.trace_review_state == "unreviewed"
            })
            .count() as i64,
        action_review_queue: needs_you,
        waiting: in_work_scope
            .iter()
            .filter(|thread| thread.trace_review_state == "waiting")
            .count() as i64,
        deferred: in_work_scope
            .iter()
            .filter(|thread| thread.trace_review_state == "deferred")
            .count() as i64,
        handled,
        unlinked: read_count(WorkMailViewId::Unlinked),
        excluded: read_count(WorkMailViewId::Excluded),
        uncertain_links: pending_links,
        scope_domains: list_work_mail_domains(pool)
            .await?
            .into_iter()
            .filter(|domain| domain.enabled)
            .map(|domain| domain.domain)
            .collect(),
    })
}

pub async fn list_work_mail_domains(pool: &SqlitePool) -> Result<Vec<WorkMailDomain>, String> {
    let rows: Vec<(String, i64, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT domain, enabled, source, note, created_at, updated_at
           FROM gmail_work_domains
          ORDER BY enabled DESC, domain ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(domain, enabled, source, note, created_at, updated_at)| WorkMailDomain {
                domain,
                enabled: enabled != 0,
                source,
                note,
                created_at,
                updated_at,
            },
        )
        .collect())
}

pub async fn upsert_work_mail_domain(
    pool: &SqlitePool,
    input: UpsertWorkMailDomainInput,
) -> Result<WorkMailDomain, String> {
    let domain = normalize_work_mail_domain(&input.domain)?;
    let now = Utc::now().timestamp_millis();
    sqlx::query(
        r#"
        INSERT INTO gmail_work_domains (domain, enabled, source, note, created_at, updated_at)
        VALUES (?, ?, 'manual', ?, ?, ?)
        ON CONFLICT(domain) DO UPDATE SET
          enabled = excluded.enabled,
          source = 'manual',
          note = excluded.note,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(&domain)
    .bind(if input.enabled { 1_i64 } else { 0_i64 })
    .bind(&input.note)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    record_work_mail_agent_event(
        pool,
        None,
        "work_domain",
        "user",
        &format!("Updated Work Mail domain {domain}."),
        json!(["Work Mail domain allowlist changed by user."]),
        json!({ "domain": domain, "enabled": input.enabled }),
        None,
    )
    .await?;
    list_work_mail_domains(pool)
        .await?
        .into_iter()
        .find(|item| item.domain == domain)
        .ok_or_else(|| "work mail domain not found after save".to_string())
}

pub async fn delete_work_mail_domain(pool: &SqlitePool, domain: &str) -> Result<(), String> {
    let domain = normalize_work_mail_domain(domain)?;
    sqlx::query("DELETE FROM gmail_work_domains WHERE domain = ?")
        .bind(&domain)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    record_work_mail_agent_event(
        pool,
        None,
        "work_domain",
        "user",
        &format!("Removed Work Mail domain {domain}."),
        json!(["Work Mail domain allowlist changed by user."]),
        json!({ "domain": domain, "deleted": true }),
        None,
    )
    .await
}

pub async fn promote_work_mail_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<crate::gmail_intel::UserClassification, String> {
    set_work_mail_relevance_override(
        pool,
        thread_id,
        "promoted",
        "Promoted into Work Mail by user.",
        "promote",
    )
    .await
}

pub async fn restore_work_mail_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<crate::gmail_intel::UserClassification, String> {
    set_work_mail_relevance_override(
        pool,
        thread_id,
        "promoted",
        "Restored from recoverable Excluded queue by user.",
        "restore",
    )
    .await
}

pub async fn exclude_work_mail_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<crate::gmail_intel::UserClassification, String> {
    set_work_mail_relevance_override(
        pool,
        thread_id,
        "excluded",
        "Excluded from Work Mail by user.",
        "exclude",
    )
    .await
}

async fn set_work_mail_relevance_override(
    pool: &SqlitePool,
    thread_id: &str,
    work_relevance: &str,
    note: &str,
    event_kind: &str,
) -> Result<crate::gmail_intel::UserClassification, String> {
    let current = crate::gmail_intel::get_override(pool, thread_id).await?;
    let input = crate::gmail_intel::SetOverrideInput {
        thread_id: thread_id.to_string(),
        category: current.as_ref().and_then(|value| value.category.clone()),
        priority: current.as_ref().and_then(|value| value.priority.clone()),
        intent: current.as_ref().and_then(|value| value.intent.clone()),
        action_required: current.as_ref().and_then(|value| value.action_required),
        thread_state: current
            .as_ref()
            .and_then(|value| value.thread_state.clone()),
        work_relevance: Some(work_relevance.to_string()),
        attention_state: current
            .as_ref()
            .and_then(|value| value.attention_state.clone()),
        message_type: current
            .as_ref()
            .and_then(|value| value.message_type.clone()),
        note: current
            .as_ref()
            .and_then(|value| value.note.clone())
            .or_else(|| Some(note.to_string())),
    };
    let classification = crate::gmail_intel::set_override(pool, input).await?;
    record_work_mail_agent_event(
        pool,
        Some(thread_id),
        event_kind,
        "user",
        note,
        json!(["Explicit thread-level Work Mail relevance override."]),
        json!({ "work_relevance": work_relevance }),
        None,
    )
    .await?;
    Ok(classification)
}

pub async fn list_work_mail_agent_events(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<WorkMailAgentEvent>, String> {
    let rows: Vec<(
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
    )> = sqlx::query_as(
        r#"
        SELECT id, thread_id, event_kind, actor, summary, reason_json, payload_json,
               undo_payload_json, created_at
        FROM gmail_work_mail_agent_events
        ORDER BY created_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                thread_id,
                event_kind,
                actor,
                summary,
                reason_json,
                payload_json,
                undo_payload_json,
                created_at,
            )| WorkMailAgentEvent {
                id,
                thread_id,
                event_kind,
                actor,
                summary,
                reason: serde_json::from_str(&reason_json).unwrap_or_else(|_| json!([])),
                payload: serde_json::from_str(&payload_json).unwrap_or_else(|_| json!({})),
                undo_payload: undo_payload_json
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok()),
                created_at,
            },
        )
        .collect())
}

async fn backfill_work_mail_dimensions(pool: &SqlitePool, limit: i64) -> Result<(), String> {
    let thread_ids: Vec<String> = sqlx::query_scalar(
        "SELECT thread_id
           FROM gmail_threads
          WHERE work_mail_updated_at IS NULL
          ORDER BY last_message_at DESC
          LIMIT ?",
    )
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    for thread_id in thread_ids {
        refresh_work_mail_dimensions(pool, &thread_id).await?;
    }
    Ok(())
}

fn work_mail_thread_matches(thread: &GmailLocalThread, query: &WorkMailQuery) -> bool {
    let in_work_scope = work_mail_is_work_scope(&thread.work_relevance);
    let matches_view = match query.view {
        WorkMailViewId::AllWork => in_work_scope,
        WorkMailViewId::NeedsMe => in_work_scope && work_mail_needs_me_reason(thread).is_some(),
        WorkMailViewId::Projects => in_work_scope && !thread.linked_initiatives.is_empty(),
        WorkMailViewId::Deliverables => in_work_scope && !thread.linked_deliverables.is_empty(),
        WorkMailViewId::Stakeholders => in_work_scope && !thread.linked_stakeholders.is_empty(),
        WorkMailViewId::Files => in_work_scope && !thread.artifact_urls.is_empty(),
        WorkMailViewId::Meetings => in_work_scope && thread.message_type == "meeting",
        WorkMailViewId::Unlinked => {
            in_work_scope
                && thread.linked_initiatives.is_empty()
                && thread.linked_deliverables.is_empty()
                && thread.linked_stakeholders.is_empty()
        }
        WorkMailViewId::Excluded => {
            matches!(
                thread.work_relevance.as_str(),
                "excluded" | "non_work" | "unknown"
            )
        }
        WorkMailViewId::AgentActivity => false,
    };
    matches_view
        && query.work_relevance.as_ref().map_or(true, |value| {
            thread.work_relevance.eq_ignore_ascii_case(value)
        })
        && query.attention_state.as_ref().map_or(true, |value| {
            thread.attention_state.eq_ignore_ascii_case(value)
        })
        && query.message_type.as_ref().map_or(true, |value| {
            thread.message_type.eq_ignore_ascii_case(value)
        })
        && query
            .unread_only
            .map_or(true, |value| !value || thread.has_unread)
        && query.gmail_unread.map_or(true, |value| {
            if value {
                thread.has_unread
            } else {
                !thread.has_unread
            }
        })
        && query.trace_unseen.map_or(true, |value| {
            if value {
                thread.trace_seen_at.is_none()
            } else {
                thread.trace_seen_at.is_some()
            }
        })
        && query.seen_unreviewed.map_or(true, |value| {
            if value {
                thread.trace_seen_at.is_some() && thread.trace_review_state == "unreviewed"
            } else {
                true
            }
        })
        && query.review_state.as_ref().map_or(true, |value| {
            thread.trace_review_state.eq_ignore_ascii_case(value)
        })
        && query
            .has_artifact
            .map_or(true, |value| value == !thread.artifact_urls.is_empty())
        && query.sender_domain.as_ref().map_or(true, |value| {
            sender_domain_matches(&thread.last_from_email, value)
        })
}

fn work_mail_review_is_handled(value: &str) -> bool {
    matches!(
        value,
        "reviewed" | "deferred" | "waiting" | "resolved" | "replied"
    )
}

pub fn work_mail_needs_me_reason(thread: &GmailLocalThread) -> Option<String> {
    if work_mail_review_is_handled(&thread.trace_review_state) && !thread.new_since_review {
        return None;
    }
    if thread.new_since_review {
        return Some("New inbound mail arrived after the last Trace checkpoint.".to_string());
    }
    if thread.has_unread {
        return Some("Unread work mail in Gmail.".to_string());
    }
    if thread.trace_seen_at.is_some() && thread.trace_review_state == "unreviewed" {
        return Some("Opened in Trace and still unreviewed.".to_string());
    }
    match thread.attention_state.as_str() {
        "needs_me" => Some("Trace inferred an action needs you.".to_string()),
        "review" => Some("Trace inferred this thread needs review.".to_string()),
        _ => None,
    }
}

fn work_mail_is_work_scope(value: &str) -> bool {
    matches!(value, "work" | "linked_external" | "promoted")
}

fn sender_domain_matches(email: &str, domain: &str) -> bool {
    let expected = domain.trim().trim_start_matches('@').to_ascii_lowercase();
    email
        .rsplit_once('@')
        .map(|(_, value)| value.trim().eq_ignore_ascii_case(&expected))
        .unwrap_or(false)
}

fn normalize_work_mail_domain(value: &str) -> Result<String, String> {
    let domain = value
        .trim()
        .trim_start_matches('@')
        .trim()
        .to_ascii_lowercase();
    if domain.is_empty()
        || domain.contains('@')
        || domain.chars().any(char::is_whitespace)
        || !domain.contains('.')
    {
        return Err("work mail domain must look like example.com".to_string());
    }
    Ok(domain)
}

impl WorkMailViewId {
    fn all_thread_views() -> [WorkMailViewId; 9] {
        [
            WorkMailViewId::AllWork,
            WorkMailViewId::NeedsMe,
            WorkMailViewId::Projects,
            WorkMailViewId::Deliverables,
            WorkMailViewId::Stakeholders,
            WorkMailViewId::Files,
            WorkMailViewId::Meetings,
            WorkMailViewId::Unlinked,
            WorkMailViewId::Excluded,
        ]
    }
}

pub struct ThreadCategory {
    pub category: String,
    pub priority: String,
    pub confidence: f64,
    pub reasons: Vec<String>,
}

pub struct WorkMailDimensions {
    pub work_relevance: String,
    pub work_relevance_reasons: Vec<String>,
    pub work_relevance_confidence: f64,
    pub attention_state: String,
    pub attention_reasons: Vec<String>,
    pub attention_confidence: f64,
    pub message_type: String,
    pub message_type_reasons: Vec<String>,
    pub message_type_confidence: f64,
}
