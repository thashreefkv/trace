use std::{collections::BTreeSet, path::Path};

use chrono::{Duration, Utc};
use sqlx::SqlitePool;

use super::{
    analyze_thread_with_gemini, auto_link_thread, classify_url, delete_local_thread,
    delete_stale_thread_links, ensure_thread_placeholder, extract_urls, fetch_api_thread,
    fetch_profile, get_sync_settings, get_valid_access_token, gmail_connected, is_artifact_url,
    is_blocked_spam_or_trash, list_api_threads, list_api_threads_page, load_relevance_context,
    max_history, now_utc, parse_api_message, purge_blocked_threads, rebuild_global_participants,
    rebuild_thread_aggregate, refresh_thread_intelligence, sync_drafts, sync_labels,
    thread_is_relevant, update_followups, upsert_message,
};
use super::linking::count_orphan_threads;
use super::models::*;

pub async fn sync_mailbox(dir: &Path, pool: &SqlitePool) -> Result<GmailSyncReport, String> {
    if !gmail_connected(dir) {
        return Err("Gmail not connected".to_string());
    }

    let settings = get_sync_settings(pool).await?;
    let started_at = now_utc();
    sqlx::query(
        "UPDATE gmail_sync_settings SET last_sync_started_at = ?, last_error = NULL WHERE id = 1",
    )
    .bind(&started_at)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    match sync_mailbox_inner(dir, pool, &settings, &started_at).await {
        Ok(report) => Ok(report),
        Err(error) => {
            sqlx::query("UPDATE gmail_sync_settings SET last_error = ? WHERE id = 1")
                .bind(&error)
                .execute(pool)
                .await
                .map_err(crate::db::sql_error)?;
            Err(error)
        }
    }
}

async fn sync_mailbox_inner(
    dir: &Path,
    pool: &SqlitePool,
    settings: &GmailSyncSettings,
    started_at: &str,
) -> Result<GmailSyncReport, String> {
    let token = get_valid_access_token(dir).await?;
    let client = reqwest::Client::new();
    let profile = fetch_profile(&client, &token).await.ok();
    let account_email = profile
        .as_ref()
        .and_then(|profile| profile.email_address.clone())
        .or_else(|| settings.account_email.clone());

    let synced_labels = sync_labels(&client, &token, pool).await?;
    let mut state = ThreadSyncState {
        max_history_id: profile.and_then(|profile| profile.history_id),
        ..ThreadSyncState::default()
    };
    for query in sync_queries(settings.include_sent, Some("newer_than:365d")) {
        let api_threads = list_api_threads(
            &client,
            &token,
            &query,
            settings.max_threads_per_sync.max(10) as u32,
        )
        .await?;
        sync_thread_refs(
            &client,
            &token,
            pool,
            api_threads,
            started_at,
            &mut state,
        )
        .await?;
    }

    let before_backfill_threads = state.synced_threads;
    let backfill_queries = sync_queries(settings.include_sent, None);
    let (backfill_page_token, backfill_query, backfill_complete) = if settings.backfill_enabled {
        let stored_query = settings.backfill_query.as_deref();
        let stored_index = stored_query.and_then(|query| {
            backfill_queries
                .iter()
                .position(|candidate| candidate == query)
        });
        let completed_stored_query =
            settings.backfill_completed_at.is_some() && settings.backfill_page_token.is_none();
        let query_index = if completed_stored_query {
            stored_index.map(|index| index + 1).unwrap_or(0)
        } else {
            stored_index.unwrap_or(0)
        };

        if query_index >= backfill_queries.len() {
            (None, settings.backfill_query.clone(), true)
        } else {
            let active_query = backfill_queries[query_index].clone();
            let page_token = settings
                .backfill_page_token
                .as_deref()
                .filter(|_| stored_query == Some(active_query.as_str()));
            let page = list_api_threads_page(
                &client,
                &token,
                &active_query,
                settings.max_threads_per_sync.max(10) as u32,
                page_token,
            )
            .await?;
            let ApiThreadPage {
                threads,
                next_page_token,
            } = page;
            sync_thread_refs(
                &client,
                &token,
                pool,
                threads,
                started_at,
                &mut state,
            )
            .await?;
            let query_complete = next_page_token.is_none();
            if query_complete && query_index + 1 < backfill_queries.len() {
                (None, Some(backfill_queries[query_index + 1].clone()), false)
            } else {
                (next_page_token, Some(active_query), query_complete)
            }
        }
    } else {
        (
            settings.backfill_page_token.clone(),
            settings.backfill_query.clone(),
            settings.backfill_completed_at.is_some(),
        )
    };
    let backfilled_threads = state.synced_threads - before_backfill_threads;

    let synced_drafts = if settings.include_drafts {
        sync_drafts(&client, &token, pool, started_at).await?
    } else {
        0
    };

    let mut auto_linked_threads = 0i64;
    for thread_id in state.touched_threads.clone() {
        rebuild_thread_aggregate(pool, &thread_id, started_at).await?;
        let link_report = auto_link_thread(pool, &thread_id).await?;
        if link_report.linked_stakeholders
            + link_report.linked_deliverables
            + link_report.linked_initiatives
            > 0
        {
            auto_linked_threads += 1;
        }
    }
    let analysis_report = if settings.auto_analyze_enabled {
        auto_analyze_relevant_threads(dir, pool, settings.auto_analyze_limit, Some(started_at))
            .await
    } else {
        AutoAnalyzeReport::default()
    };
    let purged_threads = purge_blocked_threads(pool).await?;
    rebuild_global_participants(pool).await?;
    update_followups(pool).await?;
    let orphan_threads = count_orphan_threads(pool).await.unwrap_or(0);

    let completed_at = now_utc();
    sqlx::query(
        r#"
        UPDATE gmail_sync_settings
        SET account_email = ?,
            last_sync_completed_at = ?,
            last_history_id = COALESCE(?, last_history_id),
            backfill_page_token = ?,
            backfill_query = ?,
            last_backfill_at = CASE WHEN ? THEN ? ELSE last_backfill_at END,
            backfill_completed_at = CASE WHEN ? THEN ? ELSE NULL END,
            last_error = NULL
        WHERE id = 1
        "#,
    )
    .bind(&account_email)
    .bind(&completed_at)
    .bind(&state.max_history_id)
    .bind(&backfill_page_token)
    .bind(&backfill_query)
    .bind(settings.backfill_enabled)
    .bind(&completed_at)
    .bind(backfill_complete)
    .bind(&completed_at)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    Ok(GmailSyncReport {
        synced_threads: state.synced_threads,
        synced_messages: state.synced_messages,
        backfilled_threads,
        backfill_complete,
        skipped_spam_threads: state.skipped_spam_threads,
        skipped_irrelevant_threads: state.skipped_irrelevant_threads,
        purged_threads,
        ai_analyzed_threads: analysis_report.analyzed,
        auto_linked_threads,
        analysis_refreshed_threads: analysis_report.refreshed,
        analysis_failed_threads: analysis_report.failed,
        orphan_threads,
        new_messages: state.new_messages,
        new_threads: state.new_thread_ids.len() as i64,
        synced_labels,
        synced_drafts,
        started_at: started_at.to_string(),
        completed_at,
        account_email,
    })
}

pub async fn sync_due(pool: &SqlitePool) -> Result<bool, String> {
    let settings = get_sync_settings(pool).await?;
    if !settings.sync_enabled {
        return Ok(false);
    }
    let Some(last) = settings.last_sync_completed_at.as_deref() else {
        return Ok(true);
    };
    let last = chrono::DateTime::parse_from_rfc3339(last)
        .map_err(|e| format!("invalid last sync timestamp: {e}"))?
        .with_timezone(&Utc);
    Ok(Utc::now().signed_duration_since(last) >= Duration::hours(settings.sync_interval_hours))
}

fn sync_queries(include_sent: bool, age_filter: Option<&str>) -> Vec<String> {
    let mut queries = vec![gmail_query("in:inbox", age_filter)];
    if include_sent {
        queries.push(gmail_query("in:sent", age_filter));
    }
    queries
}

fn gmail_query(mailbox: &str, age_filter: Option<&str>) -> String {
    let mut parts = vec![mailbox.to_string()];
    if let Some(age_filter) = age_filter {
        parts.push(age_filter.to_string());
    }
    parts.push("-in:spam".to_string());
    parts.push("-in:trash".to_string());
    parts.push("-in:chats".to_string());
    parts.join(" ")
}

async fn sync_thread_refs(
    client: &reqwest::Client,
    token: &str,
    pool: &SqlitePool,
    thread_refs: Vec<ApiThreadRef>,
    started_at: &str,
    state: &mut ThreadSyncState,
) -> Result<(), String> {
    for thread_ref in thread_refs {
        if state.touched_threads.contains(&thread_ref.id) {
            continue;
        }

        let detail = fetch_api_thread(client, token, &thread_ref.id).await?;
        let thread_id = detail.id.clone();
        let detail_history_id = detail.history_id.clone();
        let mut parsed_messages = Vec::new();
        for api_message in detail.messages.unwrap_or_default() {
            let parsed = parse_api_message(api_message, Some(&thread_id))?;
            if is_blocked_spam_or_trash(&parsed) {
                continue;
            }
            parsed_messages.push(parsed);
        }

        if parsed_messages.is_empty() {
            state.skipped_spam_threads += 1;
            delete_local_thread(pool, &thread_id).await?;
            continue;
        }

        ensure_thread_placeholder(pool, &thread_id, started_at).await?;

        let mut seen_artifact_urls = BTreeSet::new();
        for parsed in parsed_messages {
            for url in &parsed.artifact_urls {
                seen_artifact_urls.insert(url.clone());
            }
            let was_new = upsert_message(pool, &parsed, started_at).await?;
            if was_new {
                state.new_messages += 1;
                if parsed.label_ids.iter().any(|label| label == "INBOX")
                    && !parsed.is_sent
                    && !parsed.is_draft
                {
                    state.new_thread_ids.insert(parsed.thread_id.clone());
                }
            }
            state.synced_messages += 1;
            state.touched_threads.insert(parsed.thread_id.clone());

            if let Some(history_id) = parsed.history_id.as_deref() {
                state.max_history_id = max_history(state.max_history_id.take(), history_id);
            }
        }
        if let Some(history_id) = detail_history_id.as_deref() {
            state.max_history_id = max_history(state.max_history_id.take(), history_id);
        }
        delete_stale_thread_links(pool, &thread_id, &seen_artifact_urls).await?;
        state.synced_threads += 1;
    }
    Ok(())
}



#[derive(Debug, Clone, Default)]
pub struct AutoAnalyzeReport {
    pub analyzed: i64,
    pub refreshed: i64,
    pub failed: i64,
}
pub async fn auto_analyze_relevant_threads(
    dir: &Path,
    pool: &SqlitePool,
    limit: i64,
    synced_at: Option<&str>,
) -> AutoAnalyzeReport {
    if limit <= 0 {
        return AutoAnalyzeReport::default();
    }
    let candidates: Vec<(String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT thread_id, ai_generated_at
        FROM gmail_threads
        WHERE (? IS NULL OR last_sync_at = ?)
          AND (
            summary IS NULL
            OR summary = ''
            OR ai_generated_at IS NULL
            OR ai_title IS NULL
            OR last_analyzed_message_at IS NULL
            OR COALESCE(last_message_at, 0) > COALESCE(last_analyzed_message_at, 0)
            OR COALESCE(message_count, 0) != COALESCE(last_analyzed_message_count, 0)
          )
        ORDER BY
          CASE effective_priority
            WHEN 'urgent' THEN 0
            WHEN 'high' THEN 1
            WHEN 'medium' THEN 2
            ELSE 3
          END,
          COALESCE(last_message_at, first_message_at, 0) DESC
        LIMIT ?
        "#,
    )
    .bind(synced_at)
    .bind(synced_at)
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if candidates.is_empty() {
        return AutoAnalyzeReport::default();
    }

    let Ok(Some(api_key)) = crate::keychain::get_gemini_api_key(dir) else {
        let reason = "Gemini API key not configured. Add it in Settings.";
        for (thread_id, _) in &candidates {
            let _ = sqlx::query("UPDATE gmail_threads SET last_analysis_error = ? WHERE thread_id = ?")
                .bind(reason)
                .bind(thread_id)
                .execute(pool)
                .await;
        }
        return AutoAnalyzeReport {
            failed: candidates.len() as i64,
            ..AutoAnalyzeReport::default()
        };
    };

    let mut report = AutoAnalyzeReport::default();
    for (thread_id, previous_analysis) in candidates {
        match analyze_thread_with_gemini(&api_key, pool, &thread_id, false).await {
            Ok(_) => {
                report.analyzed += 1;
                if previous_analysis.is_some() {
                    report.refreshed += 1;
                }
            }
            Err(error) => {
                report.failed += 1;
                let _ = sqlx::query(
                    "UPDATE gmail_threads SET last_analysis_error = ? WHERE thread_id = ?",
                )
                .bind(error)
                .bind(&thread_id)
                .execute(pool)
                .await;
            }
        }
    }
    report
}

pub async fn batch_analyze_unsummarized_threads(
    dir: &std::path::Path,
    pool: &SqlitePool,
    limit: i64,
) -> Result<i64, String> {
    let api_key = crate::keychain::get_gemini_api_key(dir)?
        .ok_or_else(|| "Gemini API key not configured. Add it in Settings.".to_string())?;

    let thread_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT thread_id
        FROM gmail_threads
        WHERE summary IS NULL
           OR summary = ''
           OR ai_generated_at IS NULL
           OR ai_title IS NULL
           OR last_analyzed_message_at IS NULL
           OR COALESCE(last_message_at, 0) > COALESCE(last_analyzed_message_at, 0)
           OR COALESCE(message_count, 0) != COALESCE(last_analyzed_message_count, 0)
        ORDER BY COALESCE(last_message_at, first_message_at, 0) DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("DB error: {e}"))?;

    let mut analyzed = 0i64;
    for thread_id in &thread_ids {
        if analyze_thread_with_gemini(&api_key, pool, thread_id, false)
            .await
            .is_ok()
        {
            analyzed += 1;
        }
    }
    Ok(analyzed)
}
