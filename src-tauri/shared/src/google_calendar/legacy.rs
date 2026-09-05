use std::path::{Path, PathBuf};

use chrono::{Duration, NaiveDate, Utc};
use serde::Deserialize;

const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/userinfo.email openid";
const REFRESH_TOKEN_ACCOUNT: &str = "google-calendar-refresh-token";
const ACCESS_TOKEN_ACCOUNT: &str = "google-calendar-access-token";
const REFRESH_TOKEN_FILE: &str = "calendar_refresh_token";
const ACCESS_TOKEN_FILE: &str = "calendar_access_token";
pub const GCAL_API: &str = "https://www.googleapis.com/calendar/v3";

// ── Token file paths ──────────────────────────────────────────────────────────

fn expiry_path(dir: &Path) -> PathBuf {
    dir.join("calendar_access_token_expiry")
}

pub fn calendar_connected(dir: &Path) -> bool {
    crate::keychain::get_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE)
        .map(|token| token.is_some())
        .unwrap_or(false)
}

pub fn calendar_disconnect(dir: &Path) -> Result<(), String> {
    crate::keychain::clear_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE)?;
    crate::keychain::clear_secret(dir, ACCESS_TOKEN_ACCOUNT, ACCESS_TOKEN_FILE)?;
    let path = expiry_path(dir);
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to remove calendar token expiry: {error}"))?;
    }
    Ok(())
}

// ── OAuth ─────────────────────────────────────────────────────────────────────

pub fn build_auth_url(redirect_uri: &str) -> Result<crate::oauth::OAuthFlow, String> {
    crate::oauth::google_oauth_flow(redirect_uri, SCOPE)
}

pub async fn complete_oauth(
    dir: &Path,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<(), String> {
    let client_id = crate::oauth::google_client_id()?;
    let tokens =
        crate::oauth::exchange_code_for_tokens(code, redirect_uri, &client_id, code_verifier)
            .await?;

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
        .map_err(|e| format!("failed to save calendar token expiry: {e}"))?;

    Ok(())
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
            .ok_or_else(|| "Google Calendar not connected".to_string())?;
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
        .map_err(|e| format!("calendar token refresh request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Calendar token refresh failed with status {}",
            resp.status()
        ));
    }

    let tokens: RefreshResp = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse calendar refresh response: {e}"))?;

    crate::keychain::save_secret(
        dir,
        ACCESS_TOKEN_ACCOUNT,
        ACCESS_TOKEN_FILE,
        &tokens.access_token,
    )?;
    let expiry = Utc::now().timestamp() + tokens.expires_in as i64;
    std::fs::write(expiry_path(dir), expiry.to_string())
        .map_err(|e| format!("failed to save calendar token expiry: {e}"))?;

    Ok(tokens.access_token)
}

// ── Google Calendar API ───────────────────────────────────────────────────────

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn next_date_str(date: &str) -> Result<String, String> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| format!("invalid date '{date}'"))?;
    Ok((d + Duration::days(1)).format("%Y-%m-%d").to_string())
}

// ── Brain write tools ─────────────────────────────────────────────────────────
