use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::{
    db::AppState,
    models::{
        AttachLocalFileInput, CreateTraceFolderInput, FileLinkRef, FileRow, FilesForEntityInput,
        FolderListing, LinkFileInput, LinkFolderFilesInput, LinkFolderInput, MoveFileInput,
        MoveTraceFolderInput, RenameFileInput, RenameTraceFolderInput, TraceFolder,
        TraceFolderWithLinks,
    },
};

#[tauri::command]
pub async fn list_trace_folders(
    parent_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<TraceFolder>, String> {
    project_manager_shared::files::list_trace_folders(&state.pool, parent_id.as_deref()).await
}

#[tauri::command]
pub async fn list_folder_children(
    folder_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<FolderListing, String> {
    project_manager_shared::files::list_folder_children(&state.pool, folder_id.as_deref()).await
}

#[tauri::command]
pub async fn create_trace_folder(
    input: CreateTraceFolderInput,
    state: State<'_, AppState>,
) -> Result<TraceFolder, String> {
    let folder = project_manager_shared::files::create_folder(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(folder)
}

#[tauri::command]
pub async fn rename_trace_folder(
    input: RenameTraceFolderInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::files::rename_folder(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn move_trace_folder(
    input: MoveTraceFolderInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::files::move_folder(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn delete_trace_folder(
    id: String,
    cascade: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::files::delete_folder(&state.pool, &id, cascade.unwrap_or(false))
        .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn attach_local_file(
    input: AttachLocalFileInput,
    state: State<'_, AppState>,
) -> Result<FileRow, String> {
    let file = project_manager_shared::files::attach_local_file(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(file)
}

#[tauri::command]
pub async fn attach_local_directory(
    path: String,
    trace_folder_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<TraceFolder, String> {
    let folder = project_manager_shared::files::attach_local_directory(
        &state.pool,
        &path,
        trace_folder_id.as_deref(),
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(folder)
}

#[tauri::command]
pub async fn move_file(input: MoveFileInput, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::files::move_file(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn rename_file(input: RenameFileInput, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::files::rename_file(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn delete_file(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::files::delete_file(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn link_file_to_entity(
    input: LinkFileInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::files::link_file(
        &state.pool,
        &input.file_id,
        input.entity_kind,
        &input.entity_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn unlink_file_from_entity(
    input: LinkFileInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::files::unlink_file(
        &state.pool,
        &input.file_id,
        input.entity_kind,
        &input.entity_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn files_for_entity(
    input: FilesForEntityInput,
    state: State<'_, AppState>,
) -> Result<Vec<FileRow>, String> {
    project_manager_shared::files::files_for_entity(
        &state.pool,
        input.entity_kind,
        &input.entity_id,
    )
    .await
}

#[tauri::command]
pub async fn link_folder_to_entity(
    input: LinkFolderInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::files::link_folder(
        &state.pool,
        &input.folder_id,
        input.entity_kind,
        &input.entity_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn unlink_folder_from_entity(
    input: LinkFolderInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::files::unlink_folder(
        &state.pool,
        &input.folder_id,
        input.entity_kind,
        &input.entity_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn folder_links(
    folder_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileLinkRef>, String> {
    project_manager_shared::files::folder_links(&state.pool, &folder_id).await
}

#[tauri::command]
pub async fn folders_for_entity(
    input: FilesForEntityInput,
    state: State<'_, AppState>,
) -> Result<Vec<TraceFolderWithLinks>, String> {
    project_manager_shared::files::folders_for_entity(
        &state.pool,
        input.entity_kind,
        &input.entity_id,
    )
    .await
}

#[tauri::command]
pub async fn link_folder_files_to_entity(
    input: LinkFolderFilesInput,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let count = project_manager_shared::files::link_folder_files_to_entity(
        &state.pool,
        &input.folder_id,
        input.entity_kind,
        &input.entity_id,
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(count)
}

#[tauri::command]
pub async fn open_file(
    id: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let file = project_manager_shared::files::get_file(&state.pool, &id).await?;
    let opener = app.opener();
    match file.kind.as_str() {
        "local" => {
            let path = file
                .local_path
                .ok_or_else(|| "Local file path is missing".to_string())?;
            opener
                .open_path(path, None::<&str>)
                .map_err(|e| format!("Failed to open file: {e}"))
        }
        "drive" => {
            let url = file
                .drive_web_view_link
                .ok_or_else(|| "Drive file has no web link".to_string())?;
            let parsed = url::Url::parse(&url)
                .map_err(|_| "Drive file has an invalid web link".to_string())?;
            let host = parsed.host_str().unwrap_or_default();
            if parsed.scheme() != "https"
                || !matches!(host, "drive.google.com" | "docs.google.com")
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err("Drive links must use an approved Google HTTPS host".to_string());
            }
            opener
                .open_url(parsed.as_str(), None::<&str>)
                .map_err(|e| format!("Failed to open URL: {e}"))
        }
        _ => Err(format!("Unknown file kind: {}", file.kind)),
    }
}

#[tauri::command]
pub async fn search_files(
    query: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<FileRow>, String> {
    project_manager_shared::files::search_files(&state.pool, &query, limit.unwrap_or(50)).await
}

#[tauri::command]
pub async fn resolve_entity_folder(
    entity_kind: String,
    entity_id: String,
    state: State<'_, AppState>,
) -> Result<TraceFolder, String> {
    let folder =
        project_manager_shared::files::ensure_entity_folder(&state.pool, &entity_kind, &entity_id)
            .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(folder)
}

// ── Google Docs / Slides ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_google_doc(
    drive_file_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let doc =
        project_manager_shared::google_docs::get_document(&state.app_support_dir, &drive_file_id)
            .await?;
    serde_json::to_value(doc).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_google_doc(
    drive_file_id: String,
    requests: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let reqs = requests.as_array().cloned().unwrap_or_default();
    project_manager_shared::google_docs::batch_update(&state.app_support_dir, &drive_file_id, reqs)
        .await
}

#[tauri::command]
pub async fn get_google_slides(
    drive_file_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    project_manager_shared::google_docs::get_presentation(&state.app_support_dir, &drive_file_id)
        .await
}

#[tauri::command]
pub async fn get_slide_thumbnail(
    presentation_id: String,
    page_object_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    project_manager_shared::google_docs::get_slide_thumbnail(
        &state.app_support_dir,
        &presentation_id,
        &page_object_id,
    )
    .await
}

#[tauri::command]
pub async fn get_google_sheet(
    drive_file_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    project_manager_shared::google_docs::get_spreadsheet(&state.app_support_dir, &drive_file_id)
        .await
}

#[tauri::command]
pub async fn open_drive_preview(
    app: tauri::AppHandle,
    file_id: String,
    drive_file_id: String,
    drive_mime: String,
    title: String,
) -> Result<(), String> {
    let _ = (file_id, title);
    if drive_file_id.is_empty()
        || !drive_file_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("Invalid Google Drive file identifier".to_string());
    }
    let preview_url = match drive_mime.as_str() {
        "application/vnd.google-apps.document" => {
            format!("https://docs.google.com/document/d/{drive_file_id}/preview")
        }
        "application/vnd.google-apps.spreadsheet" => {
            format!("https://docs.google.com/spreadsheets/d/{drive_file_id}/preview")
        }
        "application/vnd.google-apps.presentation" => {
            format!("https://docs.google.com/presentation/d/{drive_file_id}/preview")
        }
        _ => format!("https://drive.google.com/file/d/{drive_file_id}/preview"),
    };
    app.opener()
        .open_url(preview_url, None::<&str>)
        .map_err(|e| format!("Failed to open preview: {e}"))
}
