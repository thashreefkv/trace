use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use ulid::Ulid;

use super::models::*;
use super::{
    delete_local_thread, get_local_thread, get_valid_access_token, now_utc, parse_string_vec,
    rebuild_thread_aggregate, record_work_mail_agent_event, request_json,
    set_work_mail_review_state, to_json_string,
};

pub async fn send_email(
    dir: &Path,
    pool: &SqlitePool,
    input: GmailSendInput,
) -> Result<GmailSendResult, String> {
    let token = get_valid_access_token(dir).await?;

    // Resolve attachment file paths: from explicit list + from local draft.
    let mut attachment_paths: Vec<std::path::PathBuf> = input
        .attachment_paths
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
    if let Some(draft_id) = input.draft_id.as_deref() {
        let attachments = crate::local_drafts::list_attachments(pool, draft_id).await?;
        for att in attachments {
            attachment_paths.push(std::path::PathBuf::from(&att.file_path));
        }
    }

    let raw = build_mime_message(&input, &attachment_paths)?;
    #[derive(Deserialize)]
    struct SendResponse {
        id: String,
        #[serde(rename = "threadId")]
        thread_id: Option<String>,
    }

    let body = if let Some(thread_id) = input.thread_id.as_deref() {
        json!({ "raw": raw, "threadId": thread_id })
    } else {
        json!({ "raw": raw })
    };
    let response: SendResponse = request_json(
        reqwest::Client::new()
            .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
            .bearer_auth(token)
            .json(&body),
        "gmail send request",
    )
    .await?;

    let result = GmailSendResult {
        message_id: response.id,
        thread_id: response.thread_id,
    };
    if let Some(thread_id) = input.thread_id.as_deref().or(result.thread_id.as_deref()) {
        let _ = set_work_mail_review_state(
            pool,
            thread_id,
            WorkMailReviewUpdate {
                state: WorkMailReviewState::Replied,
                deferred_until: None,
            },
        )
        .await;
    }
    Ok(result)
}

pub async fn archive_thread(dir: &Path, pool: &SqlitePool, thread_id: &str) -> Result<(), String> {
    modify_thread_labels(dir, thread_id, &[], &["INBOX"]).await?;
    update_local_thread_labels(pool, thread_id, &[], &["INBOX"]).await?;
    Ok(())
}

pub async fn move_thread_to_spam(
    dir: &Path,
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<(), String> {
    modify_thread_labels(dir, thread_id, &["SPAM"], &["INBOX"]).await?;
    delete_local_thread(pool, thread_id).await?;
    Ok(())
}

pub async fn mark_thread_important(
    dir: &Path,
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<GmailThreadDetail, String> {
    modify_thread_labels(dir, thread_id, &["IMPORTANT"], &[]).await?;
    ensure_system_label(pool, "IMPORTANT", "IMPORTANT").await?;
    update_local_thread_labels(pool, thread_id, &["IMPORTANT"], &[]).await?;
    get_local_thread(pool, thread_id).await
}

pub async fn star_thread(
    dir: &Path,
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<GmailThreadDetail, String> {
    modify_thread_labels(dir, thread_id, &["STARRED"], &[]).await?;
    ensure_system_label(pool, "STARRED", "STARRED").await?;
    update_local_thread_labels(pool, thread_id, &["STARRED"], &[]).await?;
    get_local_thread(pool, thread_id).await
}

pub async fn mark_thread_read_in_gmail(
    dir: &Path,
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<GmailThreadDetail, String> {
    modify_thread_labels(dir, thread_id, &[], &["UNREAD"]).await?;
    update_local_thread_labels(pool, thread_id, &[], &["UNREAD"]).await?;
    record_work_mail_agent_event(
        pool,
        Some(thread_id),
        "gmail_mark_read",
        "user",
        "Marked thread read in Gmail.",
        json!(["Explicit Gmail read writeback from Work Mail."]),
        json!({ "gmail_label_removed": "UNREAD" }),
        None,
    )
    .await?;
    get_local_thread(pool, thread_id).await
}

pub async fn mark_thread_unread_in_gmail(
    dir: &Path,
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<GmailThreadDetail, String> {
    modify_thread_labels(dir, thread_id, &["UNREAD"], &[]).await?;
    ensure_system_label(pool, "UNREAD", "UNREAD").await?;
    update_local_thread_labels(pool, thread_id, &["UNREAD"], &[]).await?;
    record_work_mail_agent_event(
        pool,
        Some(thread_id),
        "gmail_mark_unread",
        "user",
        "Marked thread unread in Gmail.",
        json!(["Explicit Gmail unread writeback from Work Mail."]),
        json!({ "gmail_label_added": "UNREAD" }),
        None,
    )
    .await?;
    get_local_thread(pool, thread_id).await
}

async fn modify_thread_labels(
    dir: &Path,
    thread_id: &str,
    add_label_ids: &[&str],
    remove_label_ids: &[&str],
) -> Result<(), String> {
    let token = get_valid_access_token(dir).await?;
    let body = json!({
        "addLabelIds": add_label_ids,
        "removeLabelIds": remove_label_ids,
    });
    let result: Result<serde_json::Value, String> = request_json(
        reqwest::Client::new()
            .post(format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/threads/{thread_id}/modify"
            ))
            .bearer_auth(token)
            .json(&body),
        "gmail thread label modify request",
    )
    .await;

    result
        .map(|_| ())
        .map_err(|error| {
            if error.contains("insufficientPermissions")
                || error.contains("Request had insufficient authentication scopes")
            {
                "Gmail needs the modify permission for archive/spam actions. Disconnect and reconnect Gmail once, then try again.".to_string()
            } else {
                error
            }
        })
}

pub(super) async fn ensure_system_label(
    pool: &SqlitePool,
    label_id: &str,
    name: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO gmail_labels (gmail_label_id, name, type, color, updated_at)
        VALUES (?, ?, 'system', NULL, ?)
        ON CONFLICT(gmail_label_id) DO UPDATE SET name = excluded.name, updated_at = excluded.updated_at
        "#,
    )
    .bind(label_id)
    .bind(name)
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

async fn update_local_thread_labels(
    pool: &SqlitePool,
    thread_id: &str,
    add_label_ids: &[&str],
    remove_label_ids: &[&str],
) -> Result<(), String> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT message_id, label_ids_json FROM gmail_messages WHERE thread_id = ?")
            .bind(thread_id)
            .fetch_all(pool)
            .await
            .map_err(crate::db::sql_error)?;

    for (message_id, labels_json) in rows {
        let mut labels = parse_string_vec(&labels_json)
            .into_iter()
            .filter(|label| {
                !remove_label_ids
                    .iter()
                    .any(|remove| *remove == label.as_str())
            })
            .collect::<BTreeSet<_>>();
        for label in add_label_ids {
            labels.insert((*label).to_string());
        }
        let labels = labels.into_iter().collect::<Vec<_>>();
        let unread = labels.iter().any(|label| label == "UNREAD");
        sqlx::query(
            "UPDATE gmail_messages SET label_ids_json = ?, is_unread = ? WHERE message_id = ?",
        )
        .bind(to_json_string(&labels))
        .bind(unread)
        .bind(message_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    }

    rebuild_thread_aggregate(pool, thread_id, &now_utc()).await
}

fn build_mime_message(
    input: &GmailSendInput,
    attachment_paths: &[std::path::PathBuf],
) -> Result<String, String> {
    if input.to.is_empty() {
        return Err("at least one recipient is required".to_string());
    }
    let subject = sanitize_header(&input.subject);
    if subject.trim().is_empty() {
        return Err("subject is required".to_string());
    }
    if input.body.trim().is_empty() {
        return Err("body is required".to_string());
    }

    // ---- Headers ----------------------------------------------------------
    let mut headers = String::new();
    headers.push_str(&format!("To: {}\r\n", input.to.join(", ")));
    if !input.cc.is_empty() {
        headers.push_str(&format!("Cc: {}\r\n", input.cc.join(", ")));
    }
    if !input.bcc.is_empty() {
        headers.push_str(&format!("Bcc: {}\r\n", input.bcc.join(", ")));
    }
    headers.push_str(&format!("Subject: {subject}\r\n"));
    headers.push_str("MIME-Version: 1.0\r\n");

    let has_html = input
        .body_html
        .as_deref()
        .map(|h| !h.trim().is_empty())
        .unwrap_or(false);
    let has_attachments = !attachment_paths.is_empty();

    // ---- Body assembly ----------------------------------------------------
    let body_part = if has_html {
        // multipart/alternative wrapping text + html
        let alt_boundary = format!("alt_{}", Ulid::new());
        let mut s = String::new();
        s.push_str(&format!(
            "Content-Type: multipart/alternative; boundary=\"{alt_boundary}\"\r\n\r\n"
        ));
        s.push_str(&format!("--{alt_boundary}\r\n"));
        s.push_str("Content-Type: text/plain; charset=\"UTF-8\"\r\n");
        s.push_str("Content-Transfer-Encoding: 8bit\r\n\r\n");
        s.push_str(&input.body);
        s.push_str("\r\n\r\n");
        s.push_str(&format!("--{alt_boundary}\r\n"));
        s.push_str("Content-Type: text/html; charset=\"UTF-8\"\r\n");
        s.push_str("Content-Transfer-Encoding: 8bit\r\n\r\n");
        s.push_str(input.body_html.as_deref().unwrap_or(""));
        s.push_str("\r\n\r\n");
        s.push_str(&format!("--{alt_boundary}--\r\n"));
        s
    } else {
        let mut s = String::new();
        s.push_str("Content-Type: text/plain; charset=\"UTF-8\"\r\n");
        s.push_str("Content-Transfer-Encoding: 8bit\r\n\r\n");
        s.push_str(&input.body);
        s.push_str("\r\n");
        s
    };

    let message = if has_attachments {
        // multipart/mixed wrapping body_part + each attachment
        let mixed_boundary = format!("mix_{}", Ulid::new());
        let mut s = headers;
        s.push_str(&format!(
            "Content-Type: multipart/mixed; boundary=\"{mixed_boundary}\"\r\n\r\n"
        ));
        s.push_str(&format!("--{mixed_boundary}\r\n"));
        s.push_str(&body_part);
        s.push_str("\r\n");
        for path in attachment_paths {
            s.push_str(&format!("--{mixed_boundary}\r\n"));
            s.push_str(&build_attachment_part(path)?);
            s.push_str("\r\n");
        }
        s.push_str(&format!("--{mixed_boundary}--\r\n"));
        s
    } else {
        let mut s = headers;
        s.push_str(&body_part);
        s
    };

    Ok(URL_SAFE_NO_PAD.encode(message.as_bytes()))
}

fn build_attachment_part(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read attachment {}: {e}", path.display()))?;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "invalid attachment filename".to_string())?;
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    // Base64 encode and wrap at 76 chars per line (RFC 2045).
    let encoded = STANDARD.encode(&bytes);
    let mut wrapped = String::with_capacity(encoded.len() + encoded.len() / 76 * 2);
    for chunk in encoded.as_bytes().chunks(76) {
        wrapped.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        wrapped.push_str("\r\n");
    }

    // Sanitize filename for the Content-Disposition header.
    let safe_name = filename.replace(['\r', '\n', '"'], "_");
    let mut part = String::new();
    part.push_str(&format!(
        "Content-Type: {mime_type}; name=\"{safe_name}\"\r\n"
    ));
    part.push_str("Content-Transfer-Encoding: base64\r\n");
    part.push_str(&format!(
        "Content-Disposition: attachment; filename=\"{safe_name}\"\r\n\r\n"
    ));
    part.push_str(&wrapped);
    Ok(part)
}

fn sanitize_header(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_string()
}
