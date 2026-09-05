//! Drive file listing + import. Extracted from legacy.rs (13-std4).

use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::db::sql_error;
use crate::files::row_to_file_public;
use crate::models::FileRow;
use super::{get_valid_access_token, DRIVE_API, FOLDER_MIME};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveEntry {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub is_folder: bool,
    pub web_view_link: Option<String>,
    pub modified_time: Option<String>,
    pub size: Option<i64>,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveListing {
    pub entries: Vec<DriveEntry>,
    pub next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct DriveListResp {
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    files: Vec<DriveFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "webViewLink")]
    pub web_view_link: Option<String>,
    #[serde(rename = "modifiedTime")]
    pub modified_time: Option<String>,
    pub size: Option<String>,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub trashed: bool,
    #[serde(rename = "md5Checksum")]
    pub md5_checksum: Option<String>,
}

const LIST_FIELDS: &str = "nextPageToken,files(id,name,mimeType,webViewLink,modifiedTime,size,parents,trashed,md5Checksum)";

pub async fn list_children(
    dir: &Path,
    parent_id: Option<&str>,
    page_token: Option<&str>,
) -> Result<DriveListing, String> {
    let token = get_valid_access_token(dir).await?;
    let parent = parent_id.unwrap_or("root");
    let q = format!(
        "'{}' in parents and trashed = false",
        parent.replace('\'', "\\'")
    );
    let client = reqwest::Client::new();
    let mut req = client
        .get(format!("{DRIVE_API}/files"))
        .bearer_auth(token)
        .query(&[
            ("q", q.as_str()),
            ("fields", LIST_FIELDS),
            ("pageSize", "200"),
            ("orderBy", "folder,name_natural"),
        ]);
    if let Some(pt) = page_token {
        req = req.query(&[("pageToken", pt)]);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("drive list request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("drive list failed ({status}): {body}"));
    }
    let parsed: DriveListResp = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse drive list: {e}"))?;
    Ok(DriveListing {
        entries: parsed.files.into_iter().map(into_entry).collect(),
        next_page_token: parsed.next_page_token,
    })
}

pub async fn get_metadata(dir: &Path, file_id: &str) -> Result<DriveFile, String> {
    let token = get_valid_access_token(dir).await?;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{DRIVE_API}/files/{file_id}"))
        .bearer_auth(token)
        .query(&[(
            "fields",
            "id,name,mimeType,webViewLink,modifiedTime,size,parents,trashed,md5Checksum",
        )])
        .send()
        .await
        .map_err(|e| format!("drive get failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("drive get failed ({status}): {body}"));
    }
    resp.json()
        .await
        .map_err(|e| format!("failed to parse drive get: {e}"))
}

fn into_entry(f: DriveFile) -> DriveEntry {
    let is_folder = f.mime_type == FOLDER_MIME;
    DriveEntry {
        id: f.id,
        name: f.name,
        mime_type: f.mime_type,
        is_folder,
        web_view_link: f.web_view_link,
        modified_time: f.modified_time,
        size: f.size.and_then(|s| s.parse::<i64>().ok()),
        parents: f.parents,
    }
}

// ── Import ───────────────────────────────────────────────────────────────────

pub async fn import_files(
    pool: &SqlitePool,
    dir: &Path,
    api_key: Option<&str>,
    account_id: &str,
    drive_file_ids: &[String],
    trace_folder_id: Option<&str>,
) -> Result<Vec<FileRow>, String> {
    let mut out: Vec<FileRow> = Vec::new();
    for fid in drive_file_ids {
        let meta = get_metadata(dir, fid).await?;
        if meta.mime_type == FOLDER_MIME {
            // Skip folders for import (they're navigation only).
            continue;
        }
        let parent = meta.parents.first().cloned();
        let size_bytes: Option<i64> = meta.size.and_then(|s| s.parse::<i64>().ok());

        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM files WHERE drive_account_id = ? AND drive_file_id = ?")
                .bind(account_id)
                .bind(&meta.id)
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?;

        let now_iso = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let file_id = if let Some((id,)) = existing {
            sqlx::query(
                "UPDATE files SET name = ?, mime_type = ?, size_bytes = ?, \
                  drive_parent_id = ?, drive_mime = ?, drive_web_view_link = ?, \
                  drive_modified_time = ?, drive_trashed = ?, drive_md5 = ?, \
                  trace_folder_id = COALESCE(?, trace_folder_id), updated_at = ? \
                 WHERE id = ?",
            )
            .bind(&meta.name)
            .bind(&meta.mime_type)
            .bind(size_bytes)
            .bind(&parent)
            .bind(&meta.mime_type)
            .bind(&meta.web_view_link)
            .bind(&meta.modified_time)
            .bind(if meta.trashed { 1i64 } else { 0i64 })
            .bind(&meta.md5_checksum)
            .bind(trace_folder_id)
            .bind(&now_iso)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
            id
        } else {
            let id = Ulid::new().to_string();
            sqlx::query(
                "INSERT INTO files \
                  (id, kind, trace_folder_id, name, mime_type, size_bytes, \
                   drive_account_id, drive_file_id, drive_parent_id, drive_mime, \
                   drive_web_view_link, drive_modified_time, drive_trashed, drive_md5, \
                   created_at, updated_at) \
                 VALUES (?, 'drive', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(trace_folder_id)
            .bind(&meta.name)
            .bind(&meta.mime_type)
            .bind(size_bytes)
            .bind(account_id)
            .bind(&meta.id)
            .bind(&parent)
            .bind(&meta.mime_type)
            .bind(&meta.web_view_link)
            .bind(&meta.modified_time)
            .bind(if meta.trashed { 1i64 } else { 0i64 })
            .bind(&meta.md5_checksum)
            .bind(&now_iso)
            .bind(&now_iso)
            .execute(pool)
            .await
            .map_err(sql_error)?;
            id
        };

        // Spawn background content embedding for Google Workspace files.
        let drive_mime = meta.mime_type.clone();
        if matches!(
            drive_mime.as_str(),
            "application/vnd.google-apps.document"
                | "application/vnd.google-apps.spreadsheet"
                | "application/vnd.google-apps.presentation"
        ) {
            if let Some(key) = api_key {
                let (pool2, key2, dir2, fid2, dfid2, mime2) = (
                    pool.clone(),
                    key.to_string(),
                    dir.to_path_buf(),
                    file_id.clone(),
                    meta.id.clone(),
                    drive_mime.clone(),
                );
                crate::runtime::spawn(async move {
                    if let Err(e) = crate::files::embed_drive_file_content(
                        &pool2, &key2, &dir2, &fid2, &dfid2, &mime2,
                    )
                    .await
                    {
                        eprintln!("[files] embed error: {e}");
                    }
                });
            }
        }

        let row = sqlx::query("SELECT * FROM files WHERE id = ?")
            .bind(&file_id)
            .fetch_one(pool)
            .await
            .map_err(sql_error)?;
        out.push(row_to_file_public(row));
    }
    Ok(out)
}

// ── Folder import ─────────────────────────────────────────────────────────────

/// Creates a Trace folder and imports all direct file children from a Drive folder into it.
pub async fn import_drive_folder(
    pool: &SqlitePool,
    dir: &Path,
    api_key: Option<&str>,
    account_id: &str,
    drive_folder_id: &str,
    folder_name: &str,
    parent_trace_folder_id: Option<&str>,
) -> Result<crate::models::TraceFolder, String> {
    use crate::files::create_folder;
    use crate::models::CreateTraceFolderInput;

    let trace_folder = create_folder(
        pool,
        CreateTraceFolderInput {
            parent_id: parent_trace_folder_id.map(String::from),
            name: folder_name.to_string(),
        },
    )
    .await?;

    let mut page_token: Option<String> = None;
    loop {
        let listing = list_children(dir, Some(drive_folder_id), page_token.as_deref()).await?;
        let file_ids: Vec<String> = listing
            .entries
            .iter()
            .filter(|e| !e.is_folder)
            .map(|e| e.id.clone())
            .collect();
        if !file_ids.is_empty() {
            import_files(
                pool,
                dir,
                api_key,
                account_id,
                &file_ids,
                Some(&trace_folder.id),
            )
            .await?;
        }
        match listing.next_page_token {
            Some(next) => page_token = Some(next),
            None => break,
        }
    }

    Ok(trace_folder)
}

// ── Changes / Reconciliation ──────────────────────────────────────────────────

