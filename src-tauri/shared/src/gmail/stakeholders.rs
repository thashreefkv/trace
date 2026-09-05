//! Stakeholder lens: threads for a stakeholder, exclude-from-stakeholder,
//! relationship health, suggestions, and the relationship graph.
//! Extracted from legacy.rs (Section 13-G16).

use chrono::Utc;
use sqlx::SqlitePool;

use super::models::*;
use super::{get_sync_settings, list_local_threads, stakeholder_email};

pub async fn stakeholder_threads(
    pool: &SqlitePool,
    stakeholder_id: &str,
    limit: i64,
) -> Result<Vec<GmailLocalThread>, String> {
    list_local_threads(
        pool,
        GmailThreadFilter {
            stakeholder_id: Some(stakeholder_id.to_string()),
            limit: Some(limit),
            ..GmailThreadFilter::default()
        },
    )
    .await
}

pub async fn exclude_thread_from_stakeholder(
    pool: &SqlitePool,
    thread_id: &str,
    stakeholder_id: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR REPLACE INTO gmail_thread_stakeholder_excludes (thread_id, stakeholder_id, excluded_at) VALUES (?, ?, ?)",
    )
    .bind(thread_id)
    .bind(stakeholder_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

pub async fn stakeholder_health(
    pool: &SqlitePool,
    stakeholder_id: &str,
) -> Result<GmailStakeholderHealth, String> {
    let email = stakeholder_email(pool, stakeholder_id)
        .await?
        .ok_or_else(|| "stakeholder has no email address".to_string())?;
    let stats: Option<(String, i64, i64, i64)> = sqlx::query_as(
        r#"
        SELECT last_seen_at, sent_count, received_count, thread_count
        FROM gmail_participants
        WHERE lower(email) = lower(?)
        "#,
    )
    .bind(&email)
    .fetch_optional(pool)
    .await
    .map_err(crate::db::sql_error)?;

    let (last_seen_at, sent_count, received_count, thread_count) =
        stats.unwrap_or_else(|| (String::new(), 0, 0, 0));
    let days_since_last_email = if last_seen_at.is_empty() {
        None
    } else {
        chrono::DateTime::parse_from_rfc3339(&last_seen_at)
            .ok()
            .map(|dt| {
                Utc::now()
                    .signed_duration_since(dt.with_timezone(&Utc))
                    .num_days()
            })
    };
    let response_rate = if sent_count == 0 {
        if received_count > 0 {
            1.0
        } else {
            0.0
        }
    } else {
        (received_count as f64 / sent_count as f64).min(1.0)
    };
    let recency_score = match days_since_last_email {
        None => 0,
        Some(days) if days <= 7 => 45,
        Some(days) if days <= 21 => 30,
        Some(days) if days <= 45 => 18,
        Some(_) => 8,
    };
    let volume_score = thread_count.min(20);
    let health_score =
        (recency_score + (response_rate * 35.0).round() as i64 + volume_score).clamp(0, 100);

    Ok(GmailStakeholderHealth {
        stakeholder_id: stakeholder_id.to_string(),
        email,
        days_since_last_email,
        sent_count,
        received_count,
        thread_count,
        response_rate,
        health_score,
    })
}

pub async fn stakeholder_suggestions(
    pool: &SqlitePool,
    min_threads: i64,
) -> Result<Vec<GmailStakeholderSuggestion>, String> {
    let account_email = get_sync_settings(pool)
        .await?
        .account_email
        .unwrap_or_default();
    let rows: Vec<(String, String, i64, i64, i64, String)> = sqlx::query_as(
        r#"
        SELECT p.email, p.name, p.thread_count, p.sent_count, p.received_count, p.last_seen_at
        FROM gmail_participants p
        LEFT JOIN stakeholders s ON lower(s.email) = lower(p.email)
        WHERE p.email != ''
          AND lower(p.email) != lower(?)
          AND s.id IS NULL
          AND p.thread_count >= ?
        ORDER BY p.thread_count DESC, p.last_seen_at DESC
        LIMIT 25
        "#,
    )
    .bind(account_email)
    .bind(min_threads.max(1))
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(email, name, thread_count, sent_count, received_count, last_seen_at)| {
                GmailStakeholderSuggestion {
                    email,
                    name,
                    thread_count,
                    sent_count,
                    received_count,
                    last_seen_at,
                }
            },
        )
        .collect())
}

pub async fn relationship_graph(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<GmailRelationshipEdge>, String> {
    let account_email = get_sync_settings(pool)
        .await?
        .account_email
        .unwrap_or_default();
    let rows: Vec<(String, String, String, String, i64, String)> = sqlx::query_as(
        r#"
        SELECT
          lower(a.email) AS left_email,
          COALESCE(NULLIF(a.name, ''), lower(a.email)) AS left_name,
          lower(b.email) AS right_email,
          COALESCE(NULLIF(b.name, ''), lower(b.email)) AS right_name,
          COUNT(DISTINCT a.thread_id) AS thread_count,
          MAX(CASE WHEN a.last_seen_at > b.last_seen_at THEN a.last_seen_at ELSE b.last_seen_at END) AS last_seen_at
        FROM gmail_thread_participants a
        INNER JOIN gmail_thread_participants b
          ON b.thread_id = a.thread_id
         AND lower(b.email) > lower(a.email)
        WHERE lower(a.email) != lower(?)
          AND lower(b.email) != lower(?)
          AND a.email != ''
          AND b.email != ''
        GROUP BY lower(a.email), lower(b.email)
        ORDER BY thread_count DESC, last_seen_at DESC
        LIMIT ?
        "#,
    )
    .bind(&account_email)
    .bind(&account_email)
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows
        .into_iter()
        .map(
            |(left_email, left_name, right_email, right_name, thread_count, last_seen_at)| {
                GmailRelationshipEdge {
                    left_email,
                    left_name,
                    right_email,
                    right_name,
                    thread_count,
                    last_seen_at,
                }
            },
        )
        .collect())
}

