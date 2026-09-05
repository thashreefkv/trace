use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
use ulid::Ulid;

use super::{
    auto_link_thread, build_graph_context_for_thread, format_ts, get_local_thread, now_utc,
    refresh_thread_intelligence, strip_html, to_json_string,
};

use super::models::*;

pub async fn analyze_thread_with_gemini(
    api_key: &str,
    pool: &SqlitePool,
    thread_id: &str,
    include_reply: bool,
) -> Result<GmailAiResult, String> {
    let _ = auto_link_thread(pool, thread_id).await;
    let detail = get_local_thread(pool, thread_id).await?;
    let transcript = thread_transcript(&detail);
    let graph_context = build_graph_context_for_thread(pool, thread_id)
        .await
        .unwrap_or_else(|_| json!({}));
    let graph_context_text = serde_json::to_string_pretty(&graph_context)
        .unwrap_or_else(|_| "{}".to_string());
    let account_email: Option<String> =
        sqlx::query_scalar("SELECT account_email FROM gmail_sync_settings WHERE id = 1")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten();
    let reply_schema = if include_reply {
        json!({ "type": "string" })
    } else {
        json!({ "type": "string", "nullable": true })
    };
    let schema = json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "summary": { "type": "string" },
            "sentiment": { "type": "string" },
            "urgency": { "type": "string" },
            "category": {
                "type": "string",
                "enum": ["personal", "work", "action_required", "newsletter", "receipt", "meeting", "archive", "spam", "other"]
            },
            "priority": {
                "type": "string",
                "enum": ["low", "medium", "high", "urgent"]
            },
            "confidence": { "type": "number" },
            "reasons": {
                "type": "array",
                "items": { "type": "string" }
            },
            "intent": {
                "type": "string",
                "enum": ["asking", "informing", "requesting_decision", "scheduling", "acknowledging", "venting", "other"]
            },
            "action_required": { "type": "boolean" },
            "predicted_action": {
                "type": "string",
                "enum": ["reply", "schedule", "file", "ignore", "other"]
            },
            "thread_state": {
                "type": "string",
                "enum": ["waiting_on_you", "waiting_on_them", "resolved", "dormant"]
            },
            "dimensions_confidence": {
                "type": "object",
                "properties": {
                    "category": { "type": "number" },
                    "priority": { "type": "number" },
                    "intent": { "type": "number" },
                    "action_required": { "type": "number" },
                    "thread_state": { "type": "number" }
                }
            },
            "tasks": {
                "type": "array",
                "items": ai_candidate_schema()
            },
            "deliverables": {
                "type": "array",
                "items": ai_candidate_schema()
            },
            "initiatives": {
                "type": "array",
                "items": ai_candidate_schema()
            },
            "deadlines": {
                "type": "array",
                "items": ai_candidate_schema()
            },
            "reply": reply_schema
        },
        "required": ["title", "summary", "sentiment", "urgency", "category", "priority", "intent", "action_required", "predicted_action", "thread_state", "reasons", "tasks", "deliverables", "initiatives", "deadlines"]
    });
    let account_context = account_email
        .as_deref()
        .map(|e| format!("The workspace owner's email is {e}. "))
        .unwrap_or_default();
    let prompt = format!(
        "Analyze this Gmail thread for Trace, a personal project manager.\n\
         {account_context}Return JSON only.\n\
         - title: format exactly as \"[Label] Crisp Title\". Label is a 1-2 word interaction type that describes what kind of thread this is — choose the most specific word that fits (e.g. Meeting, Discussion, Request, Update, OTP, Feedback, Invite, Alert, Announcement, Review, Approval, Introduction, Follow-up, Report, Notification, Reminder, Offer, Welcome). Then 3-5 word title in title case. Strip Re:/Fwd:/FW: prefixes and any existing [TAG] markers. No email addresses, dates, or times in the title. Examples: \"Invitation: Image overlay design discussion @ Mon May 4 11:30am (name@example.com)\" → \"[Meeting] Image Overlay Design Discussion\"; \"Your OTP to log in\" → \"[OTP] Account Login Passcode\"; \"Fwd: Subtitle files request\" → \"[Request] Subtitle Files\"; \"Re: Documents for article creation\" → \"[Discussion] Article Creation Documents\"; \"A teammate mentioned you in the study room\" → \"[Alert] Team Chat Mention\"; \"Welcome to your Google Developer Profile!\" → \"[Welcome] Google Developer Profile Setup\".\n\
         - summary: 1-2 concise plain-text sentences describing what this thread is about, what was requested or decided, and any open action. No markdown, no bullets. Keep it under 180 characters so it reads as a clean email preview.\n\
         - sentiment: calm|neutral|frustrated|urgent|positive.\n\
         - urgency: low|medium|high.\n\
         - category: personal|work|action_required|newsletter|receipt|meeting|archive|spam|other. **personal** = direct interpersonal email that is not part of a configured work domain; newsletters, receipts, automated alerts, promotions, and bulk mail are NOT personal.\n\
         - priority: low|medium|high|urgent, reflecting user attention needed.\n\
         - reasons: 1-3 short reasons for the category.\n\
         - intent: what the sender is doing in this thread. asking = question expecting answer; informing = status/update FYI; requesting_decision = wants the workspace owner to choose between options; scheduling = proposing/confirming a time; acknowledging = receipt/thanks/closing; venting = emotional message with no clear ask; other = none of the above.\n\
         - action_required: true if the workspace owner needs to do something concrete (reply, decide, schedule, file). false for purely informational, automated, or already-resolved threads.\n\
         - predicted_action: the single most likely next action if action_required is true (else use 'other'). reply = compose a response; schedule = pick a time / accept invite; file = save attachment or archive; ignore = no action; other = something else.\n\
         - thread_state: who's holding the next ball. waiting_on_you = owner needs to respond/decide; waiting_on_them = owner already replied / asked something / awaiting external; resolved = nothing more to do; dormant = no activity in a long time and unlikely to need attention.\n\
         - dimensions_confidence: numeric 0-1 confidence per dimension (category, priority, intent, action_required, thread_state). Be honest — use ~0.5 when truly uncertain so Trace can flag it for review.\n\
         - tasks: action items that should be reviewed before creating tasks.\n\
         - deliverables: possible deliverables or initiatives suggested by the email.\n\
         - initiatives: broader workstreams or projects that should be reviewed before creation.\n\
         - deadlines: explicit or implied dates/deadlines.\n\
         - reply: {}.\n\n\
         Graph context already known by Trace. Use it to classify and rank work impact, but do not invent facts outside the email and graph packet:\n{}\n\n\
         Thread:\n{}",
        if include_reply {
            "a concise suggested reply"
        } else {
            "empty string"
        },
        graph_context_text,
        transcript
    );
    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema,
            // Hard caps prevent the Flash "thinking" pass from running away
            // and generating tens of thousands of tokens before committing
            // to JSON. Healthy classifications fit comfortably under 8k
            // completion tokens; observed runaways hit 64k+.
            "temperature": 0.1,
            "maxOutputTokens": 8000,
            "thinkingConfig": { "thinkingBudget": 2048 }
        }
    });
    let raw_json = crate::gemini::post_gemini_external(
        Some(pool),
        "email_classify",
        "gemini-3-flash-preview",
        api_key,
        &body,
    )
    .await?;
    let text = raw_json
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .and_then(|p| p.last())
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| "Gemini email analysis response did not include text".to_string())?
        .to_string();
    let result: GmailAiResult = serde_json::from_str(&text)
        .map_err(|e| format!("Gemini email analysis JSON failed validation: {e}"))?;

    let dims_json = result
        .dimensions_confidence
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "{}".to_string());
    let bundle_id = compute_bundle_id(&detail);

    sqlx::query(
        r#"
        UPDATE gmail_threads
        SET ai_title = ?,
            summary = ?,
            sentiment = ?,
            urgency = ?,
            ai_category = ?,
            ai_priority = ?,
            ai_category_confidence = ?,
            ai_category_reasons = ?,
            ai_generated_at = ?,
            ai_triaged_at = ?,
            last_analyzed_message_at = ?,
            last_analyzed_message_count = ?,
            last_analysis_error = NULL,
            intent = ?,
            action_required = ?,
            predicted_action = ?,
            thread_state = ?,
            dimensions_confidence_json = ?,
            bundle_id = ?
        WHERE thread_id = ?
        "#,
    )
    .bind(&result.title)
    .bind(&result.summary)
    .bind(&result.sentiment)
    .bind(&result.urgency)
    .bind(normalize_ai_category(&result.category))
    .bind(normalize_ai_priority(&result.priority))
    .bind(result.confidence)
    .bind(to_json_string(&result.reasons))
    .bind(now_utc())
    .bind(now_utc())
    .bind(detail.thread.last_message_at)
    .bind(detail.thread.message_count)
    .bind(normalize_intent(result.intent.as_deref()))
    .bind(if result.action_required.unwrap_or(false) { 1_i64 } else { 0_i64 })
    .bind(normalize_predicted_action(result.predicted_action.as_deref()))
    .bind(normalize_thread_state(result.thread_state.as_deref()))
    .bind(&dims_json)
    .bind(&bundle_id)
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    // Cross-thread context: if this thread is part of a bundle, inherit
    // link suggestions from any sibling that already has them.
    let _ = inherit_links_from_bundle(pool, thread_id, &bundle_id).await;

    store_ai_suggestions(pool, thread_id, &result).await?;
    let _ = crate::repo::record_work_intake_from_gmail_ai(pool, thread_id, &result).await;
    refresh_thread_intelligence(pool, thread_id).await?;
    let _ = record_analysis_snapshot(pool, thread_id, &detail, &result, "manual").await;
    Ok(result)
}

/// Snapshot of one analysis run for a thread. Returned by
/// `list_analysis_history` so the UI can show a timeline + diff what changed.
#[derive(Debug, Clone, Serialize)]
pub struct GmailAnalysisSnapshot {
    pub id: String,
    pub thread_id: String,
    pub analyzed_at: String,
    pub trigger: String,
    pub result: GmailAiResult,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub summary: Option<String>,
    pub message_count_at_analysis: i64,
}

async fn record_analysis_snapshot(
    pool: &SqlitePool,
    thread_id: &str,
    detail: &GmailThreadDetail,
    result: &GmailAiResult,
    trigger: &str,
) -> Result<(), String> {
    let id = Ulid::new().to_string();
    let result_json = serde_json::to_string(result)
        .map_err(|e| format!("failed to serialise analysis snapshot: {e}"))?;
    sqlx::query(
        r#"INSERT INTO gmail_thread_analysis_history
             (id, thread_id, analyzed_at, trigger, result_json,
              category, priority, summary, message_count_at_analysis)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&id)
    .bind(thread_id)
    .bind(now_utc())
    .bind(trigger)
    .bind(&result_json)
    .bind(format!("{:?}", result.category).to_lowercase())
    .bind(format!("{:?}", result.priority).to_lowercase())
    .bind(&result.summary)
    .bind(detail.thread.message_count)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(())
}

pub async fn list_analysis_history(
    pool: &SqlitePool,
    thread_id: &str,
    limit: i64,
) -> Result<Vec<GmailAnalysisSnapshot>, String> {
    let rows = sqlx::query_as::<_, (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
    )>(
        r#"SELECT id, thread_id, analyzed_at, trigger, result_json,
                  category, priority, summary, message_count_at_analysis
           FROM gmail_thread_analysis_history
           WHERE thread_id = ?
           ORDER BY analyzed_at DESC
           LIMIT ?"#,
    )
    .bind(thread_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;

    let mut snapshots = Vec::with_capacity(rows.len());
    for (id, thread_id, analyzed_at, trigger, result_json, category, priority, summary, message_count_at_analysis) in rows {
        let result: GmailAiResult = serde_json::from_str(&result_json)
            .map_err(|e| format!("failed to parse history result_json: {e}"))?;
        snapshots.push(GmailAnalysisSnapshot {
            id,
            thread_id,
            analyzed_at,
            trigger,
            result,
            category,
            priority,
            summary,
            message_count_at_analysis,
        });
    }
    Ok(snapshots)
}

/// Variant of `analyze_thread_with_gemini` that records a snapshot with the
/// given trigger label. Used by the background auto-analyze hook to mark runs
/// as `auto_new_mail` instead of `manual`.
pub async fn analyze_thread_with_gemini_tagged(
    api_key: &str,
    pool: &SqlitePool,
    thread_id: &str,
    include_reply: bool,
    trigger: &str,
) -> Result<GmailAiResult, String> {
    let result = analyze_thread_with_gemini(api_key, pool, thread_id, include_reply).await?;
    if trigger != "manual" {
        // Re-tag the most recent snapshot if the underlying analyze just wrote one.
        let _ = sqlx::query(
            r#"UPDATE gmail_thread_analysis_history
               SET trigger = ?
               WHERE id = (
                 SELECT id FROM gmail_thread_analysis_history
                 WHERE thread_id = ?
                 ORDER BY analyzed_at DESC
                 LIMIT 1
               )"#,
        )
        .bind(trigger)
        .bind(thread_id)
        .execute(pool)
        .await;
    }
    Ok(result)
}

/// Find threads where messages have arrived since the most recent analysis
/// snapshot. Returns up to `limit` thread IDs, most-stale first.
pub async fn list_threads_needing_reanalysis(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT t.thread_id
           FROM gmail_threads t
           LEFT JOIN (
             SELECT thread_id, MAX(message_count_at_analysis) AS last_mc
             FROM gmail_thread_analysis_history
             GROUP BY thread_id
           ) h ON h.thread_id = t.thread_id
           WHERE t.message_count > COALESCE(h.last_mc, 0)
             AND t.message_count > 0
           ORDER BY t.last_message_at DESC NULLS LAST
           LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

fn normalize_intent(intent: Option<&str>) -> Option<String> {
    let v = intent.unwrap_or_default().trim().to_ascii_lowercase();
    if v.is_empty() {
        return None;
    }
    match v.as_str() {
        "asking" | "informing" | "requesting_decision" | "scheduling" | "acknowledging"
        | "venting" | "other" => Some(v),
        _ => Some("other".to_string()),
    }
}

fn normalize_predicted_action(value: Option<&str>) -> Option<String> {
    let v = value.unwrap_or_default().trim().to_ascii_lowercase();
    if v.is_empty() {
        return None;
    }
    match v.as_str() {
        "reply" | "schedule" | "file" | "ignore" | "other" => Some(v),
        _ => Some("other".to_string()),
    }
}

fn normalize_thread_state(value: Option<&str>) -> Option<String> {
    let v = value.unwrap_or_default().trim().to_ascii_lowercase();
    if v.is_empty() {
        return None;
    }
    match v.as_str() {
        "waiting_on_you" | "waiting_on_them" | "resolved" | "dormant" => Some(v),
        _ => None,
    }
}

/// Derive a stable bundle id from subject + participants + day window.
/// Threads sharing a subject (Re:/Fwd: stripped) within 7 days fold together.
fn compute_bundle_id(detail: &GmailThreadDetail) -> String {
    let raw_subject: String = detail.thread.subject.clone();
    let mut subject = raw_subject.trim().to_ascii_lowercase();
    for prefix in ["re:", "fwd:", "fw:"] {
        while let Some(rest) = subject.strip_prefix(prefix) {
            subject = rest.trim_start().to_string();
        }
    }
    let mut participants: Vec<String> = detail
        .thread
        .participants
        .iter()
        .map(|a| a.email.to_ascii_lowercase())
        .collect();
    participants.sort();
    participants.dedup();
    let day_bucket = detail
        .thread
        .last_message_at
        .map(|ts| ts / (7 * 24 * 60 * 60))
        .unwrap_or(0);
    let payload = format!("{}|{}|{}", subject, participants.join(","), day_bucket);
    let mut h: u64 = 0xcbf29ce484222325;
    for b in payload.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("b_{:016x}", h)
}

async fn inherit_links_from_bundle(
    pool: &SqlitePool,
    thread_id: &str,
    bundle_id: &str,
) -> Result<(), String> {
    if bundle_id.is_empty() {
        return Ok(());
    }
    // Find sibling threads in the same bundle that have accepted link suggestions
    // and copy them over as pending suggestions on the new thread.
    let siblings: Vec<(String, String, String, f64, Option<String>)> = sqlx::query_as(
        "SELECT s.target_kind, s.target_id, s.rationale, COALESCE(s.confidence, 0.5), s.note
           FROM gmail_thread_link_suggestions s
           JOIN gmail_threads t ON t.thread_id = s.thread_id
          WHERE t.bundle_id = ? AND t.thread_id != ?
            AND s.status = 'accepted'",
    )
    .bind(bundle_id)
    .bind(thread_id)
    .fetch_all(pool)
    .await
    .map_err(crate::db::sql_error)?;

    for (target_kind, target_id, rationale, confidence, _note) in siblings {
        let id = ulid::Ulid::new().to_string();
        let _ = sqlx::query(
            "INSERT INTO gmail_thread_link_suggestions
               (id, thread_id, target_kind, target_id, confidence, rationale, status, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 'pending', ?)
             ON CONFLICT DO NOTHING",
        )
        .bind(&id)
        .bind(thread_id)
        .bind(&target_kind)
        .bind(&target_id)
        .bind(confidence.min(0.9))
        .bind(format!("Inherited from same conversation: {rationale}"))
        .bind(now_utc())
        .execute(pool)
        .await;
    }
    Ok(())
}

/// Agentic email draft using the second brain.
///
/// Pulls the workspace owner's voice and context from three sources:
/// 1. The thread transcript (what's being replied to)
/// 2. The work-graph brain (entities/projects/people Trace knows)
/// 3. Saved memory (facts, preferences, prior decisions)
///
/// Returns plain-text reply with blank-line paragraph breaks — ready to insert
/// into the rich text editor.
pub async fn draft_reply_with_brain(
    api_key: &str,
    pool: &SqlitePool,
    brain_path: &Path,
    thread_id: &str,
) -> Result<String, String> {
    let _ = auto_link_thread(pool, thread_id).await;
    let detail = get_local_thread(pool, thread_id).await?;
    let account_email: Option<String> =
        sqlx::query_scalar("SELECT account_email FROM gmail_sync_settings WHERE id = 1")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten();

    // Build a direction-labeled transcript so the model can distinguish
    // messages the owner has already sent from messages they need to reply to.
    let transcript = thread_transcript_with_direction(&detail, account_email.as_deref());
    let subject = detail.thread.subject.clone();

    // The MOST RECENT inbound message (not from the owner) is the one to reply to.
    let last_inbound = detail
        .messages
        .iter()
        .rev()
        .find(|m| !m.is_sent)
        .or_else(|| detail.messages.last());
    let last_msg_excerpt = last_inbound
        .map(|m| {
            let body = if !m.plain_body.is_empty() {
                m.plain_body.as_str()
            } else {
                m.html_body.as_str()
            };
            body.chars().take(500).collect::<String>()
        })
        .unwrap_or_default();
    let query = format!("{subject} {last_msg_excerpt}").trim().to_string();
    let query = if query.is_empty() {
        "draft email reply".to_string()
    } else {
        query
    };

    // Thread-linked graph context: stakeholders, deliverables, initiatives,
    // active deliverables for linked people, recent meetings. This is the
    // "activity / task / deliverable" context the user explicitly wants.
    let thread_graph_context = build_graph_context_for_thread(pool, thread_id)
        .await
        .unwrap_or_else(|_| json!({}));
    let thread_graph_text = serde_json::to_string_pretty(&thread_graph_context)
        .unwrap_or_else(|_| "{}".to_string());

    // Brain context: top-ranked work-graph entities by relevance.
    let brain_input = crate::models::BrainRetrieveInput {
        query: query.clone(),
        focus_entity_id: None,
        max_hops: Some(2),
        limit: Some(20),
    };
    let brain_ctx = crate::brain::retrieve_brain_context(pool, brain_path, brain_input)
        .await
        .ok();

    // Memory: saved facts + procedural pins.
    let memory_input = crate::models::RetrieveMemoryInput {
        query: query.clone(),
        limit: Some(15),
        kinds: vec![],
        source_kind: None,
        source_id: None,
        task_type: Some("email_draft".to_string()),
        include_pinned: Some(true),
    };
    let memory = crate::repo::retrieve_memories_with_key(pool, memory_input, Some(api_key))
        .await
        .ok();

    let account_context = account_email
        .as_deref()
        .map(|e| {
            format!(
                "The workspace owner's email is {e}. Messages in the transcript prefixed with [YOU] were already sent by them — do NOT repeat or re-promise anything in [YOU] messages. The reply you draft is the workspace owner's next outbound message in first person, in their usual voice. "
            )
        })
        .unwrap_or_else(|| "Draft this reply as the recipient of the [INBOUND] messages — first person. ".to_string());

    let brain_section = brain_ctx
        .as_ref()
        .map(|ctx| {
            let mut s = String::from(
                "\n\n=== Work-graph context (entities, projects, people Trace knows about) ===\n",
            );
            if !ctx.summary.trim().is_empty() {
                s.push_str(&ctx.summary);
                s.push('\n');
            }
            if !ctx.ranked_nodes.is_empty() {
                s.push_str("Top related entities:\n");
                for node in ctx.ranked_nodes.iter().take(10) {
                    let subtitle = node.subtitle.as_deref().unwrap_or("");
                    s.push_str(&format!(
                        "- [{}] {}{}\n",
                        node.kind,
                        node.label,
                        if subtitle.is_empty() {
                            String::new()
                        } else {
                            format!(": {subtitle}")
                        }
                    ));
                }
            }
            s
        })
        .unwrap_or_default();

    let memory_section = memory
        .as_ref()
        .filter(|m| !m.context.trim().is_empty())
        .map(|m| {
            format!(
                "\n\n=== Saved memory (facts, preferences, prior decisions) ===\n{}",
                m.context.trim()
            )
        })
        .unwrap_or_default();

    let schema = json!({
        "type": "object",
        "properties": {
            "reply": { "type": "string" }
        },
        "required": ["reply"]
    });

    let prompt = format!(
        "You are an agentic email drafter inside Trace, a personal project manager.\n\
         {account_context}\n\
         \n\
         Draft a reply to the most recent [INBOUND] message in this thread. Use ALL of the context below — thread-linked entities (deliverables, stakeholders, meetings), broader work-graph entities by relevance, and saved memory. Reference them concretely where useful (use the actual deliverable titles, dates, names). Do NOT invent facts that are not in the transcript or these context sections.\n\
         \n\
         Critical rules to avoid common mistakes:\n\
         - DO NOT promise to do something the [YOU] messages already say has been done or sent. Check the transcript carefully — if you already shared an article, a doc, or a decision, the reply should acknowledge the inbound response, not re-promise the same thing.\n\
         - If the inbound message is asking a question or requesting something specific, address that directly. If it's a status update or acknowledgement, respond proportionally (short ack, or move the conversation forward).\n\
         - If you have a relevant deliverable, task, or commitment in the context that the other side should know about, weave it in naturally with specifics (title + state + deadline if available).\n\
         \n\
         Voice & format:\n\
         - Concise, professional, but warm — sound like the workspace owner, not a generic assistant.\n\
         - Plain text. Use blank lines between paragraphs. No markdown headings or bullet lists unless the incoming email itself uses them.\n\
         - If a decision or commitment is being made, be specific (concrete dates, names, numbers).\n\
         - If there's not enough info to commit, propose one clear next step or ask the single most useful clarifying question.\n\
         - Do NOT include a subject line, salutation like \"Dear ...\", or a signature/sign-off block — those are handled separately.\n\
         \n\
         === Thread-linked context (stakeholders, active deliverables, meetings already linked to this thread) ===\n{thread_graph_text}\n{brain_section}{memory_section}\n\n\
         === Thread (oldest message first; [YOU] = workspace owner, [INBOUND] = others) ===\n{transcript}\n\n\
         Return JSON only: {{ \"reply\": \"...\" }}"
    );

    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema
        }
    });

    let raw_json = crate::gemini::post_gemini_external(
        Some(pool),
        "email_draft_agentic",
        "gemini-3-flash-preview",
        api_key,
        &body,
    )
    .await?;

    let text = raw_json
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .and_then(|p| p.last())
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| "Gemini draft response did not include text".to_string())?
        .to_string();

    #[derive(Deserialize)]
    struct DraftResponse {
        reply: String,
    }
    let parsed: DraftResponse = serde_json::from_str(&text)
        .map_err(|e| format!("Gemini draft JSON failed validation: {e}"))?;
    Ok(parsed.reply)
}

pub async fn triage_thread_with_gemini(
    api_key: &str,
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<GmailTriageResult, String> {
    let _ = auto_link_thread(pool, thread_id).await;
    let detail = get_local_thread(pool, thread_id).await?;
    let transcript = thread_transcript(&detail);
    let graph_context = build_graph_context_for_thread(pool, thread_id)
        .await
        .unwrap_or_else(|_| json!({}));
    let graph_context_text = serde_json::to_string_pretty(&graph_context)
        .unwrap_or_else(|_| "{}".to_string());
    let schema = json!({
        "type": "object",
        "properties": {
            "category": {
                "type": "string",
                "enum": ["personal", "work", "action_required", "newsletter", "receipt", "meeting", "archive", "spam", "other"]
            },
            "priority": {
                "type": "string",
                "enum": ["low", "medium", "high", "urgent"]
            },
            "confidence": { "type": "number" },
            "reasons": {
                "type": "array",
                "items": { "type": "string" }
            },
            "suggested_actions": {
                "type": "array",
                "items": {
                    "type": "string",
                    "enum": ["mark_important", "star", "archive", "move_to_spam", "create_capture", "create_task", "create_deliverable", "create_stakeholder", "reply", "no_action"]
                }
            },
            "stakeholder_candidates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "email": { "type": "string" },
                        "reason": { "type": "string" },
                        "confidence": { "type": "number" }
                    },
                    "required": ["name", "email", "reason"]
                }
            }
        },
        "required": ["category", "priority", "reasons", "suggested_actions", "stakeholder_candidates"]
    });
    let prompt = format!(
        "You are Trace's Gmail triage agent. Read this synced email thread and produce a reviewable triage decision.\n\
         Return JSON only. Do not take actions.\n\
         Categorize as personal, work, action_required, newsletter, receipt, meeting, archive, spam, or other.\n\
         Personal means direct human/stakeholder-like mail; newsletters, receipts, automated alerts, promotions, and bulk mail are not personal.\n\
         Priority should reflect user attention, not sender importance.\n\
         Treat automated marketing, newsletters, community mentions, OTPs, sign-in alerts, and obvious bulk mail as archive/newsletter/spam unless they clearly affect active work.\n\
         Recommend stakeholder candidates only for real people or high-value recurring work contacts, not noreply, marketing, bots, newsletters, or automated services.\n\
         Suggested actions are review suggestions only.\n\n\
         Graph context already known by Trace. Use it to classify and rank work impact, but do not invent facts outside the email and graph packet:\n{}\n\n\
         Thread:\n{}",
        graph_context_text,
        transcript
    );
    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema,
            // See note on analyze_thread_with_gemini above — same caps apply
            // here so the triage path can't run away either.
            "temperature": 0.1,
            "maxOutputTokens": 8000,
            "thinkingConfig": { "thinkingBudget": 2048 }
        }
    });
    const TRIAGE_MODEL: &str = "gemini-3-flash-preview";
    let raw = crate::gemini::post_gemini_external(
        Some(pool),
        "email_classify",
        TRIAGE_MODEL,
        api_key,
        &body,
    )
    .await?;
    #[derive(Deserialize)]
    struct GeminiResponse {
        candidates: Option<Vec<GeminiCandidate>>,
    }
    #[derive(Deserialize)]
    struct GeminiCandidate {
        content: Option<GeminiContent>,
    }
    #[derive(Deserialize)]
    struct GeminiContent {
        parts: Option<Vec<GeminiPart>>,
    }
    #[derive(Deserialize)]
    struct GeminiPart {
        text: Option<String>,
    }
    let response: GeminiResponse = serde_json::from_value(raw)
        .map_err(|e| format!("Gemini email triage response was not valid JSON: {e}"))?;
    let text = response
        .candidates
        .and_then(|mut candidates| candidates.pop())
        .and_then(|candidate| candidate.content)
        .and_then(|content| content.parts)
        .and_then(|mut parts| parts.pop())
        .and_then(|part| part.text)
        .ok_or_else(|| "Gemini email triage response did not include text".to_string())?;
    let result: GmailTriageResult = serde_json::from_str(&text)
        .map_err(|e| format!("Gemini email triage JSON failed validation: {e}"))?;

    let now = now_utc();
    let category = normalize_ai_category(&result.category);
    let priority = normalize_ai_priority(&result.priority);
    sqlx::query(
        r#"
        UPDATE gmail_threads
        SET ai_category = ?,
            ai_priority = ?,
            ai_category_confidence = ?,
            ai_category_reasons = ?,
            ai_triaged_at = ?,
            graph_context_json = ?,
            last_analysis_error = NULL
        WHERE thread_id = ?
        "#,
    )
    .bind(&category)
    .bind(&priority)
    .bind(result.confidence)
    .bind(to_json_string(&result.reasons))
    .bind(&now)
    .bind(graph_context.to_string())
    .bind(thread_id)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    sqlx::query(
        r#"
        INSERT INTO gmail_ai_suggestions (id, thread_id, kind, title, body, payload, status, created_at)
        VALUES (?, ?, 'triage', ?, ?, ?, 'pending', ?)
        "#,
    )
    .bind(Ulid::new().to_string())
    .bind(thread_id)
    .bind(format!("{category} / {priority}"))
    .bind(result.reasons.join("\n"))
    .bind(to_json_string(&result))
    .bind(now)
    .execute(pool)
    .await
    .map_err(crate::db::sql_error)?;

    refresh_thread_intelligence(pool, thread_id).await?;
    Ok(result)
}

fn ai_candidate_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "body": { "type": "string" },
            "kind": { "type": "string" },
            "due_date": { "type": "string" },
            "artifact_url": { "type": "string" },
            "confidence": { "type": "number" }
        },
        "required": ["title"]
    })
}

fn normalize_ai_category(value: &str) -> String {
    match value {
        "personal" | "work" | "action_required" | "newsletter" | "receipt" | "meeting"
        | "archive" | "spam" | "other" => value.to_string(),
        "important" | "priority" | "task" => "action_required".to_string(),
        _ => "other".to_string(),
    }
}

fn normalize_ai_priority(value: &str) -> String {
    match value {
        "low" | "medium" | "high" | "urgent" => value.to_string(),
        _ => "low".to_string(),
    }
}

async fn store_ai_suggestions(
    pool: &SqlitePool,
    thread_id: &str,
    result: &GmailAiResult,
) -> Result<(), String> {
    let now = now_utc();
    let groups = [
        ("task", &result.tasks),
        ("deliverable", &result.deliverables),
        ("initiative", &result.initiatives),
        ("deadline", &result.deadlines),
    ];
    for (kind, candidates) in groups {
        for candidate in candidates {
            sqlx::query(
                r#"
                INSERT INTO gmail_ai_suggestions (id, thread_id, kind, title, body, payload, status, created_at)
                VALUES (?, ?, ?, ?, ?, ?, 'pending', ?)
                "#,
            )
            .bind(Ulid::new().to_string())
            .bind(thread_id)
            .bind(kind)
            .bind(&candidate.title)
            .bind(&candidate.body)
            .bind(to_json_string(candidate))
            .bind(&now)
            .execute(pool)
            .await
            .map_err(crate::db::sql_error)?;
        }
    }
    Ok(())
}

fn thread_transcript(detail: &GmailThreadDetail) -> String {
    let mut text = format!("Subject: {}\n\n", detail.thread.subject);
    for message in &detail.messages {
        let raw_body = if message.plain_body.trim().is_empty() {
            message.html_body.clone()
        } else {
            message.plain_body.clone()
        };
        let date = format_ts(message.internal_date_ts.or(message.date_ts)).unwrap_or_default();
        let sanitized = crate::prompt_safety::sanitize_email_body(&raw_body);
        let wrapped = crate::prompt_safety::wrap_email_body(
            &message.from_email,
            &date,
            &sanitized,
        );
        text.push_str(&format!(
            "From: {} <{}>\nTo: {}\nDate: {}\n\n{}\n\n---\n\n",
            message.from_name,
            message.from_email,
            message
                .to
                .iter()
                .map(|address| address.email.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            date,
            wrapped
        ));
    }
    text
}

/// Same as `thread_transcript` but prefixes each message with `[YOU]` (owner)
/// or `[INBOUND]` (anyone else). Used by the agentic draft so the model knows
/// which messages are already-sent and which one is being replied to.
fn thread_transcript_with_direction(
    detail: &GmailThreadDetail,
    owner_email: Option<&str>,
) -> String {
    let owner_lower = owner_email.map(|e| e.trim().to_lowercase());
    let mut text = format!("Subject: {}\n\n", detail.thread.subject);
    for message in &detail.messages {
        let raw_body = if message.plain_body.trim().is_empty() {
            message.html_body.clone()
        } else {
            message.plain_body.clone()
        };
        let from_is_owner = match owner_lower.as_deref() {
            Some(owner) => message.from_email.trim().to_lowercase() == owner,
            None => message.is_sent,
        };
        let direction = if from_is_owner { "[YOU]" } else { "[INBOUND]" };
        let date = format_ts(message.internal_date_ts.or(message.date_ts)).unwrap_or_default();
        // Owner-sent messages are first-party content; outbound replies the
        // user wrote themselves shouldn't be wrapped as untrusted. Only wrap
        // inbound messages.
        let body_block = if from_is_owner {
            if message.plain_body.trim().is_empty() {
                strip_html(&message.html_body)
            } else {
                message.plain_body.clone()
            }
        } else {
            let sanitized = crate::prompt_safety::sanitize_email_body(&raw_body);
            crate::prompt_safety::wrap_email_body(
                &message.from_email,
                &date,
                &sanitized,
            )
        };
        text.push_str(&format!(
            "{} From: {} <{}>\nTo: {}\nDate: {}\n\n{}\n\n---\n\n",
            direction,
            message.from_name,
            message.from_email,
            message
                .to
                .iter()
                .map(|address| address.email.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            date,
            body_block
        ));
    }
    text
}
