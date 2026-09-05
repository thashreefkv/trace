use std::path::{Path, PathBuf};

use serde::Deserialize;

const SCOPE: &str =
    "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.send https://www.googleapis.com/auth/gmail.modify";
const REFRESH_TOKEN_ACCOUNT: &str = "google-gmail-refresh-token";
const ACCESS_TOKEN_ACCOUNT: &str = "google-gmail-access-token";
const REFRESH_TOKEN_FILE: &str = "gmail_refresh_token";
const ACCESS_TOKEN_FILE: &str = "gmail_access_token";

pub(super) fn expiry_path(dir: &Path) -> PathBuf {
    dir.join("gmail_access_token_expiry")
}

pub fn gmail_connected(dir: &Path) -> bool {
    crate::keychain::get_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE)
        .map(|token| token.is_some())
        .unwrap_or(false)
}

pub fn gmail_disconnect(dir: &Path) -> Result<(), String> {
    crate::keychain::clear_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE)?;
    crate::keychain::clear_secret(dir, ACCESS_TOKEN_ACCOUNT, ACCESS_TOKEN_FILE)?;
    let path = expiry_path(dir);
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to remove Gmail token expiry: {error}"))?;
    }
    Ok(())
}

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
    let expiry = chrono::Utc::now().timestamp() + tokens.expires_in as i64;
    std::fs::write(expiry_path(dir), expiry.to_string())
        .map_err(|e| format!("failed to save token expiry: {e}"))?;

    Ok(())
}

pub async fn get_valid_access_token(dir: &Path) -> Result<String, String> {
    // Return cached token if still valid (with 60-second buffer)
    if let Ok(expiry_str) = std::fs::read_to_string(expiry_path(dir)) {
        if let Ok(expiry) = expiry_str.trim().parse::<i64>() {
            if chrono::Utc::now().timestamp() + 60 < expiry {
                if let Some(token) =
                    crate::keychain::get_secret(dir, ACCESS_TOKEN_ACCOUNT, ACCESS_TOKEN_FILE)?
                {
                    return Ok(token);
                }
            }
        }
    }

    // Refresh
    let refresh_token =
        crate::keychain::get_secret(dir, REFRESH_TOKEN_ACCOUNT, REFRESH_TOKEN_FILE)?
            .ok_or_else(|| "Gmail not connected".to_string())?;
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
        .map_err(|e| format!("token refresh request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Gmail token refresh failed with status {}",
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
    let expiry = chrono::Utc::now().timestamp() + tokens.expires_in as i64;
    std::fs::write(expiry_path(dir), expiry.to_string())
        .map_err(|e| format!("failed to save token expiry: {e}"))?;

    Ok(tokens.access_token)
}
