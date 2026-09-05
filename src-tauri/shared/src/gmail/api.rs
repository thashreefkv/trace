use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use ulid::Ulid;

use super::{
    auto_link_thread, classify_url, extract_urls, format_ts, get_valid_access_token,
    header_value, max_history, message_preview, normalize_email, now_utc, parse_address_header,
    parse_addresses_json, parse_string_vec, percent_encode, rebuild_thread_aggregate,
    refresh_thread_intelligence, refresh_work_mail_dimensions, strip_html, to_json_string,
    GmailThread,
};
use super::models::*;

pub async fn search_threads(
    dir: &Path,
    query: &str,
    max_results: u32,
) -> Result<Vec<GmailThread>, String> {
    let token = get_valid_access_token(dir).await?;
    let client = reqwest::Client::new();

    #[derive(Deserialize)]
    struct ListResp {
        threads: Option<Vec<ThreadRef>>,
    }
    #[derive(Deserialize)]
    struct ThreadRef {
        id: String,
        snippet: Option<String>,
    }

    let list: ListResp = client
        .get("https://gmail.googleapis.com/gmail/v1/users/me/threads")
        .bearer_auth(&token)
        .query(&[("q", query), ("maxResults", &max_results.to_string())])
        .send()
        .await
        .map_err(|e| format!("gmail list request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("failed to parse gmail thread list: {e}"))?;

    let mut threads = Vec::new();
    for tref in list.threads.unwrap_or_default() {
        match fetch_thread(&client, &token, &tref.id, tref.snippet).await {
            Ok(t) => threads.push(t),
            Err(_) => continue,
        }
    }

    Ok(threads)
}

async fn fetch_thread(
    client: &reqwest::Client,
    token: &str,
    thread_id: &str,
    snippet_hint: Option<String>,
) -> Result<GmailThread, String> {
    #[derive(Deserialize)]
    struct ThreadDetail {
        id: String,
        messages: Vec<ThreadMessage>,
    }
    #[derive(Deserialize)]
    struct ThreadMessage {
        snippet: Option<String>,
        payload: Option<Payload>,
        #[serde(rename = "internalDate")]
        internal_date: Option<String>,
    }
    #[derive(Deserialize)]
    struct Payload {
        headers: Option<Vec<Header>>,
    }
    #[derive(Deserialize)]
    struct Header {
        name: String,
        value: String,
    }

    let detail: ThreadDetail = client
        .get(format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/threads/{thread_id}"
        ))
        .bearer_auth(token)
        .query(&[
            ("format", "metadata"),
            ("metadataHeaders", "Subject"),
            ("metadataHeaders", "From"),
            ("metadataHeaders", "Date"),
        ])
        .send()
        .await
        .map_err(|e| format!("thread detail request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("failed to parse thread detail: {e}"))?;

    let first = detail.messages.first().ok_or("empty thread")?;
    let last = detail.messages.last().unwrap();

    let headers: &[Header] = first
        .payload
        .as_ref()
        .and_then(|p| p.headers.as_deref())
        .unwrap_or(&[]);

    let subject = headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("Subject"))
        .map(|h| h.value.clone())
        .unwrap_or_else(|| "(no subject)".to_string());

    let from = headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("From"))
        .map(|h| h.value.clone())
        .unwrap_or_default();

    let (sender, sender_email) = parse_from(&from);

    let date_ts = first
        .internal_date
        .as_deref()
        .and_then(|d| d.parse::<i64>().ok())
        .map(|ms| ms / 1000)
        .unwrap_or(0);

    let snippet = snippet_hint
        .or_else(|| last.snippet.clone())
        .unwrap_or_default();

    Ok(GmailThread {
        thread_id: detail.id,
        subject,
        snippet,
        sender,
        sender_email,
        date_ts,
        message_count: detail.messages.len() as u32,
    })
}

fn parse_from(from: &str) -> (String, String) {
    if let (Some(start), Some(end)) = (from.find('<'), from.find('>')) {
        let name = from[..start].trim().trim_matches('"').to_string();
        let email = from[start + 1..end].trim().to_string();
        let display = if name.is_empty() { email.clone() } else { name };
        (display, email)
    } else {
        let trimmed = from.trim().to_string();
        (trimmed.clone(), trimmed)
    }
}
pub async fn fetch_profile(client: &reqwest::Client, token: &str) -> Result<GmailProfile, String> {
    request_json(
        client
            .get("https://gmail.googleapis.com/gmail/v1/users/me/profile")
            .bearer_auth(token),
        "gmail profile request",
    )
    .await
}

pub async fn sync_labels(
    client: &reqwest::Client,
    token: &str,
    pool: &SqlitePool,
) -> Result<i64, String> {
    let labels: ApiLabelList = request_json(
        client
            .get("https://gmail.googleapis.com/gmail/v1/users/me/labels")
            .bearer_auth(token),
        "gmail label list request",
    )
    .await?;

    let now = now_utc();
    let labels = labels.labels.unwrap_or_default();
    for label in &labels {
        sqlx::query(
            r#"
            INSERT INTO gmail_labels (gmail_label_id, name, type, color, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(gmail_label_id) DO UPDATE SET
              name = excluded.name,
              type = excluded.type,
              color = excluded.color,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(&label.id)
        .bind(&label.name)
        .bind(label.label_type.as_deref().unwrap_or(""))
        .bind(
            label
                .color
                .as_ref()
                .and_then(|color| color.background_color.clone()),
        )
        .bind(&now)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;

        mirror_gmail_label(pool, label, &now).await?;
    }

    Ok(labels.len() as i64)
}

async fn mirror_gmail_label(pool: &SqlitePool, label: &ApiLabel, now: &str) -> Result<(), String> {
    let Some(label_type) = label.label_type.as_deref() else {
        return Ok(());
    };
    if label_type != "user" {
        return Ok(());
    }
    let existing_id: Option<String> = sqlx::query_scalar("SELECT id FROM labels WHERE name = ?")
        .bind(&label.name)
        .fetch_optional(pool)
        .await
        .map_err(crate::db::sql_error)?;

    if existing_id.is_none() {
        sqlx::query("INSERT INTO labels (id, name, color) VALUES (?, ?, 'zinc')")
            .bind(Ulid::new().to_string())
            .bind(&label.name)
            .execute(pool)
            .await
            .map_err(crate::db::sql_error)?;
    }
    let _ = now;
    Ok(())
}

pub async fn list_api_threads(
    client: &reqwest::Client,
    token: &str,
    query: &str,
    max_results: u32,
) -> Result<Vec<ApiThreadRef>, String> {
    let mut threads = Vec::new();
    let mut page_token: Option<String> = None;
    let per_page = max_results.clamp(1, 500);

    while threads.len() < max_results as usize {
        let remaining = max_results as usize - threads.len();
        let page = list_api_threads_page(
            client,
            token,
            query,
            per_page.min(remaining as u32),
            page_token.as_deref(),
        )
        .await?;

        threads.extend(page.threads);
        page_token = page.next_page_token;
        if page_token.is_none() {
            break;
        }
    }

    Ok(threads)
}

pub async fn list_api_threads_page(
    client: &reqwest::Client,
    token: &str,
    query: &str,
    max_results: u32,
    page_token: Option<&str>,
) -> Result<ApiThreadPage, String> {
    let mut params = vec![
        ("q".to_string(), query.to_string()),
        (
            "maxResults".to_string(),
            max_results.clamp(1, 500).to_string(),
        ),
    ];
    if let Some(token) = page_token {
        params.push(("pageToken".to_string(), token.to_string()));
    }

    let response: ApiThreadList = request_json(
        client
            .get("https://gmail.googleapis.com/gmail/v1/users/me/threads")
            .bearer_auth(token)
            .query(&params),
        "gmail thread list request",
    )
    .await?;

    Ok(ApiThreadPage {
        threads: response.threads.unwrap_or_default(),
        next_page_token: response.next_page_token,
    })
}

pub async fn fetch_api_thread(
    client: &reqwest::Client,
    token: &str,
    thread_id: &str,
) -> Result<ApiThreadDetail, String> {
    request_json(
        client
            .get(format!(
                "https://gmail.googleapis.com/gmail/v1/users/me/threads/{thread_id}"
            ))
            .bearer_auth(token)
            .query(&[("format", "full")]),
        "gmail thread detail request",
    )
    .await
}

pub async fn sync_drafts(
    client: &reqwest::Client,
    token: &str,
    pool: &SqlitePool,
    synced_at: &str,
) -> Result<i64, String> {
    let mut count = 0i64;
    let mut page_token: Option<String> = None;
    let mut seen = BTreeSet::new();

    loop {
        let mut params = vec![("maxResults".to_string(), "100".to_string())];
        if let Some(token) = page_token.as_deref() {
            params.push(("pageToken".to_string(), token.to_string()));
        }

        let response: ApiDraftList = request_json(
            client
                .get("https://gmail.googleapis.com/gmail/v1/users/me/drafts")
                .bearer_auth(token)
                .query(&params),
            "gmail draft list request",
        )
        .await?;

        for draft in response.drafts.unwrap_or_default() {
            seen.insert(draft.id.clone());
            let detail: ApiDraftDetail = request_json(
                client
                    .get(format!(
                        "https://gmail.googleapis.com/gmail/v1/users/me/drafts/{}",
                        draft.id
                    ))
                    .bearer_auth(token)
                    .query(&[("format", "full")]),
                "gmail draft detail request",
            )
            .await?;
            upsert_draft(pool, detail, synced_at).await?;
            count += 1;
        }

        page_token = response.next_page_token;
        if page_token.is_none() {
            break;
        }
    }

    let existing: Vec<String> = sqlx::query_scalar("SELECT draft_id FROM gmail_drafts")
        .fetch_all(pool)
        .await
        .map_err(crate::db::sql_error)?;
    for draft_id in existing {
        if !seen.contains(&draft_id) {
            sqlx::query("DELETE FROM gmail_drafts WHERE draft_id = ?")
                .bind(draft_id)
                .execute(pool)
                .await
                .map_err(crate::db::sql_error)?;
        }
    }

    Ok(count)
}

pub async fn request_json<T: serde::de::DeserializeOwned>(
    builder: reqwest::RequestBuilder,
    label: &str,
) -> Result<T, String> {
    let response = builder
        .send()
        .await
        .map_err(|e| format!("{label} failed: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("{label} failed with {status}: {body}"));
    }
    response
        .json::<T>()
        .await
        .map_err(|e| format!("{label} response was not valid JSON: {e}"))
}

pub fn parse_api_message(
    api_message: ApiMessage,
    thread_hint: Option<&str>,
) -> Result<ParsedMessage, String> {
    let thread_id = api_message
        .thread_id
        .clone()
        .or_else(|| thread_hint.map(str::to_string))
        .ok_or_else(|| "Gmail message did not include a thread id".to_string())?;
    let payload = api_message.payload.clone().unwrap_or(ApiPayload {
        mime_type: None,
        filename: None,
        headers: None,
        body: None,
        parts: None,
    });

    let headers = payload.headers.clone().unwrap_or_default();
    let header_map = headers
        .iter()
        .map(|header| (header.name.clone(), header.value.clone()))
        .collect::<BTreeMap<_, _>>();
    let subject = header_value(&headers, "Subject").unwrap_or_else(|| "(no subject)".to_string());
    let from = parse_address_header(&header_value(&headers, "From").unwrap_or_default())
        .into_iter()
        .next()
        .unwrap_or(EmailAddress {
            name: String::new(),
            email: String::new(),
        });
    let to = parse_address_header(&header_value(&headers, "To").unwrap_or_default());
    let cc = parse_address_header(&header_value(&headers, "Cc").unwrap_or_default());
    let bcc = parse_address_header(&header_value(&headers, "Bcc").unwrap_or_default());
    let label_ids = api_message.label_ids.clone().unwrap_or_default();
    let date_ts = header_value(&headers, "Date").and_then(|date| {
        chrono::DateTime::parse_from_rfc2822(&date)
            .ok()
            .map(|date| date.timestamp())
    });
    let internal_date_ts = api_message
        .internal_date
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok())
        .map(|ms| ms / 1000);

    let mut plain_parts = Vec::new();
    let mut html_parts = Vec::new();
    let mut attachments = Vec::new();
    collect_payload_parts(
        &payload,
        &mut plain_parts,
        &mut html_parts,
        &mut attachments,
    );
    let plain_body = plain_parts.join("\n\n").trim().to_string();
    let html_body = html_parts.join("\n\n").trim().to_string();
    let searchable_text = if plain_body.is_empty() {
        strip_html(&html_body)
    } else {
        plain_body.clone()
    };
    let artifact_urls = extract_urls(&format!("{}\n{}\n{}", subject, searchable_text, html_body));

    Ok(ParsedMessage {
        message_id: api_message.id,
        thread_id,
        history_id: api_message.history_id,
        subject,
        snippet: api_message.snippet.unwrap_or_default(),
        from,
        to,
        cc,
        bcc,
        date_ts,
        internal_date_ts,
        plain_body,
        html_body,
        headers_json: serde_json::to_string(&header_map).unwrap_or_else(|_| "{}".to_string()),
        is_sent: label_ids.iter().any(|label| label == "SENT"),
        is_draft: label_ids.iter().any(|label| label == "DRAFT"),
        is_unread: label_ids.iter().any(|label| label == "UNREAD"),
        label_ids,
        size_estimate: api_message.size_estimate,
        artifact_urls,
        attachments,
    })
}

fn collect_payload_parts(
    payload: &ApiPayload,
    plain_parts: &mut Vec<String>,
    html_parts: &mut Vec<String>,
    attachments: &mut Vec<ParsedAttachment>,
) {
    let mime_type = payload
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream")
        .to_lowercase();
    let filename = payload.filename.clone().unwrap_or_default();

    if !filename.trim().is_empty()
        || payload
            .body
            .as_ref()
            .and_then(|b| b.attachment_id.as_ref())
            .is_some()
    {
        attachments.push(ParsedAttachment {
            attachment_id: payload.body.as_ref().and_then(|b| b.attachment_id.clone()),
            filename,
            mime_type: mime_type.clone(),
            size: payload.body.as_ref().and_then(|b| b.size),
        });
    } else if let Some(data) = payload.body.as_ref().and_then(|body| body.data.as_deref()) {
        if let Ok(decoded) = decode_base64_url(data) {
            match mime_type.as_str() {
                "text/plain" => plain_parts.push(decoded),
                "text/html" => html_parts.push(decoded),
                _ => {}
            }
        }
    }

    if let Some(parts) = payload.parts.as_deref() {
        for part in parts {
            collect_payload_parts(part, plain_parts, html_parts, attachments);
        }
    }
}

fn decode_base64_url(data: &str) -> Result<String, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(data.as_bytes())
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(data.as_bytes()))
        .map_err(|e| format!("failed to decode Gmail body: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

pub async fn ensure_thread_placeholder(
    pool: &SqlitePool,
    thread_id: &str,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO gmail_threads (thread_id, last_sync_at)
        VALUES (?, ?)
        ON CONFLICT(thread_id) DO UPDATE SET last_sync_at = excluded.last_sync_at
        "#,
    )
    .bind(thread_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

pub async fn upsert_message(
    pool: &SqlitePool,
    parsed: &ParsedMessage,
    synced_at: &str,
) -> Result<bool, String> {
    let existed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM gmail_messages WHERE message_id = ?")
            .bind(&parsed.message_id)
            .fetch_one(pool)
            .await
            .map_err(crate::db::sql_error)?;
    let to_json = to_json_string(&parsed.to);
    let cc_json = to_json_string(&parsed.cc);
    let bcc_json = to_json_string(&parsed.bcc);
    let labels_json = to_json_string(&parsed.label_ids);
    let artifact_urls_json = to_json_string(&parsed.artifact_urls);

    sqlx::query(
        r#"
        INSERT INTO gmail_messages (
          message_id, thread_id, history_id, subject, snippet, from_name, from_email,
          to_json, cc_json, bcc_json, date_ts, internal_date_ts, plain_body, html_body,
          raw_headers_json, label_ids_json, is_sent, is_draft, is_unread,
          size_estimate, artifact_urls_json, synced_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(message_id) DO UPDATE SET
          thread_id = excluded.thread_id,
          history_id = excluded.history_id,
          subject = excluded.subject,
          snippet = excluded.snippet,
          from_name = excluded.from_name,
          from_email = excluded.from_email,
          to_json = excluded.to_json,
          cc_json = excluded.cc_json,
          bcc_json = excluded.bcc_json,
          date_ts = excluded.date_ts,
          internal_date_ts = excluded.internal_date_ts,
          plain_body = excluded.plain_body,
          html_body = excluded.html_body,
          raw_headers_json = excluded.raw_headers_json,
          label_ids_json = excluded.label_ids_json,
          is_sent = excluded.is_sent,
          is_draft = excluded.is_draft,
          is_unread = excluded.is_unread,
          size_estimate = excluded.size_estimate,
          artifact_urls_json = excluded.artifact_urls_json,
          synced_at = excluded.synced_at
        "#,
    )
    .bind(&parsed.message_id)
    .bind(&parsed.thread_id)
    .bind(&parsed.history_id)
    .bind(&parsed.subject)
    .bind(&parsed.snippet)
    .bind(&parsed.from.name)
    .bind(&parsed.from.email)
    .bind(&to_json)
    .bind(&cc_json)
    .bind(&bcc_json)
    .bind(parsed.date_ts)
    .bind(parsed.internal_date_ts)
    .bind(&parsed.plain_body)
    .bind(&parsed.html_body)
    .bind(&parsed.headers_json)
    .bind(&labels_json)
    .bind(parsed.is_sent)
    .bind(parsed.is_draft)
    .bind(parsed.is_unread)
    .bind(parsed.size_estimate)
    .bind(&artifact_urls_json)
    .bind(synced_at)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    sqlx::query("DELETE FROM gmail_attachments WHERE message_id = ?")
        .bind(&parsed.message_id)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    let shared_with_json = to_json_string(
        &parsed
            .to
            .iter()
            .chain(parsed.cc.iter())
            .chain(parsed.bcc.iter())
            .cloned()
            .collect::<Vec<_>>(),
    );
    for attachment in &parsed.attachments {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO gmail_attachments (
              id, message_id, thread_id, attachment_id, filename, mime_type, size,
              shared_by_email, shared_with_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(Ulid::new().to_string())
        .bind(&parsed.message_id)
        .bind(&parsed.thread_id)
        .bind(&attachment.attachment_id)
        .bind(&attachment.filename)
        .bind(&attachment.mime_type)
        .bind(attachment.size)
        .bind(&parsed.from.email)
        .bind(&shared_with_json)
        .bind(synced_at)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    }

    for url in &parsed.artifact_urls {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO gmail_links (id, thread_id, message_id, url, kind, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(Ulid::new().to_string())
        .bind(&parsed.thread_id)
        .bind(&parsed.message_id)
        .bind(url)
        .bind(classify_url(url))
        .bind(synced_at)
        .execute(pool)
        .await
        .map_err(crate::db::sql_error)?;
    }

    Ok(existed == 0)
}

pub async fn delete_stale_thread_links(
    pool: &SqlitePool,
    thread_id: &str,
    current_urls: &BTreeSet<String>,
) -> Result<(), String> {
    let existing: Vec<(String, String)> =
        sqlx::query_as("SELECT id, url FROM gmail_links WHERE thread_id = ?")
            .bind(thread_id)
            .fetch_all(pool)
            .await
            .map_err(crate::db::sql_error)?;
    for (id, url) in existing {
        if !current_urls.contains(&url) {
            sqlx::query("DELETE FROM gmail_links WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await
                .map_err(crate::db::sql_error)?;
        }
    }
    Ok(())
}

pub async fn upsert_draft(
    pool: &SqlitePool,
    detail: ApiDraftDetail,
    synced_at: &str,
) -> Result<(), String> {
    let parsed = parse_api_message(detail.message, None)?;
    let preview = message_preview(&parsed);
    sqlx::query(
        r#"
        INSERT INTO gmail_drafts (
          draft_id, message_id, thread_id, subject, to_json, cc_json, bcc_json,
          body_preview, updated_at, synced_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(draft_id) DO UPDATE SET
          message_id = excluded.message_id,
          thread_id = excluded.thread_id,
          subject = excluded.subject,
          to_json = excluded.to_json,
          cc_json = excluded.cc_json,
          bcc_json = excluded.bcc_json,
          body_preview = excluded.body_preview,
          updated_at = excluded.updated_at,
          synced_at = excluded.synced_at
        "#,
    )
    .bind(detail.id)
    .bind(parsed.message_id)
    .bind(parsed.thread_id)
    .bind(parsed.subject)
    .bind(to_json_string(&parsed.to))
    .bind(to_json_string(&parsed.cc))
    .bind(to_json_string(&parsed.bcc))
    .bind(preview)
    .bind(format_ts(parsed.internal_date_ts))
    .bind(synced_at)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

