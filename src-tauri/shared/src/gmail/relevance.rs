use std::collections::BTreeSet;

use sqlx::SqlitePool;

use super::{email_domain, is_artifact_url, load_message_rows, strip_html};
use super::models::*;

pub async fn load_relevance_context(pool: &SqlitePool) -> Result<RelevanceContext, String> {
    let mut context = RelevanceContext::default();
    let emails: Vec<String> =
        sqlx::query_scalar("SELECT lower(email) FROM stakeholders WHERE email != ''")
            .fetch_all(pool)
            .await
            .map_err(crate::db::sql_error)?;
    for email in emails {
        if let Some(domain) = email_domain(&email) {
            context.work_domains.insert(domain);
        }
        context.known_emails.insert(email);
    }

    let terms: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT title FROM deliverables WHERE state != 'killed'
        UNION
        SELECT title FROM initiatives WHERE status IN ('live', 'paused')
        UNION
        SELECT name FROM stakeholders WHERE name != ''
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    let mut seen = BTreeSet::new();
    for term in terms {
        for token in relevance_terms(&term) {
            if seen.insert(token.clone()) {
                context.terms.push(token);
            }
        }
    }
    Ok(context)
}

fn relevance_terms(value: &str) -> Vec<String> {
    let lower = value.to_lowercase();
    let mut terms = Vec::new();
    let trimmed = lower.trim();
    if trimmed.len() >= 4 {
        terms.push(trimmed.to_string());
    }
    for token in lower
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::trim)
        .filter(|token| token.len() >= 4)
    {
        if !matches!(
            token,
            "this" | "that" | "with" | "from" | "about" | "into" | "your" | "have" | "will"
        ) {
            terms.push(token.to_string());
        }
    }
    terms
}

pub fn thread_is_relevant(messages: &[ParsedMessage], context: &RelevanceContext) -> bool {
    if messages.iter().any(|message| message.is_sent) {
        return true;
    }
    if messages.iter().any(message_has_priority_signal) {
        return true;
    }
    if messages
        .iter()
        .any(|message| !message.attachments.is_empty())
    {
        return true;
    }
    if messages
        .iter()
        .any(|message| message.artifact_urls.iter().any(|url| is_artifact_url(url)))
    {
        return true;
    }
    if messages.iter().any(|message| {
        message_has_known_contact(message, &context.known_emails)
            || message_from_known_work_domain(message, &context.work_domains)
    }) {
        return true;
    }

    if !context.terms.is_empty() {
        let haystack = messages
            .iter()
            .map(|message| {
                format!(
                    "{}\n{}\n{}\n{}",
                    message.subject,
                    message.snippet,
                    message.plain_body,
                    strip_html(&message.html_body)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        if context.terms.iter().any(|term| haystack.contains(term)) {
            return true;
        }
    }

    let all_low_signal = messages.iter().all(message_is_low_signal);
    messages
        .iter()
        .any(|message| label_ids_contain(&message.label_ids, "INBOX") && !all_low_signal)
}

pub fn is_blocked_spam_or_trash(message: &ParsedMessage) -> bool {
    label_ids_contain(&message.label_ids, "SPAM") || label_ids_contain(&message.label_ids, "TRASH")
}

fn message_has_priority_signal(message: &ParsedMessage) -> bool {
    label_ids_contain(&message.label_ids, "IMPORTANT")
        || label_ids_contain(&message.label_ids, "STARRED")
}

fn message_has_known_contact(message: &ParsedMessage, known_emails: &BTreeSet<String>) -> bool {
    std::iter::once(&message.from)
        .chain(message.to.iter())
        .chain(message.cc.iter())
        .chain(message.bcc.iter())
        .any(|address| known_emails.contains(&address.email.to_lowercase()))
}

fn message_from_known_work_domain(
    message: &ParsedMessage,
    work_domains: &BTreeSet<String>,
) -> bool {
    email_domain(&message.from.email)
        .map(|domain| work_domains.contains(&domain))
        .unwrap_or(false)
}

fn message_is_low_signal(message: &ParsedMessage) -> bool {
    let labels = &message.label_ids;
    let low_signal_category = label_ids_contain(labels, "CATEGORY_PROMOTIONS")
        || label_ids_contain(labels, "CATEGORY_SOCIAL")
        || label_ids_contain(labels, "CATEGORY_FORUMS")
        || label_ids_contain(labels, "CATEGORY_UPDATES");
    let sender = message.from.email.to_lowercase();
    let headers = message.headers_json.to_lowercase();
    low_signal_category
        || sender.contains("no-reply")
        || sender.contains("noreply")
        || sender.contains("do-not-reply")
        || sender.contains("donotreply")
        || sender.contains("notification")
        || sender.contains("newsletter")
        || sender.contains("marketing")
        || sender.contains("automoderator")
        || sender.contains("mailer-daemon")
        || headers.contains("list-unsubscribe")
        || headers.contains("auto-submitted")
        || headers.contains("precedence")
        || headers.contains("bulk")
}

fn label_ids_contain(label_ids: &[String], needle: &str) -> bool {
    label_ids.iter().any(|label| label == needle)
}

pub async fn delete_local_thread(pool: &SqlitePool, thread_id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM gmail_thread_search WHERE thread_id = ?")
        .bind(thread_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    sqlx::query("DELETE FROM gmail_threads WHERE thread_id = ?")
        .bind(thread_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    Ok(())
}

pub async fn purge_blocked_threads(pool: &SqlitePool) -> Result<i64, String> {
    let blocked: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT thread_id
        FROM gmail_messages
        WHERE label_ids_json LIKE '%"SPAM"%'
           OR label_ids_json LIKE '%"TRASH"%'
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    let count = blocked.len() as i64;
    for thread_id in blocked {
        delete_local_thread(pool, &thread_id).await?;
    }
    Ok(count)
}

pub async fn purge_irrelevant_threads(
    pool: &SqlitePool,
    relevance: &RelevanceContext,
) -> Result<i64, String> {
    let candidate_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT t.thread_id
        FROM gmail_threads t
        WHERE NOT EXISTS (
            SELECT 1 FROM gmail_thread_deliverables td WHERE td.thread_id = t.thread_id
        )
          AND NOT EXISTS (
            SELECT 1 FROM gmail_thread_initiatives ti WHERE ti.thread_id = t.thread_id
        )
          AND NOT EXISTS (
            SELECT 1 FROM gmail_thread_captures tc WHERE tc.thread_id = t.thread_id
        )
          AND NOT EXISTS (
            SELECT 1 FROM gmail_attachments a WHERE a.thread_id = t.thread_id
        )
          AND NOT EXISTS (
            SELECT 1 FROM gmail_links l WHERE l.thread_id = t.thread_id AND l.kind != 'url'
        )
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;

    let mut purged = 0i64;
    for thread_id in candidate_ids {
        let messages = load_message_rows(pool, &thread_id).await?;
        if messages.is_empty() || !local_thread_is_relevant(&messages, relevance) {
            delete_local_thread(pool, &thread_id).await?;
            purged += 1;
        }
    }
    Ok(purged)
}

fn local_thread_is_relevant(messages: &[GmailMessageRecord], context: &RelevanceContext) -> bool {
    if messages.iter().any(|message| message.is_sent) {
        return true;
    }
    if messages.iter().any(local_message_has_priority_signal) {
        return true;
    }
    if messages.iter().any(|message| {
        record_has_known_contact(message, &context.known_emails)
            || record_from_known_work_domain(message, &context.work_domains)
    }) {
        return true;
    }
    if !context.terms.is_empty() {
        let haystack = messages
            .iter()
            .map(|message| {
                format!(
                    "{}\n{}\n{}\n{}",
                    message.subject,
                    message.snippet,
                    message.plain_body,
                    strip_html(&message.html_body)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        if context.terms.iter().any(|term| haystack.contains(term)) {
            return true;
        }
    }

    let all_low_signal = messages.iter().all(local_message_is_low_signal);
    messages
        .iter()
        .any(|message| label_ids_contain(&message.label_ids, "INBOX") && !all_low_signal)
}

fn local_message_has_priority_signal(message: &GmailMessageRecord) -> bool {
    label_ids_contain(&message.label_ids, "IMPORTANT")
        || label_ids_contain(&message.label_ids, "STARRED")
}

fn record_has_known_contact(message: &GmailMessageRecord, known_emails: &BTreeSet<String>) -> bool {
    std::iter::once(EmailAddress {
        name: message.from_name.clone(),
        email: message.from_email.clone(),
    })
    .chain(message.to.iter().cloned())
    .chain(message.cc.iter().cloned())
    .chain(message.bcc.iter().cloned())
    .any(|address| known_emails.contains(&address.email.to_lowercase()))
}

fn record_from_known_work_domain(
    message: &GmailMessageRecord,
    work_domains: &BTreeSet<String>,
) -> bool {
    email_domain(&message.from_email)
        .map(|domain| work_domains.contains(&domain))
        .unwrap_or(false)
}

pub fn local_message_is_low_signal(message: &GmailMessageRecord) -> bool {
    let labels = &message.label_ids;
    let low_signal_category = label_ids_contain(labels, "CATEGORY_PROMOTIONS")
        || label_ids_contain(labels, "CATEGORY_SOCIAL")
        || label_ids_contain(labels, "CATEGORY_FORUMS")
        || label_ids_contain(labels, "CATEGORY_UPDATES");
    let sender = message.from_email.to_lowercase();
    let subject = message.subject.to_lowercase();
    low_signal_category
        || sender.contains("no-reply")
        || sender.contains("noreply")
        || sender.contains("do-not-reply")
        || sender.contains("donotreply")
        || sender.contains("notification")
        || sender.contains("newsletter")
        || sender.contains("marketing")
        || sender.contains("automoderator")
        || sender.contains("mailer-daemon")
        || subject.contains("unsubscribe")
        || subject.contains("newsletter")
        || subject.contains("digest")
        || subject.contains("exclusive deal")
        || subject.contains("subscribe")
}
