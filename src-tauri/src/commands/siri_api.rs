//! Tauri commands backing the Settings → Siri / Remote Access card.
//!
//! The HTTP server itself runs inside the Tauri process (see
//! `src/http_api.rs`). These commands expose three things to the frontend:
//!
//!   1. `get_siri_api_status` — whether Tailscale is up, the URL to paste
//!      into the Shortcut, and the masked token preview.
//!   2. `get_siri_token` — the full token, intentionally separate so it
//!      doesn't ride along with every status refresh and end up in logs.
//!   3. `regenerate_siri_token` — mints a fresh token, persists it, and
//!      hot-swaps the in-memory value so existing requests using the old
//!      token immediately start receiving 401s.

use serde::Serialize;
use tauri::State;

use crate::{db::AppState, http_api};

#[derive(Debug, Serialize)]
pub struct SiriApiStatus {
    /// True if Tailscale is running and the server was able to bind.
    /// (We re-detect on every call so the UI reflects toggling Tailscale.)
    pub running: bool,
    pub port: u16,
    pub tailscale_ipv4: Option<String>,
    /// The URL the user pastes into the Shortcut. `None` when Tailscale
    /// isn't detected.
    pub tailscale_url: Option<String>,
    pub localhost_url: String,
    /// First 8 + last 4 chars of the token, with the middle redacted.
    /// Safe to display in a card without a click-to-reveal.
    pub token_preview: String,
}

#[tauri::command]
pub async fn get_siri_api_status(
    app_state: State<'_, AppState>,
    http_state: State<'_, http_api::HttpApiState>,
) -> Result<SiriApiStatus, String> {
    let snapshot = http_api::detect_status();
    let token = project_manager_shared::keychain::get_siri_token(&app_state.app_support_dir)?
        .unwrap_or_default();
    // Defensive: if the on-disk token has somehow diverged from the in-memory
    // one (e.g. another process wrote it), realign the in-memory cache.
    {
        let current = http_state.token.read().await.clone();
        if !token.is_empty() && current != token {
            http_state.set_token(token.clone()).await;
        }
    }
    Ok(SiriApiStatus {
        running: snapshot.tailscale_ipv4.is_some(),
        port: snapshot.port,
        tailscale_ipv4: snapshot.tailscale_ipv4,
        tailscale_url: snapshot.tailscale_url,
        localhost_url: snapshot.localhost_url,
        token_preview: token_preview(&token),
    })
}

#[tauri::command]
pub async fn get_siri_token(app_state: State<'_, AppState>) -> Result<String, String> {
    project_manager_shared::keychain::get_or_create_siri_token(&app_state.app_support_dir)
}

#[tauri::command]
pub async fn regenerate_siri_token(
    app_state: State<'_, AppState>,
    http_state: State<'_, http_api::HttpApiState>,
) -> Result<String, String> {
    let new_token =
        project_manager_shared::keychain::regenerate_siri_token(&app_state.app_support_dir)?;
    http_state.set_token(new_token.clone()).await;
    Ok(new_token)
}

fn token_preview(token: &str) -> String {
    if token.is_empty() {
        return "(not generated)".to_string();
    }
    if token.len() <= 12 {
        // Pathologically short — just fully mask it.
        return "•".repeat(token.len());
    }
    let head: String = token.chars().take(8).collect();
    let tail: String = token
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_preview_masks_middle() {
        let preview = token_preview("0123456789abcdef0123456789abcdef");
        assert_eq!(preview, "01234567…cdef");
    }

    #[test]
    fn token_preview_handles_empty() {
        assert_eq!(token_preview(""), "(not generated)");
    }

    #[test]
    fn token_preview_fully_masks_short_token() {
        assert_eq!(token_preview("short"), "•••••");
    }
}
