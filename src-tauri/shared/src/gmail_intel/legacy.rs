//! User-correctable email classification layer.
//!
//! Sits in front of the existing Gemini classifier. Resolves the *effective*
//! category/priority for a thread by checking:
//!   1. Per-thread user override (`gmail_user_classifications`) — explicit.
//!   2. Sender-pattern rule (`gmail_sender_rules`) — deterministic.
//!   3. LLM-set `ai_category` / `ai_priority` — fallback.
//!
//! All user overrides emit a `brain_rl_events` row so the RL system learns
//! from corrections.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::overrides::get_override;
use super::sender_rules::first_matching_rule;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UserClassification {
    pub thread_id: String,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub intent: Option<String>,
    pub action_required: Option<bool>,
    pub thread_state: Option<String>,
    pub work_relevance: Option<String>,
    pub attention_state: Option<String>,
    pub message_type: Option<String>,
    pub note: Option<String>,
    pub source: String,
    pub rule_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SenderRule {
    pub id: String,
    pub pattern: String,
    pub pattern_kind: String,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub work_relevance: Option<String>,
    pub attention_state: Option<String>,
    pub message_type: Option<String>,
    pub note: Option<String>,
    pub enabled: bool,
    pub applied_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SetOverrideInput {
    pub thread_id: String,
    pub category: Option<String>,
    pub priority: Option<String>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub action_required: Option<bool>,
    #[serde(default)]
    pub thread_state: Option<String>,
    #[serde(default)]
    pub work_relevance: Option<String>,
    #[serde(default)]
    pub attention_state: Option<String>,
    #[serde(default)]
    pub message_type: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateSenderRuleInput {
    pub pattern: String,
    pub pattern_kind: Option<String>, // 'exact' | 'glob' | 'domain' (default 'glob')
    pub category: Option<String>,
    pub priority: Option<String>,
    #[serde(default)]
    pub work_relevance: Option<String>,
    #[serde(default)]
    pub attention_state: Option<String>,
    #[serde(default)]
    pub message_type: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct EffectiveClassification {
    pub category: String,
    pub priority: String,
    pub intent: Option<String>,
    pub action_required: bool,
    pub thread_state: Option<String>,
    pub work_relevance: String,
    pub attention_state: String,
    pub message_type: String,
    pub recency_adjusted_priority: String,
    pub recency_decay_note: Option<String>,
    pub source: ClassificationSource,
    pub override_note: Option<String>,
    pub rule_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationSource {
    Override,
    Rule,
    Llm,
}

/// Resolve effective classification for one thread across every dimension.
/// Override > sender rule > LLM. Also computes a recency-decayed priority
/// so an "urgent" thread that's been quiet for a week cools off naturally.
pub async fn effective_for_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<EffectiveClassification, String> {
    let row: Option<(
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
        Option<String>,
        String,
        String,
        String,
        Option<i64>,
    )> = sqlx::query_as(
        "SELECT COALESCE(ai_category, 'other'),
                COALESCE(ai_priority, 'low'),
                last_from_email,
                intent,
                COALESCE(action_required, 0),
                thread_state,
                COALESCE(work_relevance, 'unknown'),
                COALESCE(attention_state, 'fyi'),
                COALESCE(message_type, 'other'),
                last_message_at
           FROM gmail_threads
          WHERE thread_id = ?",
    )
    .bind(thread_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("read thread: {e}"))?;

    let (
        llm_category,
        llm_priority,
        last_from_email,
        llm_intent,
        llm_action,
        llm_state,
        deterministic_relevance,
        deterministic_attention,
        deterministic_message_type,
        last_message_at,
    ) = row.unwrap_or((
        "other".to_string(),
        "low".to_string(),
        None,
        None,
        0,
        None,
        "unknown".to_string(),
        "fyi".to_string(),
        "other".to_string(),
        None,
    ));

    let (
        category,
        priority,
        intent,
        action_required,
        thread_state,
        work_relevance,
        attention_state,
        message_type,
        source,
        note,
        rule_id,
    ) = if let Some(over) = get_override(pool, thread_id).await? {
        (
            over.category.clone().unwrap_or(llm_category.clone()),
            over.priority.clone().unwrap_or(llm_priority.clone()),
            over.intent.clone().or(llm_intent.clone()),
            over.action_required.unwrap_or(llm_action != 0),
            over.thread_state.clone().or(llm_state.clone()),
            over.work_relevance
                .clone()
                .unwrap_or(deterministic_relevance.clone()),
            over.attention_state
                .clone()
                .unwrap_or(deterministic_attention.clone()),
            over.message_type
                .clone()
                .unwrap_or(deterministic_message_type.clone()),
            ClassificationSource::Override,
            over.note,
            over.rule_id,
        )
    } else if let Some(sender) = last_from_email.as_deref() {
        if let Some(rule) = first_matching_rule(pool, sender).await? {
            (
                rule.category.unwrap_or(llm_category.clone()),
                rule.priority.unwrap_or(llm_priority.clone()),
                llm_intent.clone(),
                llm_action != 0,
                llm_state.clone(),
                rule.work_relevance
                    .unwrap_or(deterministic_relevance.clone()),
                rule.attention_state
                    .unwrap_or(deterministic_attention.clone()),
                rule.message_type
                    .unwrap_or(deterministic_message_type.clone()),
                ClassificationSource::Rule,
                rule.note,
                Some(rule.id),
            )
        } else {
            (
                llm_category.clone(),
                llm_priority.clone(),
                llm_intent.clone(),
                llm_action != 0,
                llm_state.clone(),
                deterministic_relevance.clone(),
                deterministic_attention.clone(),
                deterministic_message_type.clone(),
                ClassificationSource::Llm,
                None,
                None,
            )
        }
    } else {
        (
            llm_category.clone(),
            llm_priority.clone(),
            llm_intent.clone(),
            llm_action != 0,
            llm_state.clone(),
            deterministic_relevance.clone(),
            deterministic_attention.clone(),
            deterministic_message_type.clone(),
            ClassificationSource::Llm,
            None,
            None,
        )
    };

    let (recency_adjusted_priority, recency_decay_note) =
        decay_priority(&priority, last_message_at);

    Ok(EffectiveClassification {
        category,
        priority,
        intent,
        action_required,
        thread_state,
        work_relevance,
        attention_state,
        message_type,
        recency_adjusted_priority,
        recency_decay_note,
        source,
        override_note: note,
        rule_id,
    })
}

/// Step-down a priority based on age. Urgent → high after 3 days, high → medium
/// after 7, medium → low after 14. User overrides bypass this (they signal
/// continued importance).
fn decay_priority(priority: &str, last_message_at: Option<i64>) -> (String, Option<String>) {
    let Some(ts_secs) = last_message_at else {
        return (priority.to_string(), None);
    };
    let now_secs = chrono::Utc::now().timestamp();
    let age_days = ((now_secs - ts_secs).max(0)) / 86_400;
    let decayed = match priority {
        "urgent" if age_days >= 3 => Some(("high", age_days)),
        "high" if age_days >= 7 => Some(("medium", age_days)),
        "medium" if age_days >= 14 => Some(("low", age_days)),
        _ => None,
    };
    if let Some((to, days)) = decayed {
        (
            to.to_string(),
            Some(format!(
                "was {} · cooled to {} after {} day{}",
                priority,
                to,
                days,
                if days == 1 { "" } else { "s" }
            )),
        )
    } else {
        (priority.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::super::sender_rules::{glob_match, match_pattern};
    use super::*;

    #[test]
    fn glob_matches_wildcards() {
        assert!(glob_match("*@example.com", "alice@example.com"));
        assert!(glob_match("newsletter@*", "newsletter@medium.com"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("*@example.com", "alice@other.com"));
    }

    #[test]
    fn domain_match_normalizes() {
        assert!(match_pattern("domain", "example.com", "alice@example.com"));
        assert!(match_pattern("domain", "@example.com", "alice@example.com"));
        assert!(!match_pattern("domain", "example.com", "alice@other.com"));
    }

    #[test]
    fn exact_match_is_strict() {
        assert!(match_pattern(
            "exact",
            "alice@example.com",
            "alice@example.com"
        ));
        assert!(!match_pattern(
            "exact",
            "alice@example.com",
            "alice@other.com"
        ));
    }
}
