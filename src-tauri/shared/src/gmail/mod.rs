mod aggregation;
mod ai;
mod api;
mod digest;
mod legacy;
mod linking;
mod linking_actions;
mod links;
mod models;
mod oauth;
mod relevance;
mod send;
mod settings;
mod stakeholders;
mod sync;
mod threads;
mod util;
mod work_mail;

pub use aggregation::{rebuild_global_participants, rebuild_thread_aggregate, update_followups};
pub use ai::{
    analyze_thread_with_gemini, analyze_thread_with_gemini_tagged, draft_reply_with_brain,
    list_analysis_history, list_threads_needing_reanalysis, triage_thread_with_gemini,
    GmailAnalysisSnapshot,
};
pub use api::{
    delete_stale_thread_links, ensure_thread_placeholder, fetch_api_thread, fetch_profile,
    list_api_threads, list_api_threads_page, parse_api_message, request_json, search_threads,
    sync_drafts, sync_labels, upsert_draft, upsert_message,
};
pub use digest::weekly_digest;
pub use legacy::*;
pub use linking::{
    accept_thread_link, auto_link_thread, backfill_stakeholder_thread_links,
    build_graph_context_for_thread, list_orphan_threads, list_thread_link_suggestions,
    reanalyze_stale_threads, refresh_thread_intelligence, reject_thread_link,
};
pub use linking_actions::{
    create_capture_from_thread, create_task_from_thread, link_thread_to_deliverable,
    link_thread_to_initiative, suggest_threads_for_deliverable, unlink_thread_from_deliverable,
};
pub use links::{
    category_counts, labels_for_thread, linked_deliverables_for_thread,
    linked_initiatives_for_thread, linked_stakeholders_for_thread, links_for_thread_urls,
    list_attachments, list_drafts, list_gmail_labels, list_links,
};
pub use models::*;
pub use relevance::{
    delete_local_thread, is_blocked_spam_or_trash, load_relevance_context, local_message_is_low_signal,
    purge_blocked_threads, purge_irrelevant_threads, thread_is_relevant,
};
pub use send::{
    archive_thread, mark_thread_important, mark_thread_read_in_gmail, mark_thread_unread_in_gmail,
    move_thread_to_spam, send_email, star_thread,
};
pub use settings::{get_sync_settings, update_sync_settings};
pub use stakeholders::{
    exclude_thread_from_stakeholder, relationship_graph, stakeholder_health,
    stakeholder_suggestions, stakeholder_threads,
};
pub use sync::{
    auto_analyze_relevant_threads, batch_analyze_unsummarized_threads, sync_due, sync_mailbox,
};
pub use threads::{get_local_thread, list_local_threads, load_message_rows};
pub use work_mail::{
    defer_work_mail_thread, delete_work_mail_domain, exclude_work_mail_thread,
    infer_thread_category, list_work_mail_agent_events, list_work_mail_domains,
    list_work_mail_threads, mark_work_mail_thread_seen, promote_work_mail_thread,
    record_work_mail_agent_event, refresh_work_mail_dimensions,
    refresh_work_mail_review_state, reopen_work_mail_thread, restore_work_mail_thread,
    set_work_mail_review_state, upsert_work_mail_domain, work_mail_brief,
    work_mail_needs_me_reason, work_mail_review_summary, work_mail_view_counts,
    ThreadCategory, WorkMailDimensions,
};
pub use oauth::{
    build_auth_url, complete_oauth, get_valid_access_token, gmail_connected, gmail_disconnect,
};
pub use util::{
    classify_url, email_domain, extract_urls, format_ts, header_value, is_artifact_url,
    max_history, message_preview, normalize_email, now_utc, parse_address_header,
    parse_addresses_json, parse_single_address, parse_string_vec, strip_html, to_json_string,
    truncate_text,
};
