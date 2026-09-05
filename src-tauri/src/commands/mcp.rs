use std::{
    fs,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::{
    db::AppState,
    models::{GeminiKeyStatus, GeminiTestResult, McpConfigSnippet, McpInstallState},
};

const MCP_BINARY_NAME: &str = "mcp-server";
const MCP_SNIPPET_NAME: &str = "claude_desktop_config.snippet.json";

#[tauri::command]
pub async fn get_mcp_install_state(state: State<'_, AppState>) -> Result<McpInstallState, String> {
    Ok(mcp_install_state(&state, None))
}

#[tauri::command]
pub async fn install_mcp_server(state: State<'_, AppState>) -> Result<McpInstallState, String> {
    match install_mcp_server_inner(&state) {
        Ok(()) => Ok(mcp_install_state(&state, None)),
        Err(error) => Ok(mcp_install_state(&state, Some(error))),
    }
}

#[tauri::command]
pub async fn get_mcp_config_snippet(
    state: State<'_, AppState>,
) -> Result<McpConfigSnippet, String> {
    let snippet = mcp_config_snippet(&state)?;
    let path = snippet_path(&state);
    fs::write(&path, &snippet)
        .map_err(|error| format!("failed to write MCP config snippet: {error}"))?;

    Ok(McpConfigSnippet {
        config_path: claude_desktop_config_path(),
        snippet,
    })
}

#[tauri::command]
pub async fn open_mcp_log(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let log_path = project_manager_shared::mcp_log::log_path(&state.app_support_dir);
    if !log_path.exists() {
        fs::write(&log_path, "")
            .map_err(|error| format!("failed to create MCP log file: {error}"))?;
    }

    app.opener()
        .open_path(log_path.to_string_lossy().to_string(), None::<String>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_gemini_api_key(
    key: String,
    state: State<'_, AppState>,
) -> Result<GeminiKeyStatus, String> {
    project_manager_shared::keychain::save_gemini_api_key(&state.app_support_dir, &key)?;
    project_manager_shared::runtime::set_gemini_api_key(Some(key));
    Ok(GeminiKeyStatus { configured: true })
}

#[tauri::command]
pub async fn clear_gemini_api_key(state: State<'_, AppState>) -> Result<GeminiKeyStatus, String> {
    project_manager_shared::keychain::clear_gemini_api_key(&state.app_support_dir)?;
    project_manager_shared::runtime::set_gemini_api_key(None);
    Ok(GeminiKeyStatus { configured: false })
}

#[tauri::command]
pub async fn get_gemini_key_status(state: State<'_, AppState>) -> Result<GeminiKeyStatus, String> {
    Ok(GeminiKeyStatus {
        configured: project_manager_shared::keychain::gemini_api_key_configured(
            &state.app_support_dir,
        )?,
    })
}

#[tauri::command]
pub async fn list_tool_call_log(
    filter: Option<project_manager_shared::tool_audit::ToolCallLogFilter>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::tool_audit::ToolCallLogSnapshot, String> {
    project_manager_shared::tool_audit::list(&state.pool, filter.unwrap_or_default()).await
}

#[tauri::command]
pub async fn get_gemini_usage_summary(
    hours: Option<i64>,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::gemini_usage::UsageSummary, String> {
    let hours = hours.unwrap_or(24).clamp(1, 24 * 30);
    project_manager_shared::gemini_usage::summary_for_hours(&state.pool, hours).await
}

#[tauri::command]
pub async fn test_gemini_api_key(state: State<'_, AppState>) -> Result<GeminiTestResult, String> {
    let Some(key) = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
    else {
        return Ok(GeminiTestResult {
            ok: false,
            message: "No Gemini API key configured.".to_string(),
        });
    };

    match project_manager_shared::gemini::test_api_key(&state.pool, &key).await {
        Ok(()) => Ok(GeminiTestResult {
            ok: true,
            message: "Gemini API key is valid.".to_string(),
        }),
        Err(error) => Ok(GeminiTestResult {
            ok: false,
            message: error,
        }),
    }
}

pub(crate) fn install_mcp_server_if_available(state: &AppState) {
    let _ = install_mcp_server_inner(state);
}

fn install_mcp_server_inner(state: &AppState) -> Result<(), String> {
    fs::create_dir_all(&state.app_support_dir)
        .map_err(|error| format!("failed to create app support directory: {error}"))?;
    let source = find_mcp_source_binary()?;
    let destination = mcp_binary_path(state);
    fs::copy(&source, &destination).map_err(|error| {
        format!(
            "failed to copy MCP server from {} to {}: {error}",
            source.to_string_lossy(),
            destination.to_string_lossy()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&destination)
            .map_err(|error| format!("failed to inspect MCP binary: {error}"))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&destination, permissions)
            .map_err(|error| format!("failed to mark MCP binary executable: {error}"))?;
    }

    let snippet = mcp_config_snippet(state)?;
    fs::write(snippet_path(state), snippet)
        .map_err(|error| format!("failed to write MCP config snippet: {error}"))?;

    Ok(())
}

fn find_mcp_source_binary() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe()
        .map_err(|error| format!("failed to locate current executable: {error}"))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "current executable has no parent directory".to_string())?;
    let cargo_manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let mut candidates = vec![
        exe_dir.join(MCP_BINARY_NAME),
        exe_dir.join(format!("{MCP_BINARY_NAME}.exe")),
        exe_dir.parent().unwrap_or(exe_dir).join(MCP_BINARY_NAME),
        cargo_manifest_dir
            .join("target")
            .join(profile)
            .join(MCP_BINARY_NAME),
        cargo_manifest_dir
            .join("target")
            .join("release")
            .join(MCP_BINARY_NAME),
    ];

    for dir in candidate_dirs(exe_dir) {
        candidates.extend(find_sidecar_candidates(&dir));
    }

    candidates
        .into_iter()
        .find(|path| path.exists() && is_file(path))
        .ok_or_else(|| "MCP server binary has not been built yet".to_string())
}

fn candidate_dirs(exe_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![exe_dir.to_path_buf()];
    if let Some(parent) = exe_dir.parent() {
        dirs.push(parent.to_path_buf());
        dirs.push(parent.join("Resources"));
    }
    dirs
}

fn find_sidecar_candidates(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| {
                    name.starts_with(&format!("{MCP_BINARY_NAME}-"))
                        || name.starts_with(&format!("{MCP_BINARY_NAME}.exe-"))
                })
                .unwrap_or(false)
        })
        .collect()
}

fn is_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

fn mcp_install_state(state: &AppState, last_error: Option<String>) -> McpInstallState {
    let binary_path = mcp_binary_path(state);
    let log_path = project_manager_shared::mcp_log::log_path(&state.app_support_dir);
    let snippet_path = snippet_path(state);

    McpInstallState {
        installed: binary_path.exists(),
        binary_path: display_path(&binary_path),
        database_path: display_path(&state.database_path),
        log_path: display_path(&log_path),
        snippet_path: display_path(&snippet_path),
        last_error,
    }
}

fn mcp_config_snippet(state: &AppState) -> Result<String, String> {
    let value = serde_json::json!({
        "mcpServers": {
            "trace": {
                "command": display_path(&mcp_binary_path(state)),
                "args": [
                    "--db",
                    display_path(&state.database_path),
                    "--app-support-dir",
                    display_path(&state.app_support_dir)
                ]
            }
        }
    });

    serde_json::to_string_pretty(&value)
        .map_err(|error| format!("failed to serialize MCP config snippet: {error}"))
}

fn mcp_binary_path(state: &AppState) -> PathBuf {
    state.app_support_dir.join(MCP_BINARY_NAME)
}

fn snippet_path(state: &AppState) -> PathBuf {
    state.app_support_dir.join(MCP_SNIPPET_NAME)
}

fn claude_desktop_config_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
    format!("{home}/Library/Application Support/Claude/claude_desktop_config.json")
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
