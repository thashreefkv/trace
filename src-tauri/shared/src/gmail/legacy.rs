use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use ulid::Ulid;

use super::linking::count_orphan_threads;
use super::{
    analyze_thread_with_gemini, auto_link_thread, classify_url, delete_local_thread,
    delete_stale_thread_links, email_domain, ensure_thread_placeholder, extract_urls,
    fetch_api_thread, fetch_profile, format_ts, get_local_thread, get_valid_access_token,
    gmail_connected, header_value, infer_thread_category, is_artifact_url,
    is_blocked_spam_or_trash, list_api_threads, list_api_threads_page, list_local_threads,
    load_message_rows, local_message_is_low_signal, max_history, message_preview, now_utc,
    parse_address_header, parse_addresses_json, parse_api_message, parse_string_vec,
    purge_blocked_threads, rebuild_thread_aggregate, refresh_thread_intelligence,
    refresh_work_mail_review_state, strip_html, sync_drafts, sync_labels, to_json_string,
    upsert_draft, upsert_message, work_mail_needs_me_reason, work_mail_review_summary,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GmailThread {
    pub thread_id: String,
    pub subject: String,
    pub snippet: String,
    pub sender: String,
    pub sender_email: String,
    pub date_ts: i64,
    pub message_count: u32,
}

pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::{
        linking::{compute_effective_priority, score_deliverable_link},
        list_work_mail_threads, mark_work_mail_thread_seen,
        send::ensure_system_label,
        set_work_mail_review_state,
        work_mail::infer_work_mail_dimensions,
        GmailMessageRecord, WorkMailQuery, WorkMailReviewState, WorkMailReviewUpdate,
        WorkMailViewId,
    };
    use super::*;

    async fn insert_review_test_thread(pool: &SqlitePool) {
        ensure_system_label(pool, "INBOX", "INBOX").await.unwrap();
        ensure_system_label(pool, "UNREAD", "UNREAD").await.unwrap();
        sqlx::query(
            "INSERT INTO gmail_work_domains
               (domain, enabled, source, created_at, updated_at)
             VALUES ('example.com', 1, 'test', 0, 0)",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO gmail_threads (thread_id, last_sync_at) VALUES ('t-review', ?)")
            .bind(now_utc())
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            r#"
            INSERT INTO gmail_messages
              (message_id, thread_id, subject, snippet, from_name, from_email,
               internal_date_ts, date_ts, label_ids_json, is_unread, synced_at)
            VALUES
              ('m-review-1', 't-review', 'Review me', 'Inbound context',
               'Alex', 'alex@example.com', 100, 100, '["INBOX","UNREAD"]', 1, ?)
            "#,
        )
        .bind(now_utc())
        .execute(pool)
        .await
        .unwrap();
        rebuild_thread_aggregate(pool, "t-review", &now_utc())
            .await
            .unwrap();
    }

    fn message(subject: &str, from_email: &str, labels: Vec<&str>) -> GmailMessageRecord {
        GmailMessageRecord {
            message_id: "m1".to_string(),
            thread_id: "t1".to_string(),
            history_id: None,
            subject: subject.to_string(),
            snippet: String::new(),
            from_name: "Sender".to_string(),
            from_email: from_email.to_string(),
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
            date_ts: Some(1),
            internal_date_ts: Some(1),
            plain_body: String::new(),
            html_body: String::new(),
            label_ids: labels.into_iter().map(str::to_string).collect(),
            is_sent: false,
            is_draft: false,
            is_unread: false,
            size_estimate: None,
            artifact_urls: Vec::new(),
            synced_at: now_utc(),
        }
    }

    #[test]
    fn category_inference_no_longer_forces_work_domains_to_personal() {
        let work_domain = infer_thread_category(
            &[message(
                "Can you review the launch brief?",
                "alex@example.com",
                vec!["INBOX"],
            )],
            None,
        );
        assert_ne!(work_domain.category, "personal");

        let newsletter = infer_thread_category(
            &[message(
                "Weekly newsletter digest",
                "newsletter@example.com",
                vec!["INBOX", "CATEGORY_PROMOTIONS"],
            )],
            None,
        );
        assert_eq!(newsletter.category, "newsletter");
    }

    #[test]
    fn work_mail_dimensions_scope_internal_noise_and_linked_external_mail() {
        let domains = BTreeSet::from(["example.com".to_string(), "example.org".to_string()]);
        let internal = infer_work_mail_dimensions(
            &[message(
                "Can you review the launch brief?",
                "alex@example.com",
                vec!["INBOX"],
            )],
            &domains,
            false,
            false,
            "other",
            false,
            None,
        );
        assert_eq!(internal.work_relevance, "work");
        assert_eq!(internal.message_type, "conversation");

        let promo = infer_work_mail_dimensions(
            &[message(
                "Quarterly conference sale",
                "events@example.com",
                vec!["INBOX", "CATEGORY_PROMOTIONS"],
            )],
            &domains,
            false,
            false,
            "newsletter",
            false,
            None,
        );
        assert_eq!(promo.work_relevance, "excluded");
        assert_eq!(promo.message_type, "promotion");

        let linked_external = infer_work_mail_dimensions(
            &[message(
                "Design review notes",
                "partner@outside.test",
                vec!["INBOX"],
            )],
            &domains,
            true,
            false,
            "other",
            false,
            None,
        );
        assert_eq!(linked_external.work_relevance, "linked_external");
    }

    #[test]
    fn graph_context_boosts_stakeholder_deadline_priority() {
        let deadline = (Utc::now() + Duration::days(1))
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let context = json!({
            "stakeholders": [{
                "id": "s1",
                "name": "Alex",
                "email": "alex@example.com",
                "role": "Sponsor"
            }],
            "active_deliverables": [{
                "id": "d1",
                "title": "Board Review Deck",
                "state": "in_review",
                "deadline": deadline,
                "priority": "high",
                "blocker_reason": "",
                "artifact_url": null
            }]
        });

        let (priority, reasons, graph_signal) =
            compute_effective_priority(&context, "newsletter", "low", false);

        assert!(matches!(priority.as_str(), "high" | "urgent"));
        assert!(graph_signal);
        assert!(reasons.iter().any(|reason| reason.contains("48 hours")));
    }

    #[test]
    fn newsletter_without_active_graph_context_stays_low() {
        let (priority, _reasons, graph_signal) =
            compute_effective_priority(&json!({}), "newsletter", "low", false);

        assert_eq!(priority, "low");
        assert!(!graph_signal);
    }

    #[test]
    fn deliverable_title_and_stakeholder_match_auto_link_threshold() {
        let (score, reasons) = score_deliverable_link(
            "please review the board review deck before tomorrow",
            &[],
            &["alex@example.com".to_string()],
            "Board Review Deck",
            "",
            None,
            "alex@example.com",
            None,
            None,
            None,
        );

        assert!(score >= 0.82);
        assert!(reasons
            .iter()
            .any(|reason| reason.contains("Thread participant")));
    }

    #[tokio::test]
    async fn work_mail_seen_checkpoint_does_not_clear_gmail_unread() {
        let pool = crate::db::connect_memory().await.unwrap();
        insert_review_test_thread(&pool).await;

        let summary = mark_work_mail_thread_seen(&pool, "t-review").await.unwrap();
        let unread: bool = sqlx::query_scalar(
            "SELECT is_unread FROM gmail_messages WHERE message_id = 'm-review-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(summary.review_state, WorkMailReviewState::Unreviewed);
        assert_eq!(summary.seen.message_id.as_deref(), Some("m-review-1"));
        assert!(unread);
    }

    #[tokio::test]
    async fn reviewed_work_mail_leaves_needs_me_until_reopened() {
        let pool = crate::db::connect_memory().await.unwrap();
        insert_review_test_thread(&pool).await;

        let before = list_work_mail_threads(
            &pool,
            WorkMailQuery {
                view: WorkMailViewId::NeedsMe,
                ..WorkMailQuery::default()
            },
        )
        .await
        .unwrap();
        set_work_mail_review_state(
            &pool,
            "t-review",
            WorkMailReviewUpdate {
                state: WorkMailReviewState::Reviewed,
                deferred_until: None,
            },
        )
        .await
        .unwrap();
        let after = list_work_mail_threads(
            &pool,
            WorkMailQuery {
                view: WorkMailViewId::NeedsMe,
                ..WorkMailQuery::default()
            },
        )
        .await
        .unwrap();

        assert!(before.iter().any(|thread| thread.thread_id == "t-review"));
        assert!(!after.iter().any(|thread| thread.thread_id == "t-review"));
    }

    #[tokio::test]
    async fn new_inbound_mail_reopens_handled_work_mail() {
        let pool = crate::db::connect_memory().await.unwrap();
        insert_review_test_thread(&pool).await;
        set_work_mail_review_state(
            &pool,
            "t-review",
            WorkMailReviewUpdate {
                state: WorkMailReviewState::Resolved,
                deferred_until: None,
            },
        )
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO gmail_messages
              (message_id, thread_id, subject, snippet, from_name, from_email,
               internal_date_ts, date_ts, label_ids_json, is_unread, synced_at)
            VALUES
              ('m-review-2', 't-review', 'Review me', 'New inbound',
               'Alex', 'alex@example.com', 200, 200, '["INBOX","UNREAD"]', 1, ?)
            "#,
        )
        .bind(now_utc())
        .execute(&pool)
        .await
        .unwrap();
        rebuild_thread_aggregate(&pool, "t-review", &now_utc())
            .await
            .unwrap();

        let summary = work_mail_review_summary(&pool, "t-review").await.unwrap();

        assert_eq!(summary.review_state, WorkMailReviewState::Unreviewed);
        assert!(summary.new_since_review);
    }
}

pub async fn stakeholder_email(
    pool: &SqlitePool,
    stakeholder_id: &str,
) -> Result<Option<String>, String> {
    let email: Option<String> =
        sqlx::query_scalar("SELECT NULLIF(email, '') FROM stakeholders WHERE id = ?")
            .bind(stakeholder_id)
            .fetch_optional(pool)
            .await
            .map_err(crate::db::sql_error)?
            .flatten();
    Ok(email)
}
