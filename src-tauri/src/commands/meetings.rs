use base64::Engine as _;
use tauri::{AppHandle, Emitter, State};

use crate::{commands::memory::auto_extract_from_text, db::AppState};

#[tauri::command]
pub async fn list_meetings(
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::models::Meeting>, String> {
    project_manager_shared::repo::list_meetings(&state.pool).await
}

#[tauri::command]
pub async fn get_meeting(
    id: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::MeetingWithActions, String> {
    let result = project_manager_shared::repo::get_meeting(&state.pool, &id).await;
    let pool = state.pool.clone();
    let id_clone = id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = project_manager_shared::entity_embeddings::enqueue_high_priority(
            &pool,
            project_manager_shared::entity_embeddings::EntityKind::Meeting,
            &id_clone,
        )
        .await;
    });
    result
}

#[tauri::command]
pub async fn create_meeting(
    input: project_manager_shared::models::CreateMeetingInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::Meeting, String> {
    let meeting = project_manager_shared::repo::create_meeting(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(meeting)
}

#[tauri::command]
pub async fn update_meeting_title(
    id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::Meeting, String> {
    let meeting =
        project_manager_shared::repo::update_meeting_title(&state.pool, &id, &title).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(meeting)
}

#[tauri::command]
pub async fn delete_meeting(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_meeting(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn update_meeting_stakeholders(
    id: String,
    stakeholder_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(project_manager_shared::db::sql_error)?;
    project_manager_shared::repo::replace_meeting_stakeholders(&mut tx, &id, &stakeholder_ids)
        .await?;
    tx.commit()
        .await
        .map_err(project_manager_shared::db::sql_error)?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn process_meeting_audio(
    input: project_manager_shared::models::ProcessMeetingInput,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<project_manager_shared::models::MeetingWithActions, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| "Gemini API key not set. Configure it in Settings.".to_string())?;

    let result = project_manager_shared::gemini::process_meeting_audio(
        &state.pool,
        &api_key,
        &input.audio_base64,
        &input.mime_type,
    )
    .await;

    match result {
        Ok(output) => {
            let saved = project_manager_shared::repo::save_meeting_processed(
                &state.pool,
                &input.meeting_id,
                &output,
                input.duration_secs,
            )
            .await?;
            spawn_meeting_memory_extraction(&state, &app, &saved.meeting);
            crate::commands::brain::spawn_brain_rebuild(state.inner());
            Ok(saved)
        }
        Err(e) => {
            let _ = project_manager_shared::repo::save_meeting_error(
                &state.pool,
                &input.meeting_id,
                &e,
            )
            .await;
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn apply_meeting_action(
    input: project_manager_shared::models::ApplyMeetingActionInput,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::MeetingAction, String> {
    let action = project_manager_shared::repo::apply_meeting_action(&state.pool, input).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(action)
}

#[tauri::command]
pub async fn dismiss_meeting_action(
    action_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    project_manager_shared::repo::dismiss_meeting_action(&state.pool, &action_id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

#[tauri::command]
pub async fn list_initiative_notes(
    initiative_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<project_manager_shared::models::InitiativeNote>, String> {
    project_manager_shared::repo::list_initiative_notes(&state.pool, &initiative_id).await
}

#[tauri::command]
pub async fn create_initiative_note(
    initiative_id: String,
    body: String,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::InitiativeNote, String> {
    let note =
        project_manager_shared::repo::create_initiative_note(&state.pool, &initiative_id, &body)
            .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(note)
}

#[tauri::command]
pub async fn delete_initiative_note(id: String, state: State<'_, AppState>) -> Result<(), String> {
    project_manager_shared::repo::delete_initiative_note(&state.pool, &id).await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());
    Ok(())
}

// ── Agentic minutes upload ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn upload_meeting_minutes(
    file_path: String,
    stakeholder_ids: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::MeetingWithActions, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| "Gemini API key not set. Configure it in Settings.".to_string())?;

    let path = std::path::Path::new(&file_path);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let input = if ext == "pdf" {
        let bytes = std::fs::read(path).map_err(|e| format!("Cannot read file: {e}"))?;
        let pdf_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        project_manager_shared::models::MinutesInput {
            text: None,
            pdf_base64: Some(pdf_base64),
            filename: filename.clone(),
        }
    } else if ext == "docx" {
        let bytes = std::fs::read(path).map_err(|e| format!("Cannot read file: {e}"))?;
        let text = extract_docx_text(&bytes)?;
        project_manager_shared::models::MinutesInput {
            text: Some(text),
            pdf_base64: None,
            filename: filename.clone(),
        }
    } else {
        // txt / md and any other text format
        let text = std::fs::read_to_string(path).map_err(|e| format!("Cannot read file: {e}"))?;
        project_manager_shared::models::MinutesInput {
            text: Some(text),
            pdf_base64: None,
            filename: filename.clone(),
        }
    };

    // Create a meeting session record before processing so the user can navigate to it
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    // Strip extension for a readable initial title
    let initial_title = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace(['_', '-'], " ");
    let meeting = project_manager_shared::repo::create_meeting(
        &state.pool,
        project_manager_shared::models::CreateMeetingInput {
            title: initial_title,
            date: today,
            stakeholder_ids,
        },
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());

    // Progress channel — forward events to frontend as `minutes:progress` events
    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<project_manager_shared::models::AskProgressEvent>();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(evt) = rx.recv().await {
            let _ = app_handle.emit("minutes:progress", &evt);
        }
    });

    match project_manager_shared::gemini::process_minutes_agentic(
        &api_key,
        &input,
        &state.pool,
        Some(tx),
    )
    .await
    {
        Ok(result) => {
            // Persist title, date, summary, and all agent actions into the meeting record
            let saved = project_manager_shared::repo::save_minutes_summary(
                &state.pool,
                &meeting.id,
                &result,
            )
            .await?;
            spawn_meeting_memory_extraction(&state, &app, &saved.meeting);
            crate::commands::brain::spawn_brain_rebuild(state.inner());
            Ok(saved)
        }
        Err(e) => {
            let _ = project_manager_shared::repo::save_meeting_error(&state.pool, &meeting.id, &e)
                .await;
            Err(e)
        }
    }
}

fn spawn_meeting_memory_extraction(
    state: &State<'_, AppState>,
    app: &AppHandle,
    meeting: &project_manager_shared::models::Meeting,
) {
    let pool = state.pool.clone();
    let app_support_dir = state.app_support_dir.clone();
    let app = app.clone();
    let meeting_id = meeting.id.clone();
    let title = meeting.title.clone();
    let date = meeting.date.clone();
    let summary = meeting.summary.clone().unwrap_or_default();
    let transcript = meeting.transcript.clone().unwrap_or_default();
    let key_decisions = meeting.key_decisions.clone().unwrap_or_default();

    tauri::async_runtime::spawn(async move {
        let mut source = format!("Meeting '{title}' on {date}.\n");
        if !summary.is_empty() {
            source.push_str(&format!("\nSummary:\n{summary}\n"));
        }
        if !key_decisions.is_empty() {
            source.push_str(&format!("\nKey decisions:\n{key_decisions}\n"));
        }
        if !transcript.is_empty() {
            // Cap transcript length so the extractor stays in budget.
            let trimmed = if transcript.len() > 8000 {
                format!("{}…", &transcript[..8000])
            } else {
                transcript
            };
            source.push_str(&format!("\nTranscript:\n{trimmed}\n"));
        }
        auto_extract_from_text(
            &pool,
            &app_support_dir,
            &app,
            "meeting",
            Some(&meeting_id),
            &source,
        )
        .await;
    });
}

// ── Import Drive transcript ───────────────────────────────────────────────────

#[tauri::command]
pub async fn import_drive_transcript(
    drive_file_id: String,
    file_name: String,
    stakeholder_ids: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::MeetingWithActions, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| "Gemini API key not set. Configure it in Settings.".to_string())?;

    let text = project_manager_shared::google_drive::export_doc_text(
        &state.app_support_dir,
        &drive_file_id,
        "application/vnd.google-apps.document",
    )
    .await?;

    let input = project_manager_shared::models::MinutesInput {
        text: Some(text),
        pdf_base64: None,
        filename: file_name.clone(),
    };

    let initial_title = std::path::Path::new(&file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().replace(['_', '-'], " "))
        .unwrap_or(file_name.clone());

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let meeting = project_manager_shared::repo::create_meeting(
        &state.pool,
        project_manager_shared::models::CreateMeetingInput {
            title: initial_title,
            date: today,
            stakeholder_ids,
        },
    )
    .await?;
    crate::commands::brain::spawn_brain_rebuild(state.inner());

    let (tx, mut rx) =
        tokio::sync::mpsc::unbounded_channel::<project_manager_shared::models::AskProgressEvent>();
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(evt) = rx.recv().await {
            let _ = app_handle.emit("minutes:progress", &evt);
        }
    });

    match project_manager_shared::gemini::process_minutes_agentic(
        &api_key,
        &input,
        &state.pool,
        Some(tx),
    )
    .await
    {
        Ok(result) => {
            let saved = project_manager_shared::repo::save_minutes_summary(
                &state.pool,
                &meeting.id,
                &result,
            )
            .await?;
            spawn_meeting_memory_extraction(&state, &app, &saved.meeting);
            crate::commands::brain::spawn_brain_rebuild(state.inner());
            Ok(saved)
        }
        Err(e) => {
            let _ = project_manager_shared::repo::save_meeting_error(&state.pool, &meeting.id, &e)
                .await;
            Err(e)
        }
    }
}

// ── Weekly digest ──────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_weekly_digest(
    state: State<'_, AppState>,
) -> Result<project_manager_shared::models::WeeklyDigest, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| "Gemini API key not set. Configure it in Settings.".to_string())?;

    project_manager_shared::gemini::generate_weekly_digest(&api_key, &state.pool).await
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_docx_text(bytes: &[u8]) -> Result<String, String> {
    use std::io::Read;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid DOCX (zip): {e}"))?;

    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|_| "word/document.xml not found in DOCX".to_string())?
        .read_to_string(&mut xml)
        .map_err(|e| format!("Cannot read document.xml: {e}"))?;

    // Strip XML tags, collapse whitespace
    let mut text = String::with_capacity(xml.len() / 2);
    let mut in_tag = false;
    let mut space_pending = false;
    for ch in xml.chars() {
        match ch {
            '<' => {
                in_tag = true;
                space_pending = true;
            }
            '>' => {
                in_tag = false;
            }
            _ if !in_tag => {
                if space_pending {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    space_pending = false;
                }
                text.push(ch);
            }
            _ => {}
        }
    }
    Ok(text.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[derive(serde::Deserialize)]
pub struct TranscribeVoiceInput {
    pub audio_base64: String,
    pub mime_type: String,
}

#[tauri::command]
pub async fn transcribe_voice_input(
    input: TranscribeVoiceInput,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let api_key = project_manager_shared::keychain::get_gemini_api_key(&state.app_support_dir)?
        .ok_or_else(|| "Gemini API key not set. Configure it in Settings.".to_string())?;
    project_manager_shared::gemini::transcribe_audio(
        &state.pool,
        &api_key,
        &input.audio_base64,
        &input.mime_type,
    )
    .await
}
