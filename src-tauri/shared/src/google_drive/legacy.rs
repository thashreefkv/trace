use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::db::sql_error;

// ── Content export for embeddings ─────────────────────────────────────────────

/// Export a Google Workspace file as plain text (Docs/Slides → text/plain, Sheets → text/csv).
/// Returns `Err` for unsupported mime types so callers can skip gracefully.
pub async fn export_doc_text(
    dir: &Path,
    drive_file_id: &str,
    mime_type: &str,
) -> Result<String, String> {
    let export_mime = match mime_type {
        "application/vnd.google-apps.document" => "text/plain",
        "application/vnd.google-apps.spreadsheet" => "text/csv",
        "application/vnd.google-apps.presentation" => "text/plain",
        _ => return Err(format!("unsupported mime for export: {mime_type}")),
    };
    let token = get_valid_access_token(dir).await?;
    let resp = reqwest::Client::new()
        .get(format!(
            "https://www.googleapis.com/drive/v3/files/{drive_file_id}/export"
        ))
        .query(&[("mimeType", export_mime)])
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "export {} {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    resp.text().await.map_err(|e| e.to_string())
}

const SCOPE: &str = "https://www.googleapis.com/auth/drive.readonly https://www.googleapis.com/auth/userinfo.email openid https://www.googleapis.com/auth/documents https://www.googleapis.com/auth/presentations https://www.googleapis.com/auth/spreadsheets";
const USERINFO_URI: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const REFRESH_TOKEN_ACCOUNT: &str = "google-drive-refresh-token";
const ACCESS_TOKEN_ACCOUNT: &str = "google-drive-access-token";
const REFRESH_TOKEN_FILE: &str = "drive_refresh_token";
const ACCESS_TOKEN_FILE: &str = "drive_access_token";
pub const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";

pub const FOLDER_MIME: &str = "application/vnd.google-apps.folder";

fn expiry_path(dir: &Path) -> PathBuf {
    dir.join("drive_access_token_expiry")
}

fn editor_scope_path(dir: &Path) -> PathBuf {
    dir.join("drive_editor_scopes_granted")
}

/// Returns true if the stored token includes the documents + presentations scopes.
/// Existing users who authenticated with only drive.readonly will return false
/// and need to re-connect to enable in-app editing.
pub fn has_editor_scope(dir: &Path) -> bool {
    editor_scope_path(dir).exists()
}

pub fn drive_connected(dir: &Path) -> bool {
    crate::keychain::get_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE)
        .map(|token| token.is_some())
        .unwrap_or(false)
}

pub fn drive_disconnect_tokens(dir: &Path) -> Result<(), String> {
    crate::keychain::clear_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE)?;
    crate::keychain::clear_secret(dir, ACCESS_TOKEN_ACCOUNT, ACCESS_TOKEN_FILE)?;
    for path in [expiry_path(dir), editor_scope_path(dir)] {
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("failed to remove drive token: {e}"))?;
        }
    }
    Ok(())
}

pub fn build_auth_url(redirect_uri: &str) -> Result<crate::oauth::OAuthFlow, String> {
    crate::oauth::google_oauth_flow(redirect_uri, SCOPE)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveAccount {
    pub id: String,
    pub email: String,
    pub created_at: String,
}

pub async fn complete_oauth(
    pool: &SqlitePool,
    dir: &Path,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<DriveAccount, String> {
    #[derive(Deserialize)]
    struct UserInfo {
        email: String,
    }

    let client_id = crate::oauth::google_client_id()?;
    let tokens =
        crate::oauth::exchange_code_for_tokens(code, redirect_uri, &client_id, code_verifier)
            .await?;
    let client = reqwest::Client::new();

    crate::keychain::harden_private_dir(dir)?;

    if let Some(rt) = tokens.refresh_token {
        crate::keychain::save_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE, &rt)?;
    }
    crate::keychain::save_secret(
        dir,
        ACCESS_TOKEN_ACCOUNT,
        ACCESS_TOKEN_FILE,
        &tokens.access_token,
    )?;
    let expiry = Utc::now().timestamp() + tokens.expires_in as i64;
    std::fs::write(expiry_path(dir), expiry.to_string())
        .map_err(|e| format!("failed to save token expiry: {e}"))?;
    // Mark that this token includes editor scopes (documents + presentations).
    let _ = std::fs::write(editor_scope_path(dir), "1");

    let user: UserInfo = client
        .get(USERINFO_URI)
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .map_err(|e| format!("userinfo request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("failed to parse userinfo: {e}"))?;

    let now_iso = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM google_drive_accounts WHERE email = ?")
            .bind(&user.email)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?;

    let account_id = if let Some((id,)) = existing {
        id
    } else {
        let id = Ulid::new().to_string();
        sqlx::query("INSERT INTO google_drive_accounts (id, email, created_at) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(&user.email)
            .bind(&now_iso)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        sqlx::query("INSERT OR IGNORE INTO google_drive_settings (account_id) VALUES (?)")
            .bind(&id)
            .execute(pool)
            .await
            .map_err(sql_error)?;
        id
    };

    Ok(DriveAccount {
        id: account_id,
        email: user.email,
        created_at: now_iso,
    })
}

pub async fn get_valid_access_token(dir: &Path) -> Result<String, String> {
    if let Ok(expiry_str) = std::fs::read_to_string(expiry_path(dir)) {
        if let Ok(expiry) = expiry_str.trim().parse::<i64>() {
            if Utc::now().timestamp() + 60 < expiry {
                if let Some(token) =
                    crate::keychain::get_secret(dir, ACCESS_TOKEN_ACCOUNT, ACCESS_TOKEN_FILE)?
                {
                    return Ok(token);
                }
            }
        }
    }

    let refresh_token =
        crate::keychain::get_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE)?
            .ok_or_else(|| "Drive not connected".to_string())?;
    let client_id = crate::oauth::google_client_id()?;

    #[derive(Deserialize)]
    struct RefreshResp {
        access_token: String,
        expires_in: u64,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(crate::oauth::GOOGLE_TOKEN_URI)
        .form(&[
            ("refresh_token", refresh_token.as_str()),
            ("client_id", client_id.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| format!("drive token refresh request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Drive token refresh failed with status {}",
            resp.status()
        ));
    }

    let tokens: RefreshResp = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse refresh response: {e}"))?;

    crate::keychain::save_secret(
        dir,
        ACCESS_TOKEN_ACCOUNT,
        ACCESS_TOKEN_FILE,
        &tokens.access_token,
    )?;
    let expiry = Utc::now().timestamp() + tokens.expires_in as i64;
    std::fs::write(expiry_path(dir), expiry.to_string())
        .map_err(|e| format!("failed to save drive token expiry: {e}"))?;

    Ok(tokens.access_token)
}

pub async fn list_accounts(pool: &SqlitePool) -> Result<Vec<DriveAccount>, String> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, email, created_at FROM google_drive_accounts ORDER BY created_at",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    Ok(rows
        .into_iter()
        .map(|(id, email, created_at)| DriveAccount {
            id,
            email,
            created_at,
        })
        .collect())
}

pub async fn disconnect_account(
    pool: &SqlitePool,
    dir: &Path,
    account_id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM google_drive_accounts WHERE id = ?")
        .bind(account_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    drive_disconnect_tokens(dir)?;
    Ok(())
}

// ── Listing ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmeetFolder {
    pub account_id: String,
    pub folder_id: String,
    pub folder_name: String,
}

pub async fn get_gmeet_folder(
    pool: &SqlitePool,
    account_id: &str,
) -> Result<Option<GmeetFolder>, String> {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT gmeet_folder_id, gmeet_folder_name FROM google_drive_settings WHERE account_id = ?",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    Ok(row.and_then(|(fid, fname)| match (fid, fname) {
        (Some(folder_id), Some(folder_name)) => Some(GmeetFolder {
            account_id: account_id.to_string(),
            folder_id,
            folder_name,
        }),
        _ => None,
    }))
}

pub async fn set_gmeet_folder(
    pool: &SqlitePool,
    account_id: &str,
    folder_id: &str,
    folder_name: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE google_drive_settings SET gmeet_folder_id = ?, gmeet_folder_name = ? WHERE account_id = ?",
    )
    .bind(folder_id)
    .bind(folder_name)
    .bind(account_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn clear_gmeet_folder(pool: &SqlitePool, account_id: &str) -> Result<(), String> {
    sqlx::query(
        "UPDATE google_drive_settings SET gmeet_folder_id = NULL, gmeet_folder_name = NULL WHERE account_id = ?",
    )
    .bind(account_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}
