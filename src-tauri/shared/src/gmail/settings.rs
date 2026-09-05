//! Gmail sync settings: read and update the gmail_sync_settings row.
//! Extracted from legacy.rs (Section 13-G18).

use sqlx::SqlitePool;

use super::models::*;

pub async fn get_sync_settings(pool: &SqlitePool) -> Result<GmailSyncSettings, String> {
    sqlx::query_as::<_, GmailSyncSettings>(
        r#"
        SELECT sync_enabled,
               sync_interval_hours,
               notification_poll_minutes,
               max_threads_per_sync,
               include_sent,
               include_drafts,
               notify_new_mail,
               backfill_enabled,
               relevance_filter_enabled,
               auto_analyze_enabled,
               auto_analyze_limit,
               backfill_page_token,
               backfill_query,
               last_backfill_at,
               backfill_completed_at,
               account_email,
               last_sync_started_at,
               last_sync_completed_at,
               last_history_id,
               last_error
        FROM gmail_sync_settings
        WHERE id = 1
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(crate::db::sql_error)
}

pub async fn update_sync_settings(
    pool: &SqlitePool,
    input: GmailSyncSettingsInput,
) -> Result<GmailSyncSettings, String> {
    let current = get_sync_settings(pool).await?;
    let sync_enabled = input.sync_enabled.unwrap_or(current.sync_enabled);
    let sync_interval_hours = input
        .sync_interval_hours
        .unwrap_or(current.sync_interval_hours)
        .clamp(1, 168);
    let notification_poll_minutes = input
        .notification_poll_minutes
        .unwrap_or(current.notification_poll_minutes)
        .clamp(1, 240);
    let max_threads_per_sync = input
        .max_threads_per_sync
        .unwrap_or(current.max_threads_per_sync)
        .clamp(10, 500);
    let include_sent = input.include_sent.unwrap_or(current.include_sent);
    let include_drafts = input.include_drafts.unwrap_or(current.include_drafts);
    let notify_new_mail = input.notify_new_mail.unwrap_or(current.notify_new_mail);
    let backfill_enabled = input.backfill_enabled.unwrap_or(current.backfill_enabled);
    let relevance_filter_enabled = input
        .relevance_filter_enabled
        .unwrap_or(current.relevance_filter_enabled);
    let auto_analyze_enabled = input
        .auto_analyze_enabled
        .unwrap_or(current.auto_analyze_enabled);
    let auto_analyze_limit = input
        .auto_analyze_limit
        .unwrap_or(current.auto_analyze_limit)
        .clamp(0, 25);

    sqlx::query(
        r#"
        UPDATE gmail_sync_settings
        SET sync_enabled = ?,
            sync_interval_hours = ?,
            notification_poll_minutes = ?,
            max_threads_per_sync = ?,
            include_sent = ?,
            include_drafts = ?,
            notify_new_mail = ?,
            backfill_enabled = ?,
            relevance_filter_enabled = ?,
            auto_analyze_enabled = ?,
            auto_analyze_limit = ?
        WHERE id = 1
        "#,
    )
    .bind(sync_enabled)
    .bind(sync_interval_hours)
    .bind(notification_poll_minutes)
    .bind(max_threads_per_sync)
    .bind(include_sent)
    .bind(include_drafts)
    .bind(notify_new_mail)
    .bind(backfill_enabled)
    .bind(relevance_filter_enabled)
    .bind(auto_analyze_enabled)
    .bind(auto_analyze_limit)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    get_sync_settings(pool).await
}
