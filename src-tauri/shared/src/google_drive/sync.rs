//! Drive incremental sync (changes feed) + status. From legacy.rs (13-std4).

use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::sql_error;
use super::{get_valid_access_token, DRIVE_API, FOLDER_MIME};
use super::files::DriveFile;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveSyncStatus {
    pub last_sync_at: Option<String>,
    pub initialized: bool,
}

#[derive(Deserialize)]
struct ChangesResp {
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
    #[serde(rename = "newStartPageToken")]
    new_start_page_token: Option<String>,
    #[serde(default)]
    changes: Vec<DriveChange>,
}

#[derive(Deserialize)]
struct DriveChange {
    #[serde(rename = "fileId")]
    file_id: String,
    removed: Option<bool>,
    file: Option<DriveFile>,
}

async fn get_start_page_token(dir: &Path) -> Result<String, String> {
    let token = get_valid_access_token(dir).await?;
    #[derive(Deserialize)]
    struct StartToken {
        #[serde(rename = "startPageToken")]
        token: String,
    }
    let resp = reqwest::Client::new()
        .get(format!("{DRIVE_API}/changes/startPageToken"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("startPageToken request failed: {e}"))?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("startPageToken failed: {body}"));
    }
    let parsed: StartToken = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse startPageToken: {e}"))?;
    Ok(parsed.token)
}

async fn save_page_token(pool: &SqlitePool, account_id: &str, token: &str) -> Result<(), String> {
    sqlx::query("UPDATE google_drive_settings SET last_page_token = ? WHERE account_id = ?")
        .bind(token)
        .bind(account_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

async fn reconcile_change(
    pool: &SqlitePool,
    dir: &Path,
    api_key: Option<&str>,
    account_id: &str,
    change: DriveChange,
) -> Result<(), String> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM files WHERE drive_account_id = ? AND drive_file_id = ?")
            .bind(account_id)
            .bind(&change.file_id)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?;

    let Some((our_id,)) = existing else {
        return Ok(());
    };

    let now_iso = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    if change.removed.unwrap_or(false) {
        sqlx::query("UPDATE files SET drive_trashed = 1, updated_at = ? WHERE id = ?")
            .bind(&now_iso)
            .bind(&our_id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        return Ok(());
    }

    let Some(file) = change.file else {
        return Ok(());
    };

    // Skip navigation-only types.
    if file.mime_type == FOLDER_MIME || file.mime_type == "application/vnd.google-apps.shortcut" {
        return Ok(());
    }

    let new_parent = file.parents.first().cloned();
    let size_bytes: Option<i64> = file.size.as_deref().and_then(|s| s.parse().ok());
    let trashed = if file.trashed { 1i64 } else { 0i64 };
    let drive_mime = file.mime_type.clone();
    let drive_file_id_clone = file.id.clone();

    // NOTE: trace_folder_id is intentionally NOT touched; that's user-owned Trace organisation.
    sqlx::query(
        "UPDATE files SET \
          name = ?, mime_type = ?, size_bytes = ?, \
          drive_parent_id = ?, drive_mime = ?, drive_web_view_link = ?, \
          drive_modified_time = ?, drive_trashed = ?, drive_md5 = ?, \
          updated_at = ? \
         WHERE id = ?",
    )
    .bind(&file.name)
    .bind(&file.mime_type)
    .bind(size_bytes)
    .bind(&new_parent)
    .bind(&file.mime_type)
    .bind(&file.web_view_link)
    .bind(&file.modified_time)
    .bind(trashed)
    .bind(&file.md5_checksum)
    .bind(&now_iso)
    .bind(&our_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    // Re-embed content for Google Workspace files on content change.
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
                our_id.clone(),
                drive_file_id_clone.clone(),
                drive_mime.clone(),
            );
            crate::runtime::spawn(async move {
                if let Err(e) = crate::files::embed_drive_file_content(
                    &pool2, &key2, &dir2, &fid2, &dfid2, &mime2,
                )
                .await
                {
                    eprintln!("[files] reconcile embed error: {e}");
                }
            });
        }
    }

    Ok(())
}

pub async fn pull_changes(
    pool: &SqlitePool,
    dir: &Path,
    api_key: Option<&str>,
    account_id: &str,
) -> Result<u32, String> {
    let page_token: Option<String> = sqlx::query_scalar(
        "SELECT last_page_token FROM google_drive_settings WHERE account_id = ?",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .flatten();

    let mut cursor = match page_token {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            let start = get_start_page_token(dir).await?;
            save_page_token(pool, account_id, &start).await?;
            return Ok(0);
        }
    };

    let access_token = get_valid_access_token(dir).await?;
    let client = reqwest::Client::new();
    let mut processed = 0u32;

    const CHANGE_FIELDS: &str =
        "nextPageToken,newStartPageToken,changes(fileId,removed,file(id,name,mimeType,webViewLink,modifiedTime,size,parents,trashed,md5Checksum))";

    loop {
        let resp = client
            .get(format!("{DRIVE_API}/changes"))
            .bearer_auth(&access_token)
            .query(&[
                ("pageToken", cursor.as_str()),
                ("fields", CHANGE_FIELDS),
                ("pageSize", "200"),
            ])
            .send()
            .await
            .map_err(|e| format!("changes.list request failed: {e}"))?;

        // 410 Gone means the page token expired; grab a fresh startPageToken.
        if resp.status().as_u16() == 410 {
            let start = get_start_page_token(dir).await?;
            save_page_token(pool, account_id, &start).await?;
            return Ok(processed);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("changes.list failed ({status}): {body}"));
        }

        let parsed: ChangesResp = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse changes: {e}"))?;

        for change in parsed.changes {
            reconcile_change(pool, dir, api_key, account_id, change).await?;
            processed += 1;
        }

        if let Some(next) = parsed.next_page_token {
            cursor = next;
        } else {
            if let Some(start) = parsed.new_start_page_token {
                save_page_token(pool, account_id, &start).await?;
            }
            break;
        }
    }

    let now_iso = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    sqlx::query("UPDATE google_drive_settings SET last_sync_at = ? WHERE account_id = ?")
        .bind(&now_iso)
        .bind(account_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;

    Ok(processed)
}

pub async fn get_sync_status(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<DriveSyncStatus, String> {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT last_sync_at, last_page_token FROM google_drive_settings WHERE account_id = ?",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let (last_sync_at, initialized) = row
        .map(|(sync, token)| (sync, token.map(|t| !t.trim().is_empty()).unwrap_or(false)))
        .unwrap_or((None, false));

    Ok(DriveSyncStatus {
        last_sync_at,
        initialized,
    })
}

// ── GMeet transcript folder ───────────────────────────────────────────────────

