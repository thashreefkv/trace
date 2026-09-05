//! Local-first email draft persistence.
//!
//! Drafts are persisted to SQLite immediately (auto-save) so the composer
//! survives window closes, restarts, and Gmail API outages. Attachments are
//! copied to `<app_support_dir>/draft_attachments/<draft_id>/<random>_<name>`
//! so the file bytes survive even if the source file is later deleted/moved.
//!
//! Gmail Drafts API sync (push to user's Gmail Drafts folder) is a Phase 2
//! follow-up; the `gmail_draft_id` column exists for that future bridge.

use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::db::sql_error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalEmailDraft {
    pub id: String,
    pub thread_id: Option<String>,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    pub gmail_draft_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub attachments: Vec<LocalEmailDraftAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalEmailDraftAttachment {
    pub id: String,
    pub draft_id: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: i64,
    pub file_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveLocalDraftInput {
    pub id: Option<String>,
    pub thread_id: Option<String>,
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body_html: String,
    #[serde(default)]
    pub body_text: String,
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn draft_root(app_dir: &Path) -> PathBuf {
    app_dir.join("draft_attachments")
}

fn parse_emails(json_str: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(json_str).unwrap_or_default()
}

fn serialize_emails(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

/// Returns the draft for a thread (if any), with attachments.
pub async fn get_draft_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Option<LocalEmailDraft>, String> {
    let row = sqlx::query_as::<_, (
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
    )>(
        r#"SELECT id, thread_id, to_json, cc_json, bcc_json, subject,
                  body_html, body_text, gmail_draft_id, created_at, updated_at
           FROM local_email_drafts
           WHERE thread_id = ?"#,
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let Some(row) = row else { return Ok(None) };
    let attachments = list_attachments(pool, &row.0).await?;
    Ok(Some(LocalEmailDraft {
        id: row.0,
        thread_id: row.1,
        to: parse_emails(&row.2),
        cc: parse_emails(&row.3),
        bcc: parse_emails(&row.4),
        subject: row.5,
        body_html: row.6,
        body_text: row.7,
        gmail_draft_id: row.8,
        created_at: row.9,
        updated_at: row.10,
        attachments,
    }))
}

pub async fn get_draft(pool: &SqlitePool, draft_id: &str) -> Result<Option<LocalEmailDraft>, String> {
    let row = sqlx::query_as::<_, (
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
    )>(
        r#"SELECT id, thread_id, to_json, cc_json, bcc_json, subject,
                  body_html, body_text, gmail_draft_id, created_at, updated_at
           FROM local_email_drafts
           WHERE id = ?"#,
    )
    .bind(draft_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let Some(row) = row else { return Ok(None) };
    let attachments = list_attachments(pool, &row.0).await?;
    Ok(Some(LocalEmailDraft {
        id: row.0,
        thread_id: row.1,
        to: parse_emails(&row.2),
        cc: parse_emails(&row.3),
        bcc: parse_emails(&row.4),
        subject: row.5,
        body_html: row.6,
        body_text: row.7,
        gmail_draft_id: row.8,
        created_at: row.9,
        updated_at: row.10,
        attachments,
    }))
}

/// Upsert a draft. If `input.id` is set, updates that draft. Otherwise: if a
/// draft already exists for the thread, updates it; otherwise creates a new
/// one. Returns the resulting draft (with attachments).
pub async fn save_draft(
    pool: &SqlitePool,
    input: SaveLocalDraftInput,
) -> Result<LocalEmailDraft, String> {
    let now = now_iso();

    // Resolve the target id: explicit id > existing thread draft > new ULID.
    let target_id = if let Some(id) = input.id.clone() {
        id
    } else if let Some(thread_id) = input.thread_id.as_deref() {
        if let Some(existing) = get_draft_for_thread(pool, thread_id).await? {
            existing.id
        } else {
            Ulid::new().to_string()
        }
    } else {
        Ulid::new().to_string()
    };

    let to_json = serialize_emails(&input.to);
    let cc_json = serialize_emails(&input.cc);
    let bcc_json = serialize_emails(&input.bcc);

    sqlx::query(
        r#"INSERT INTO local_email_drafts
             (id, thread_id, to_json, cc_json, bcc_json, subject,
              body_html, body_text, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET
             thread_id  = excluded.thread_id,
             to_json    = excluded.to_json,
             cc_json    = excluded.cc_json,
             bcc_json   = excluded.bcc_json,
             subject    = excluded.subject,
             body_html  = excluded.body_html,
             body_text  = excluded.body_text,
             updated_at = excluded.updated_at"#,
    )
    .bind(&target_id)
    .bind(&input.thread_id)
    .bind(&to_json)
    .bind(&cc_json)
    .bind(&bcc_json)
    .bind(&input.subject)
    .bind(&input.body_html)
    .bind(&input.body_text)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_draft(pool, &target_id)
        .await?
        .ok_or_else(|| "draft saved but failed to reload".to_string())
}

/// Delete a draft and all its attachments (rows + on-disk files).
pub async fn delete_draft(
    pool: &SqlitePool,
    app_dir: &Path,
    draft_id: &str,
) -> Result<(), String> {
    // Cascade on the DB side handles attachment rows. Clean up on-disk files.
    let dir = draft_root(app_dir).join(draft_id);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    sqlx::query("DELETE FROM local_email_drafts WHERE id = ?")
        .bind(draft_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn list_attachments(
    pool: &SqlitePool,
    draft_id: &str,
) -> Result<Vec<LocalEmailDraftAttachment>, String> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64, String, String)>(
        r#"SELECT id, draft_id, filename, mime_type, file_size, file_path, created_at
           FROM local_email_draft_attachments
           WHERE draft_id = ?
           ORDER BY created_at ASC"#,
    )
    .bind(draft_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    Ok(rows
        .into_iter()
        .map(|(id, draft_id, filename, mime_type, file_size, file_path, created_at)| {
            LocalEmailDraftAttachment {
                id,
                draft_id,
                filename,
                mime_type,
                file_size,
                file_path,
                created_at,
            }
        })
        .collect())
}

/// Copy a file from `source_path` into the draft attachment store and record
/// it in the DB. The on-disk path returned is stable for the life of the
/// draft.
pub async fn add_attachment(
    pool: &SqlitePool,
    app_dir: &Path,
    draft_id: &str,
    source_path: &Path,
) -> Result<LocalEmailDraftAttachment, String> {
    if !source_path.exists() {
        return Err(format!(
            "attachment source not found: {}",
            source_path.display()
        ));
    }
    let filename = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "invalid attachment filename".to_string())?
        .to_string();

    let metadata = std::fs::metadata(source_path)
        .map_err(|e| format!("failed to stat attachment: {e}"))?;
    let file_size = metadata.len() as i64;

    let mime_type = mime_guess::from_path(source_path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    let dest_dir = draft_root(app_dir).join(draft_id);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("failed to create attachment dir: {e}"))?;

    let attachment_id = Ulid::new().to_string();
    let dest_filename = format!("{attachment_id}_{filename}");
    let dest_path = dest_dir.join(&dest_filename);
    std::fs::copy(source_path, &dest_path)
        .map_err(|e| format!("failed to copy attachment: {e}"))?;

    let now = now_iso();
    let dest_path_str = dest_path.to_string_lossy().into_owned();

    sqlx::query(
        r#"INSERT INTO local_email_draft_attachments
             (id, draft_id, filename, mime_type, file_size, file_path, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&attachment_id)
    .bind(draft_id)
    .bind(&filename)
    .bind(&mime_type)
    .bind(file_size)
    .bind(&dest_path_str)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    // Bump the parent draft's updated_at.
    sqlx::query("UPDATE local_email_drafts SET updated_at = ? WHERE id = ?")
        .bind(&now)
        .bind(draft_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    Ok(LocalEmailDraftAttachment {
        id: attachment_id,
        draft_id: draft_id.to_string(),
        filename,
        mime_type,
        file_size,
        file_path: dest_path_str,
        created_at: now,
    })
}

pub async fn remove_attachment(
    pool: &SqlitePool,
    attachment_id: &str,
) -> Result<(), String> {
    // Look up the path first so we can clean up the file.
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT file_path, draft_id FROM local_email_draft_attachments WHERE id = ?",
    )
    .bind(attachment_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    if let Some((file_path, draft_id)) = row {
        let _ = std::fs::remove_file(&file_path);
        sqlx::query("DELETE FROM local_email_draft_attachments WHERE id = ?")
            .bind(attachment_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        // Bump parent draft updated_at.
        let now = now_iso();
        sqlx::query("UPDATE local_email_drafts SET updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&draft_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }
    Ok(())
}
