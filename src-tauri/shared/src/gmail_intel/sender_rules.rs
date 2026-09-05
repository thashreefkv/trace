//! Sender rules: CRUD + pattern matching. Extracted from legacy.rs (13-std2).

use sqlx::SqlitePool;

use super::legacy::*;

pub async fn list_sender_rules(pool: &SqlitePool) -> Result<Vec<SenderRule>, String> {
    sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
            i64,
            i64,
            i64,
        ),
    >(
        "SELECT id, pattern, pattern_kind, category, priority,
                work_relevance, attention_state, message_type, note,
                enabled, applied_count, created_at, updated_at
           FROM gmail_sender_rules ORDER BY enabled DESC, updated_at DESC",
    )
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(
                |(
                    id,
                    pattern,
                    pattern_kind,
                    category,
                    priority,
                    work_relevance,
                    attention_state,
                    message_type,
                    note,
                    enabled,
                    applied_count,
                    created_at,
                    updated_at,
                )| SenderRule {
                    id,
                    pattern,
                    pattern_kind,
                    category,
                    priority,
                    work_relevance,
                    attention_state,
                    message_type,
                    note,
                    enabled: enabled != 0,
                    applied_count,
                    created_at,
                    updated_at,
                },
            )
            .collect()
    })
    .map_err(|e| format!("list sender rules: {e}"))
}

pub async fn create_sender_rule(
    pool: &SqlitePool,
    input: CreateSenderRuleInput,
) -> Result<SenderRule, String> {
    if input.pattern.trim().is_empty() {
        return Err("pattern is required".to_string());
    }
    if input.category.is_none()
        && input.priority.is_none()
        && input.work_relevance.is_none()
        && input.attention_state.is_none()
        && input.message_type.is_none()
    {
        return Err("rule must set a classification dimension".to_string());
    }
    let id = ulid::Ulid::new().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    let kind = input.pattern_kind.as_deref().unwrap_or("glob");
    if !matches!(kind, "exact" | "glob" | "domain") {
        return Err(format!("invalid pattern_kind: {kind}"));
    }
    sqlx::query(
        "INSERT INTO gmail_sender_rules
           (id, pattern, pattern_kind, category, priority, work_relevance,
            attention_state, message_type, note, enabled, applied_count, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 0, ?, ?)",
    )
    .bind(&id)
    .bind(input.pattern.trim())
    .bind(kind)
    .bind(&input.category)
    .bind(&input.priority)
    .bind(&input.work_relevance)
    .bind(&input.attention_state)
    .bind(&input.message_type)
    .bind(&input.note)
    .bind(ts)
    .bind(ts)
    .execute(pool)
    .await
    .map_err(|e| format!("create sender rule: {e}"))?;
    let _ = crate::gmail::record_work_mail_agent_event(
        pool,
        None,
        "rule",
        "user",
        "Created a sender rule for Trace understanding.",
        serde_json::json!(["User chose a sender, domain, or pattern rule."]),
        serde_json::json!({
            "pattern": input.pattern,
            "pattern_kind": kind,
            "category": input.category,
            "priority": input.priority,
            "work_relevance": input.work_relevance,
            "attention_state": input.attention_state,
            "message_type": input.message_type
        }),
        None,
    )
    .await;

    list_sender_rules(pool)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| "rule not found after insert".to_string())
}

pub async fn delete_sender_rule(pool: &SqlitePool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM gmail_sender_rules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("delete sender rule: {e}"))?;
    Ok(())
}

pub async fn toggle_sender_rule(pool: &SqlitePool, id: &str, enabled: bool) -> Result<(), String> {
    let ts = chrono::Utc::now().timestamp_millis();
    sqlx::query("UPDATE gmail_sender_rules SET enabled = ?, updated_at = ? WHERE id = ?")
        .bind(if enabled { 1_i64 } else { 0_i64 })
        .bind(ts)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("toggle sender rule: {e}"))?;
    Ok(())
}

pub(crate) async fn first_matching_rule(
    pool: &SqlitePool,
    sender_email: &str,
) -> Result<Option<SenderRule>, String> {
    let rules = list_sender_rules(pool).await?;
    let sender_lower = sender_email.to_ascii_lowercase();
    Ok(rules
        .into_iter()
        .filter(|r| r.enabled)
        .find(|rule| match_pattern(&rule.pattern_kind, &rule.pattern, &sender_lower)))
}

pub(super) fn match_pattern(kind: &str, pattern: &str, sender: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    match kind {
        "exact" => sender == pattern,
        "domain" => {
            let domain = pattern.trim_start_matches('@');
            sender
                .split('@')
                .nth(1)
                .map(|d| d == domain)
                .unwrap_or(false)
        }
        // 'glob' (default) — `*` matches zero or more chars
        _ => glob_match(&pattern, sender),
    }
}

pub(super) fn glob_match(pattern: &str, value: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut cursor = 0;
    let mut first = true;
    for (idx, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if first && !pattern.starts_with('*') {
            if !value[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
            first = false;
            continue;
        }
        if let Some(found) = value[cursor..].find(part) {
            cursor += found + part.len();
            first = false;
        } else {
            return false;
        }
    }
    if !pattern.ends_with('*') {
        if let Some(last) = parts.last().filter(|p| !p.is_empty()) {
            return value.ends_with(last);
        }
    }
    true
}

// ── Inbox dashboard ────────────────────────────────────────────────────────
