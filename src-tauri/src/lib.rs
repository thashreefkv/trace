mod apple_notes;
mod bg_events;
mod commands;
mod db;
mod http_api;
mod menu_bar;
mod models;

use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let global_shortcut_plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_shortcuts(["CommandOrControl+Shift+M", "CommandOrControl+Shift+Space"])
        .expect("global shortcuts should be valid")
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            // ⌘⇧M → inline capture overlay inside main window.
            if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyM)
                || shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyM)
            {
                let _ = commands::windows::show_main_window_with_capture(app);
                return;
            }
            // ⌘⇧Space → floating Spotlight-style Ask overlay.
            if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::Space)
                || shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::Space)
            {
                let _ = commands::windows::toggle_spotlight_window(app);
            }
        })
        .build();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(global_shortcut_plugin);

    // NSPanel support — must be registered before the first tray window is shown.
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        // On macOS, clicking the red ✕ button on the main window hides it
        // rather than destroying it.  This keeps the process alive so the tray
        // widget continues to work.  The user can still quit via the tray menu
        // "Quit" item or Cmd+Q.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    // Exit fullscreen first so macOS reclaims the dedicated
                    // Space — otherwise hiding the window leaves an empty
                    // black Space behind.
                    if window.is_fullscreen().unwrap_or(false) {
                        let _ = window.set_fullscreen(false);
                    }
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .setup(|app| {
            let app_state = tauri::async_runtime::block_on(db::connect(app.handle()))
                .map_err(std::io::Error::other)?;
            app.manage(app_state);
            let app_state = app.state::<db::AppState>();

            // Seed the shared Gemini API key cache so retrieval can compute
            // query embeddings without plumbing the key through every caller.
            let cached_key =
                project_manager_shared::keychain::get_gemini_api_key(&app_state.app_support_dir)
                    .ok()
                    .flatten();
            project_manager_shared::runtime::set_gemini_api_key(cached_key.clone());

            // Warm the ASK prompt cache once on startup so the first ask of
            // the session benefits from cachedContents pricing.
            if let Some(key) = cached_key {
                tauri::async_runtime::spawn(async move {
                    project_manager_shared::gemini::warm_ask_cache(&key).await;
                });
            }

            commands::brain::start_brain_worker(app_state.inner());
            commands::brain::spawn_brain_rebuild(app_state.inner());
            commands::mcp::install_mcp_server_if_available(app_state.inner());
            commands::gmail::spawn_background_sync(
                app.handle().clone(),
                app_state.pool.clone(),
                app_state.app_support_dir.clone(),
                app_state.brain_path.clone(),
                app_state.brain_rebuild_lock.clone(),
            );
            commands::drive::spawn_drive_background_sync(
                app.handle().clone(),
                app_state.pool.clone(),
                app_state.app_support_dir.clone(),
                app_state.brain_path.clone(),
                app_state.brain_rebuild_lock.clone(),
            );
            commands::calendar::spawn_calendar_background_sync(
                app.handle().clone(),
                app_state.pool.clone(),
                app_state.app_support_dir.clone(),
                app_state.brain_path.clone(),
                app_state.brain_rebuild_lock.clone(),
            );
            tauri::async_runtime::spawn(project_manager_shared::files_watcher::run_watcher(
                app_state.pool.clone(),
                app_state.brain_path.clone(),
                app_state.brain_rebuild_lock.clone(),
            ));
            commands::embedding::spawn_embedding_worker(
                app.handle().clone(),
                app_state.pool.clone(),
                app_state.app_support_dir.clone(),
            );
            tauri::async_runtime::spawn(project_manager_shared::linking::run_linker_worker(
                app_state.pool.clone(),
            ));
            commands::app_config::spawn_budget_monitor(
                app.handle().clone(),
                app_state.pool.clone(),
            );
            commands::captures::spawn_apple_notes_sync(app_state.pool.clone());

            // Hourly recompute of inference confidence thresholds based on
            // accumulated accept/reject events. Idempotent — bounded step
            // per pass so the policy can't oscillate.
            let threshold_pool = app_state.pool.clone();
            tauri::async_runtime::spawn(async move {
                // Stagger the first recompute so it doesn't race startup
                // migrations.
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                loop {
                    if let Err(error) =
                        project_manager_shared::brain::recompute_inference_thresholds(
                            &threshold_pool,
                        )
                        .await
                    {
                        eprintln!("[brain] threshold recompute failed: {error}");
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
                }
            });
            commands::windows::ensure_capture_window(app.handle())
                .map_err(std::io::Error::other)?;
            commands::windows::ensure_spotlight_window(app.handle())
                .map_err(std::io::Error::other)?;

            // Build the Siri remote HTTP API state BEFORE the menu_bar setup,
            // because the latter takes `&mut app` and would invalidate the
            // immutable `app_state` borrow we're reading from here. The
            // server task itself starts after the menu bar so we still spawn
            // it from inside the setup hook (it's a fire-and-forget task and
            // its first I/O happens on a fresh runtime tick).
            let siri_token = project_manager_shared::keychain::get_or_create_siri_token(
                &app_state.app_support_dir,
            )
            .map_err(std::io::Error::other)?;
            let http_state = http_api::HttpApiState::new(
                app_state.pool.clone(),
                app_state.brain_path.clone(),
                app_state.app_support_dir.clone(),
                siri_token,
            );
            let http_state_for_task = http_state.clone();

            menu_bar::setup_menu_bar(app)?;

            // Manage the state so Tauri commands can hot-swap the token
            // after regenerate without restarting the server.
            app.manage(http_state);

            tauri::async_runtime::spawn(async move {
                if let Err(error) = http_api::start(http_state_for_task).await {
                    eprintln!("[siri-api] failed to start: {error}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::initiatives::list_initiatives,
            commands::initiatives::get_initiative,
            commands::initiatives::create_initiative,
            commands::initiatives::update_initiative,
            commands::initiatives::delete_initiative,
            commands::stakeholders::list_stakeholders,
            commands::stakeholders::list_stakeholder_details,
            commands::stakeholders::get_stakeholder_detail,
            commands::stakeholders::create_stakeholder,
            commands::stakeholders::update_stakeholder,
            commands::stakeholders::delete_stakeholder,
            commands::stakeholders::generate_stakeholder_briefing,
            commands::stakeholders::comment_on_briefing_item,
            commands::captures::create_capture,
            commands::captures::list_captures,
            commands::captures::apple_notes_setup,
            commands::captures::apple_notes_sync_now,
            commands::captures::get_capture,
            commands::captures::dismiss_capture,
            commands::captures::suggest_capture,
            commands::captures::restore_capture_to_inbox,
            commands::captures::promote_capture_to_deliverable,
            commands::captures::promote_capture_to_initiative,
            commands::captures::promote_capture_to_task,
            commands::captures::suggest_capture_promotion,
            commands::captures::get_capture_promotion_suggestion,
            commands::captures::get_capture_promotion_accuracy,
            commands::captures::apply_capture_promotion_suggestion,
            commands::captures::undo_capture_promotion,
            commands::captures::record_capture_promotion_event,
            commands::search::confirm_ask_tool_decision,
            commands::search::list_prompt_injection_log,
            commands::conversations::extract_conversation,
            commands::conversations::commit_conversation_ingest,
            commands::conversations::promote_claude_capture_to_ingest,
            commands::conversations::get_conversation,
            commands::conversations::list_conversations,
            commands::deliverables::list_deliverables,
            commands::deliverables::get_deliverable,
            commands::deliverables::create_deliverable,
            commands::deliverables::update_deliverable,
            commands::deliverables::delete_deliverable,
            commands::deliverables::update_deliverable_state,
            commands::deliverables::search_deliverables,
            commands::deliverables::list_deliverables_for_initiative,
            commands::deliverables::list_deliverable_tasks,
            commands::deliverables::create_deliverable_task,
            commands::deliverables::update_deliverable_task,
            commands::deliverables::delete_deliverable_task,
            commands::deliverables::reorder_deliverable_task,
            commands::deliverables::list_deliverable_notes,
            commands::deliverables::create_deliverable_note,
            commands::deliverables::delete_deliverable_note,
            commands::deliverables::update_deliverable_metadata,
            commands::deliverables::set_deliverable_focus,
            commands::deliverables::generate_deliverable_tasks,
            commands::deliverables::apply_generated_deliverable_tasks,
            commands::deliverables::list_labels,
            commands::deliverables::create_label,
            commands::deliverables::delete_label,
            commands::deliverables::assign_label_to_deliverable,
            commands::deliverables::remove_label_from_deliverable,
            commands::deliverables::reorder_deliverable,
            commands::deliverables::reorder_deliverable_within_state,
            commands::deliverables::update_deliverable_state_friction,
            commands::deliverables::apply_flagged_to_backlog,
            commands::context::get_work_context_graph,
            commands::brain::get_brain_status,
            commands::brain::rebuild_brain,
            commands::brain::get_brain_graph,
            commands::brain::retrieve_brain_context,
            commands::brain::query_brain_cypher,
            commands::brain::run_brain_template,
            commands::brain::get_daily_brain_brief,
            commands::brain::record_brain_feedback,
            commands::brain::record_brain_learning_event,
            commands::brain::get_brain_learning_snapshot,
            commands::brain::get_brain_learning_summary,
            // Section 6.2 — RL feedback surface
            commands::brain::list_brain_inferences,
            commands::brain::review_inference,
            commands::brain::list_inference_supersessions,
            commands::brain::revert_inference_supersession,
            commands::brain::get_rl_digest,
            commands::brain::get_template_detail,
            commands::brain::reset_brain_template_learning,
            // Phase 2 brain explorer
            commands::brain::list_saved_brain_views,
            commands::brain::save_brain_view,
            commands::brain::delete_brain_view,
            commands::brain::list_graph_communities,
            // Phase 3 brain explorer: layout cache + embeddings
            commands::brain::get_brain_layout,
            commands::brain::write_brain_layout,
            commands::brain::invalidate_brain_layouts,
            commands::brain::get_node_embeddings,
            commands::reasoning::query_reasoning_graph,
            commands::reasoning::refresh_reasoning_index,
            commands::reasoning::list_reasoning_runs,
            commands::reasoning::list_claim_review_items,
            commands::reasoning::review_claim,
            commands::reasoning::list_community_reports,
            commands::reasoning::review_community_report,
            commands::reasoning::list_reasoning_review_items,
            commands::reasoning::review_reasoning_item,
            commands::memory::get_memory_settings,
            commands::memory::update_memory_settings,
            commands::memory::list_memories,
            commands::memory::create_memory,
            commands::memory::update_memory,
            commands::memory::delete_memory,
            commands::memory::retrieve_memories,
            commands::memory::consolidate_memories,
            commands::memory::extract_memories_from_conversation,
            commands::memory::record_memory_feedback,
            commands::memory::list_memory_events,
            commands::memory::embed_memories_now,
            commands::export::export_markdown,
            commands::reports::list_report_runs,
            commands::reports::get_report_run,
            commands::reports::create_report_run,
            commands::reports::delete_report_run,
            commands::reports::generate_report_outline,
            commands::reports::approve_report_outline,
            commands::reports::generate_report_draft,
            commands::reports::approve_report,
            commands::reports::export_report,
            commands::reports::list_report_exports,
            commands::reports::link_report_deliverable,
            commands::reports::create_report_deliverable,
            commands::reports::run_report_pipeline,
            commands::reports::get_report_scope_preview,
            commands::reports::replace_scope_exclusions,
            commands::reports::update_report_sections,
            commands::reports::list_report_steps,
            commands::reports::answer_report_clarification,
            commands::reports::rerun_report_step,
            commands::reports::steer_report_cmd,
            commands::mcp::get_mcp_install_state,
            commands::mcp::install_mcp_server,
            commands::mcp::get_mcp_config_snippet,
            commands::mcp::open_mcp_log,
            commands::mcp::save_gemini_api_key,
            commands::mcp::clear_gemini_api_key,
            commands::mcp::test_gemini_api_key,
            commands::mcp::get_gemini_key_status,
            commands::mcp::list_tool_call_log,
            commands::mcp::get_gemini_usage_summary,
            commands::eval::list_eval_fixtures,
            commands::eval::create_eval_fixture,
            commands::eval::delete_eval_fixture,
            commands::eval::run_eval_fixture,
            commands::eval::run_all_evals,
            commands::eval::list_eval_runs,
            commands::eval::set_eval_baseline,
            commands::eval::get_eval_summary,
            commands::eval::import_eval_fixtures,
            commands::embedding::get_embedding_progress,
            commands::embedding::enqueue_all_embeddings,
            commands::embedding::embed_now,
            commands::week::get_week_view,
            commands::week::set_day_deliverable,
            commands::week::get_meeting_config,
            commands::week::set_meeting_date,
            commands::week::advance_meeting_date,
            commands::week::generate_week_plan,
            commands::meetings::list_meetings,
            commands::meetings::get_meeting,
            commands::meetings::create_meeting,
            commands::meetings::update_meeting_title,
            commands::meetings::delete_meeting,
            commands::meetings::process_meeting_audio,
            commands::meetings::transcribe_voice_input,
            commands::meetings::apply_meeting_action,
            commands::meetings::dismiss_meeting_action,
            commands::meetings::list_initiative_notes,
            commands::meetings::create_initiative_note,
            commands::meetings::delete_initiative_note,
            commands::meetings::upload_meeting_minutes,
            commands::meetings::import_drive_transcript,
            commands::meetings::update_meeting_stakeholders,
            commands::meetings::generate_weekly_digest,
            commands::windows::show_capture_window,
            commands::windows::hide_capture_window,
            commands::windows::show_main_window,
            commands::windows::show_spotlight_window,
            commands::windows::hide_spotlight_window,
            commands::windows::toggle_spotlight_window_cmd,
            menu_bar::get_menu_bar_state,
            menu_bar::refresh_menu_bar,
            // Search
            commands::search::search_all,
            commands::search::ask_search,
            commands::search::start_ask_run,
            commands::search::cancel_ask_run,
            commands::search::list_ask_chats,
            commands::search::get_ask_chat,
            commands::search::upsert_ask_chat,
            commands::search::delete_ask_chat,
            commands::search::archive_ask_chat,
            commands::search::append_ask_turn,
            commands::search::search_ask_chats,
            // Gantt
            commands::gantt::get_initiative_gantt,
            commands::gantt::create_initiative_section,
            commands::gantt::update_initiative_section,
            commands::gantt::delete_initiative_section,
            commands::gantt::update_deliverable_gantt_dates,
            commands::gantt::set_deliverable_section,
            // Gmail
            commands::gmail::gmail_status,
            commands::gmail::gmail_connect,
            commands::gmail::gmail_disconnect,
            commands::gmail::gmail_search_threads,
            commands::gmail::gmail_get_sync_settings,
            commands::gmail::gmail_update_sync_settings,
            commands::gmail::gmail_sync_now,
            commands::gmail::gmail_list_local_threads,
            commands::gmail::gmail_list_work_mail_threads,
            commands::gmail::gmail_work_mail_view_counts,
            commands::gmail::gmail_work_mail_brief,
            commands::gmail::gmail_list_work_mail_domains,
            commands::gmail::gmail_upsert_work_mail_domain,
            commands::gmail::gmail_delete_work_mail_domain,
            commands::gmail::gmail_list_work_mail_agent_events,
            commands::gmail::gmail_mark_work_mail_thread_seen,
            commands::gmail::gmail_set_work_mail_review_state,
            commands::gmail::gmail_defer_work_mail_thread,
            commands::gmail::gmail_reopen_work_mail_thread,
            commands::gmail::gmail_promote_work_mail_thread,
            commands::gmail::gmail_restore_work_mail_thread,
            commands::gmail::gmail_exclude_work_mail_thread,
            commands::gmail::gmail_get_local_thread,
            commands::gmail::gmail_list_labels,
            commands::gmail::gmail_list_drafts,
            commands::gmail::gmail_link_thread_to_deliverable,
            commands::gmail::gmail_unlink_thread_from_deliverable,
            commands::gmail::gmail_link_thread_to_initiative,
            commands::gmail::gmail_suggest_threads_for_deliverable,
            commands::gmail::gmail_stakeholder_threads,
            commands::gmail::gmail_exclude_thread_from_stakeholder,
            commands::gmail::gmail_stakeholder_health,
            commands::gmail::gmail_stakeholder_suggestions,
            commands::gmail::gmail_relationship_graph,
            commands::gmail::gmail_create_capture_from_thread,
            commands::gmail::gmail_create_task_from_thread,
            commands::gmail::gmail_analyze_thread,
            commands::gmail::gmail_batch_analyze,
            commands::gmail::gmail_reanalyze_stale_threads,
            commands::gmail::gmail_get_effective_classification,
            commands::gmail::gmail_get_thread_override,
            commands::gmail::gmail_set_thread_override,
            commands::gmail::gmail_clear_thread_override,
            commands::gmail::gmail_list_sender_rules,
            commands::gmail::gmail_create_sender_rule,
            commands::gmail::gmail_delete_sender_rule,
            commands::gmail::gmail_toggle_sender_rule,
            commands::gmail::gmail_inbox_dashboard,
            commands::gmail::gmail_calibration_report,
            commands::gmail::gmail_retry_failed_analyses,
            commands::gmail::gmail_auto_link_thread,
            commands::gmail::gmail_list_orphan_threads,
            commands::gmail::gmail_list_thread_link_suggestions,
            commands::gmail::gmail_accept_thread_link,
            commands::gmail::gmail_reject_thread_link,
            commands::gmail::gmail_triage_thread,
            commands::gmail::gmail_weekly_digest,
            commands::gmail::gmail_send_email,
            commands::gmail::gmail_archive_thread,
            commands::gmail::gmail_move_thread_to_spam,
            commands::gmail::gmail_mark_thread_important,
            commands::gmail::gmail_star_thread,
            commands::gmail::gmail_mark_thread_read_in_gmail,
            commands::gmail::gmail_mark_thread_unread_in_gmail,
            commands::gmail::gmail_category_counts,
            commands::gmail::gmail_generate_work_intake,
            // Local drafts + agentic AI draft
            commands::gmail::gmail_get_local_draft,
            commands::gmail::gmail_save_local_draft,
            commands::gmail::gmail_delete_local_draft,
            commands::gmail::gmail_add_draft_attachment,
            commands::gmail::gmail_remove_draft_attachment,
            commands::gmail::gmail_draft_reply_with_brain,
            commands::gmail::gmail_list_analysis_history,
            // Work intake
            commands::work_intake::list_work_intake_suggestions,
            commands::work_intake::generate_workspace_work_intake,
            commands::work_intake::approve_work_intake_suggestion,
            commands::work_intake::dismiss_work_intake_suggestion,
            // Files
            commands::files::list_trace_folders,
            commands::files::list_folder_children,
            commands::files::create_trace_folder,
            commands::files::rename_trace_folder,
            commands::files::move_trace_folder,
            commands::files::delete_trace_folder,
            commands::files::attach_local_file,
            commands::files::attach_local_directory,
            commands::files::move_file,
            commands::files::rename_file,
            commands::files::delete_file,
            commands::files::link_file_to_entity,
            commands::files::unlink_file_from_entity,
            commands::files::files_for_entity,
            commands::files::link_folder_files_to_entity,
            commands::files::link_folder_to_entity,
            commands::files::unlink_folder_from_entity,
            commands::files::folder_links,
            commands::files::folders_for_entity,
            commands::files::open_file,
            commands::files::search_files,
            commands::files::resolve_entity_folder,
            commands::files::open_drive_preview,
            // Google Docs / Slides
            commands::files::get_google_doc,
            commands::files::save_google_doc,
            commands::files::get_google_slides,
            commands::files::get_slide_thumbnail,
            commands::files::get_google_sheet,
            // Google Drive
            commands::drive::drive_status,
            commands::drive::drive_connect,
            commands::drive::drive_disconnect,
            commands::drive::drive_list_children,
            commands::drive::drive_import,
            commands::drive::drive_import_folder,
            commands::drive::drive_pull_changes,
            commands::drive::drive_sync_status,
            commands::drive::drive_has_editor_scope,
            commands::drive::drive_get_file_metadata,
            commands::drive::get_gmeet_folder,
            commands::drive::set_gmeet_folder,
            commands::drive::clear_gmeet_folder,
            // Google Calendar
            commands::calendar::gcal_status,
            commands::calendar::gcal_connect,
            commands::calendar::gcal_disconnect,
            commands::calendar::gcal_sync,
            commands::calendar::gcal_stakeholder_events,
            commands::calendar::create_gcal_meeting,
            commands::calendar::delete_gcal_meeting,
            commands::calendar::open_external_url,
            commands::profile::get_user_profile,
            commands::profile::update_user_profile,
            commands::app_config::get_app_config,
            commands::app_config::set_app_config,
            commands::app_config::get_budget_status,
            commands::app_config::get_gemini_daily_trend,
            // Siri / Apple Shortcuts remote API
            commands::siri_api::get_siri_api_status,
            commands::siri_api::get_siri_token,
            commands::siri_api::regenerate_siri_token,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
