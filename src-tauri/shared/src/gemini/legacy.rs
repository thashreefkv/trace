use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    gmail::GmailLocalThread,
    models::{
        BriefingItem, BriefingSections, ConversationExtractionInput,
        ConversationExtractionResult, Deliverable, ExtractedConversation,
        ExtractedDeliverableCandidate, GeminiMeetingOutput, GeneratedWeekPlan, Meeting,
        Stakeholder, StakeholderBriefing, WeekDayAssignment,
    },
    repo::build_fallback_sections,
};

use super::client::post_gemini_external;
use super::prompts::MEETING_PROCESSING_PROMPT;

const BRIEFING_MODEL: &str = "gemini-3-flash-preview";
const EXTRACTION_MODEL: &str = "gemini-3.1-pro-preview";

#[derive(Debug, Deserialize)]
pub(super) struct GeminiResponse {
    pub(super) candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GeminiCandidate {
    pub(super) content: Option<GeminiContent>,
}

/// Pull the last text part out of a `generateContent` response. Most
/// non-streaming features want exactly this shape (one candidate, one
/// content block, one text part). Returns the trimmed text or an error
/// if the response had no text candidate.
pub(super) fn extract_text_from_response(raw: serde_json::Value) -> Result<String, String> {
    let parsed: GeminiResponse = serde_json::from_value(raw)
        .map_err(|e| format!("Gemini response failed typed parse: {e}"))?;
    parsed
        .candidates
        .and_then(|mut c| c.pop())
        .and_then(|c| c.content)
        .and_then(|c| c.parts)
        .and_then(|mut parts| parts.pop())
        .and_then(|part| part.text)
        .ok_or_else(|| "Gemini response did not include text".to_string())
}

#[derive(Debug, Deserialize)]
pub(super) struct GeminiContent {
    pub(super) parts: Option<Vec<GeminiPart>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GeminiFunctionCall {
    pub(super) name: String,
    pub(super) args: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub(super) struct GeminiPart {
    pub(super) text: Option<String>,
    #[serde(rename = "functionCall")]
    pub(super) function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct GeminiBriefingOutput {
    tldr: String,
    #[serde(default)]
    open_commitments: Vec<BriefingItem>,
    #[serde(default)]
    waiting_on_them: Vec<BriefingItem>,
    #[serde(default)]
    recent_wins: Vec<BriefingItem>,
    #[serde(default)]
    in_flight: Vec<BriefingItem>,
    #[serde(default)]
    talking_points: Vec<String>,
    watch_out: Option<String>,
}

#[derive(Serialize)]
struct SlimThread<'a> {
    subject: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ai_title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sentiment: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    urgency: Option<&'a str>,
    message_count: i64,
}

#[derive(Serialize)]
struct SlimMeeting<'a> {
    title: &'a str,
    date: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_decisions: Option<&'a str>,
}

#[derive(Serialize)]
struct SlimDeliverable<'a> {
    title: &'a str,
    #[serde(rename = "type")]
    deliverable_type: &'a str,
    state: &'a str,
    claim: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    deadline: Option<&'a str>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GeminiExtractionOutput {
    conversation: ExtractedConversation,
    #[serde(default)]
    candidates: Vec<ExtractedDeliverableCandidate>,
}

pub async fn generate_stakeholder_briefing(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    stakeholder: &Stakeholder,
    deliverables: &[Deliverable],
    recent_threads: &[GmailLocalThread],
    recent_meetings: &[Meeting],
    brain_summary: Option<String>,
) -> Result<StakeholderBriefing, String> {
    let prompt = briefing_prompt(
        stakeholder,
        deliverables,
        recent_threads,
        recent_meetings,
        brain_summary.as_deref(),
    );

    let item_schema = json!({
        "type": "object",
        "properties": {
            "text": { "type": "string" },
            "source": { "type": "string" }
        },
        "required": ["text"]
    });

    let schema = json!({
        "type": "object",
        "properties": {
            "tldr": { "type": "string" },
            "open_commitments": { "type": "array", "items": item_schema },
            "waiting_on_them": { "type": "array", "items": item_schema.clone() },
            "recent_wins": { "type": "array", "items": item_schema.clone() },
            "in_flight": { "type": "array", "items": item_schema.clone() },
            "talking_points": { "type": "array", "items": { "type": "string" } },
            "watch_out": { "type": "string" }
        },
        "required": ["tldr", "open_commitments", "waiting_on_them", "recent_wins", "in_flight", "talking_points"]
    });

    let body = json!({
        "contents": [
            {
                "role": "user",
                "parts": [{ "text": prompt }]
            }
        ],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema
        }
    });

    let raw = post_gemini_external(Some(pool), "briefing", BRIEFING_MODEL, api_key, &body).await?;
    let text = extract_text_from_response(raw)?;

    let parsed = serde_json::from_str::<GeminiBriefingOutput>(&text)
        .map_err(|error| format!("Gemini structured output failed validation: {error}"))?;

    if parsed.tldr.trim().is_empty() {
        return Err("Gemini briefing tldr was empty".to_string());
    }

    Ok(StakeholderBriefing {
        stakeholder: stakeholder.clone(),
        generated_with: BRIEFING_MODEL.to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        sections: BriefingSections {
            tldr: parsed.tldr,
            open_commitments: parsed.open_commitments,
            waiting_on_them: parsed.waiting_on_them,
            recent_wins: parsed.recent_wins,
            in_flight: parsed.in_flight,
            talking_points: parsed.talking_points,
            watch_out: parsed.watch_out,
        },
    })
}

pub fn fallback_stakeholder_briefing(
    stakeholder: Stakeholder,
    deliverables: Vec<Deliverable>,
) -> StakeholderBriefing {
    StakeholderBriefing {
        sections: build_fallback_sections(&deliverables),
        stakeholder,
        generated_with: "fallback".to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[derive(Debug, Deserialize)]
pub struct BriefingCommentAction {
    pub action_type: String,
    pub entity_id: Option<String>,
    pub new_state: Option<String>,
    pub content: Option<String>,
    pub action_summary: String,
    pub memory_title: String,
    pub memory_body: String,
}

pub async fn process_briefing_comment(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    stakeholder_name: &str,
    section: &str,
    item_text: &str,
    item_source: Option<&str>,
    user_comment: &str,
    deliverables: &[(&str, &str, &str)], // (id, title, state)
) -> Result<BriefingCommentAction, String> {
    let source_label = item_source.unwrap_or("general");

    let del_list = if deliverables.is_empty() {
        "  (none)".to_string()
    } else {
        deliverables
            .iter()
            .map(|(id, title, state)| format!("  id={id}  [{state}]  {title}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let prompt = format!(
        "You are a chief-of-staff AI for someone who works with {stakeholder_name}.\n\
         The user left a comment on a briefing item. Interpret it and pick the best action.\n\n\
         Briefing section: {section}\n\
         Item: {item_text}\n\
         Source: {source_label}\n\
         User comment: \"{user_comment}\"\n\n\
         Deliverables for {stakeholder_name}:\n{del_list}\n\n\
         Choose ONE action_type:\n\
         - \"update_deliverable_state\": user says a deliverable is done/shipped/cancelled/in-review/back to drafting. \
           Set entity_id to the exact deliverable id from the list above, new_state to one of: \
           backlog, todo, drafting, in_review, shipped, killed.\n\
         - \"append_stakeholder_note\": user shares a preference, constraint, or context about {stakeholder_name} \
           that should be remembered long-term. Set content to the note.\n\
         - \"create_capture\": user wants to log a new follow-up action item or task. Set content to the task text.\n\
         - \"save_memory\": everything else — an observation, update, or piece of context.\n\n\
         Also return:\n\
         - action_summary: 1 specific sentence describing what was done (include names/states)\n\
         - memory_title: ≤60 chars\n\
         - memory_body: 2-3 sentences with full context including stakeholder name"
    );

    let schema = json!({
        "type": "object",
        "properties": {
            "action_type": { "type": "string" },
            "entity_id": { "type": "string" },
            "new_state": { "type": "string" },
            "content": { "type": "string" },
            "action_summary": { "type": "string" },
            "memory_title": { "type": "string" },
            "memory_body": { "type": "string" }
        },
        "required": ["action_type", "action_summary", "memory_title", "memory_body"]
    });

    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema
        }
    });

    let raw = post_gemini_external(Some(pool), "briefing", BRIEFING_MODEL, api_key, &body).await?;
    let text = extract_text_from_response(raw)?;

    serde_json::from_str::<BriefingCommentAction>(&text)
        .map_err(|e| format!("Gemini comment output invalid: {e}"))
}

pub async fn extract_conversation(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    input: ConversationExtractionInput,
    prompt_template: &str,
) -> Result<ConversationExtractionResult, String> {
    let source_chat_url = clean_optional(input.chat_url)
        .map(|value| crate::repo::normalize_claude_link(&value))
        .transpose()?;
    let pasted_text = clean_optional(input.pasted_text);

    if pasted_text.is_none() && source_chat_url.is_none() {
        return Err("Paste a Claude chat export or provide a Claude chat URL.".to_string());
    }

    let Some(pasted_text) = pasted_text else {
        return Err("Trace cannot read private Claude chats from a URL alone. Paste the chat export text to extract candidates.".to_string());
    };

    let prompt = extraction_prompt(prompt_template, source_chat_url.as_deref(), &pasted_text);
    let body = json!({
        "contents": [
            {
                "role": "user",
                "parts": [{ "text": prompt }]
            }
        ],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": extraction_schema()
        }
    });

    let raw =
        post_gemini_external(Some(pool), "conversation_extract", EXTRACTION_MODEL, api_key, &body)
            .await?;
    let text = extract_text_from_response(raw)?;

    parse_extraction_text(
        &text,
        source_chat_url,
        if pasted_text.is_empty() {
            "url"
        } else {
            "pasted_text"
        },
    )
}

pub fn parse_extraction_text(
    text: &str,
    source_chat_url: Option<String>,
    source_kind: &str,
) -> Result<ConversationExtractionResult, String> {
    let parsed = serde_json::from_str::<GeminiExtractionOutput>(text)
        .map_err(|error| format!("Gemini extraction JSON failed validation: {error}"))?;

    validate_extraction_output(parsed, source_chat_url, source_kind)
}

const TASK_GEN_MODEL: &str = "gemini-3-flash-preview";

pub async fn generate_tasks(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    title: &str,
    claim: &str,
    deliverable_type: &str,
    state: &str,
    deadline: Option<&str>,
    initiatives: &[&str],
    stakeholder_names: &[&str],
    existing_tasks: &[(&str, &str)],
    notes: &[&str],
    email_summaries: &[&str],
    meeting_notes: &[&str],
    calendar_events: &[&str],
    brain_context: Option<&str>,
) -> Result<crate::models::GeneratedTasks, String> {
    let today = chrono::Utc::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    let deadline_line = deadline
        .map(|d| format!("Deadline: {d}\n"))
        .unwrap_or_default();

    let state_guidance = match state {
        "in_review" => "The deliverable is IN REVIEW — focus on tasks that get sign-off, address feedback, and close review loops. Avoid drafting tasks.",
        "shipped" => "The deliverable is SHIPPED — focus on follow-up, communication, and measurement tasks only.",
        "drafting" => "The deliverable is DRAFTING — include both creation and early review tasks.",
        _ => "Focus on tasks that move this deliverable forward.",
    };

    let initiatives_line = if initiatives.is_empty() {
        String::new()
    } else {
        format!("Strategic context: {}\n", initiatives.join(", "))
    };

    let stakeholders_line = if stakeholder_names.is_empty() {
        String::new()
    } else {
        format!("Stakeholders: {}\n", stakeholder_names.join(", "))
    };

    let brain_line = brain_context
        .filter(|s| !s.trim().is_empty())
        .map(|ctx| format!("Relevant work memory (avoid duplicating done work; use real names/artifacts from here):\n{ctx}\n"))
        .unwrap_or_default();

    let email_line = if email_summaries.is_empty() {
        String::new()
    } else {
        let list = email_summaries
            .iter()
            .map(|s| format!("  - {s}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Linked email threads (use for decisions, blockers, open questions):\n{list}\n")
    };

    let meeting_line = if meeting_notes.is_empty() {
        String::new()
    } else {
        let list = meeting_notes
            .iter()
            .map(|s| format!("  - {s}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Recent meeting notes (decisions and action items from these inform what tasks remain):\n{list}\n")
    };

    let calendar_line = if calendar_events.is_empty() {
        String::new()
    } else {
        let list = calendar_events
            .iter()
            .map(|s| format!("  - {s}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Upcoming/recent calendar events with stakeholders (use to schedule tasks around them or identify prep work):\n{list}\n")
    };

    let existing_line = if existing_tasks.is_empty() {
        String::new()
    } else {
        let list = existing_tasks
            .iter()
            .map(|(t, s)| format!("  - [{s}] {t}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Existing tasks (do not recreate these):\n{list}\n")
    };

    let notes_line = if notes.is_empty() {
        String::new()
    } else {
        let list = notes
            .iter()
            .map(|n| format!("  - {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("Working notes:\n{list}\n")
    };

    let prompt = format!(
        "You are a chief-of-staff AI helping a PM get work done.\n\
         Generate a focused, actionable task list for a deliverable.\n\n\
         Today: {today}\n\
         {deadline_line}\
         Current state: {state} — {state_guidance}\n\
         {initiatives_line}\
         {stakeholders_line}\
         {brain_line}\
         {email_line}\
         {meeting_line}\
         {calendar_line}\
         {existing_line}\
         {notes_line}\n\
         Rules:\n\
         - Return 3–8 tasks. Stop when the list is complete — do not pad.\n\
         - Each task is one concrete action (30 min – 3 hrs). No phases or milestones.\n\
         - Imperative, no subject: \"Send draft to Indu\", \"Interview 3 users\", \"Get sign-off from CEO\".\n\
         - Reference REAL names from stakeholders/notes/memory above. Never invent people or teams.\n\
         - If work memory mentions something already done, do NOT create a task for it.\n\
         - Order chronologically — earlier work first.\n\
         - Assign due_date YYYY-MM-DD only when inferable from deadline, urgency, or sequence. null otherwise.\n\
         - suggested_deliverable_deadline: last task date or explicit deadline. null if unjustified.\n\
         - No filler: never \"Review\", \"Finalize\", \"Polish\", \"Follow up\", \"Sync with team\".\n\n\
         Title: {title}\n\
         Type: {deliverable_type}\n\
         Goal: {claim}\n\n\
         Return JSON matching the schema only."
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "tasks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "due_date": { "type": "string", "nullable": true },
                        "reason": { "type": "string" }
                    },
                    "required": ["title"]
                }
            },
            "suggested_deliverable_deadline": { "type": "string", "nullable": true },
            "rationale": { "type": "string" }
        },
        "required": ["tasks", "rationale"]
    });

    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema
        }
    });

    let raw =
        post_gemini_external(Some(pool), "task_generation", TASK_GEN_MODEL, api_key, &body).await?;
    let text = extract_text_from_response(raw)?;

    #[derive(serde::Deserialize)]
    struct TasksOutput {
        tasks: Vec<crate::models::GeneratedTaskSuggestion>,
        #[serde(default)]
        suggested_deliverable_deadline: Option<String>,
        #[serde(default)]
        rationale: String,
    }

    let parsed = serde_json::from_str::<TasksOutput>(&text)
        .map_err(|e| format!("Gemini task output failed validation: {e}"))?;

    let tasks: Vec<crate::models::GeneratedTaskSuggestion> = parsed
        .tasks
        .into_iter()
        .map(|mut task| {
            task.title = task.title.trim().to_string();
            task.reason = task.reason.trim().to_string();
            task
        })
        .filter(|task| !task.title.is_empty())
        .collect();

    if tasks.is_empty() {
        return Err("Gemini returned no tasks".to_string());
    }

    Ok(crate::models::GeneratedTasks {
        tasks,
        suggested_deliverable_deadline: clean_optional(parsed.suggested_deliverable_deadline),
        rationale: parsed.rationale.trim().to_string(),
    })
}

pub async fn generate_week_plan(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    week_start: &str,
    meeting_date: Option<&str>,
    deliverables: &[Deliverable],
) -> Result<GeneratedWeekPlan, String> {
    let days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"];
    let day_list = days
        .iter()
        .enumerate()
        .map(|(i, name)| format!("{i} = {name}"))
        .collect::<Vec<_>>()
        .join(", ");

    let deliverables_summary = deliverables
        .iter()
        .map(|d| {
            // Use 2.5 as neutral default so unrated items score 0 rather than -3
            let effort = d.effort.unwrap_or(3) as f64;
            let impact = d.impact.unwrap_or(3) as f64;
            let score = impact - effort;
            let effort_str = d.effort.map(|e| e.to_string()).unwrap_or_else(|| "?".to_string());
            let impact_str = d.impact.map(|i| i.to_string()).unwrap_or_else(|| "?".to_string());
            let deadline_str = d.deadline.as_deref().unwrap_or("none");
            let blocker_str = match &d.blocker_reason {
                Some(r) => format!(" [BLOCKED: {r}]"),
                None => String::new(),
            };
            format!(
                "id={id}\ntitle={title:?}\ntype={typ}  state={state}\neffort={effort_str}/5  impact={impact_str}/5  score={score:+.1}\ndeadline={deadline}\nclaim={claim:?}{blocker}",
                id = d.id,
                title = d.title,
                typ = d.deliverable_type,
                state = d.state,
                deadline = deadline_str,
                claim = d.claim,
                blocker = blocker_str,
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let meeting_line = match meeting_date {
        Some(d) => format!(
            "Next meeting: {d}\n\
             → Assign any meeting_prep deliverables no later than the day BEFORE the meeting.\n\
             → If the meeting falls on Mon or Tue, meeting_prep should be assigned the Friday before (day 4)."
        ),
        None => "No meeting scheduled this week.".to_string(),
    };

    let prompt = format!(
        "You are a personal project management assistant. Plan the most focused, high-leverage work week possible.\n\n\
         Week starting {week_start}. Day index mapping: {day_list}.\n\
         {meeting_line}\n\n\
         Assignment rules:\n\
         - Assign exactly ONE deliverable per day — this is the primary deep-work focus for that day\n\
         - Never assign the same deliverable to more than one day\n\
         - Skip BLOCKED deliverables entirely\n\
         - Sort by score (higher = earlier in the week). Break ties: nearer deadline first, then in_review before drafting\n\
         - Leave a day unassigned (omit it) if the remaining deliverables are all blocked or already assigned\n\
         - Fewer, deeper days beat spreading five things across five days — if there are only 2 strong deliverables, assign 2 days\n\n\
         Active deliverables (score = impact − effort, higher is better):\n\n\
         {deliverables_summary}\n\n\
         Return JSON matching the schema only."
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "assignments": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "day_index": { "type": "integer" },
                        "deliverable_id": { "type": "string" }
                    },
                    "required": ["day_index", "deliverable_id"]
                }
            }
        },
        "required": ["assignments"]
    });

    let body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema
        }
    });

    let raw =
        post_gemini_external(Some(pool), "task_generation", TASK_GEN_MODEL, api_key, &body).await?;
    let text = extract_text_from_response(raw)?;

    #[derive(serde::Deserialize)]
    struct PlanOutput {
        assignments: Vec<WeekDayAssignment>,
    }

    let parsed = serde_json::from_str::<PlanOutput>(&text)
        .map_err(|e| format!("Gemini week plan output failed validation: {e}"))?;

    let valid_ids: std::collections::HashSet<&str> =
        deliverables.iter().map(|d| d.id.as_str()).collect();

    let assignments = parsed
        .assignments
        .into_iter()
        .filter(|a| {
            a.day_index >= 0 && a.day_index < 5 && valid_ids.contains(a.deliverable_id.as_str())
        })
        .collect();

    Ok(GeneratedWeekPlan { assignments })
}

pub const TRANSCRIBE_MODEL: &str = "gemini-3-flash-preview";

pub async fn transcribe_audio(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    audio_base64: &str,
    mime_type: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [
                { "inlineData": { "mimeType": mime_type, "data": audio_base64 } },
                { "text": "Transcribe the speech in this audio clip exactly as spoken. Return only the transcription text, nothing else." }
            ]
        }]
    });

    let raw =
        post_gemini_external(Some(pool), "meeting_minutes", TRANSCRIBE_MODEL, api_key, &body)
            .await?;
    extract_text_from_response(raw)
}

const MEETING_MODEL: &str = "gemini-3-flash-preview";

pub async fn process_meeting_audio(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    audio_base64: &str,
    mime_type: &str,
) -> Result<GeminiMeetingOutput, String> {
    let prompt = MEETING_PROCESSING_PROMPT;

    let schema = json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "transcript": { "type": "string" },
            "summary": { "type": "string" },
            "key_decisions": {
                "type": "array",
                "items": { "type": "string" }
            },
            "action_suggestions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["deliverable_note", "initiative_note", "capture"]
                        },
                        "suggested_target": { "type": "string" },
                        "body": { "type": "string" }
                    },
                    "required": ["kind", "suggested_target", "body"]
                }
            }
        },
        "required": ["title", "transcript", "summary", "key_decisions", "action_suggestions"]
    });

    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [
                {
                    "inlineData": {
                        "mimeType": mime_type,
                        "data": audio_base64
                    }
                },
                { "text": prompt }
            ]
        }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema
        }
    });

    let raw =
        post_gemini_external(Some(pool), "meeting_minutes", MEETING_MODEL, api_key, &body).await?;
    let text = extract_text_from_response(raw)?;

    let output = serde_json::from_str::<GeminiMeetingOutput>(&text)
        .map_err(|e| format!("Gemini meeting output failed JSON validation: {e}"))?;

    if output.title.trim().is_empty() {
        return Err("Gemini returned an empty meeting title".to_string());
    }

    Ok(output)
}

pub async fn test_api_key(pool: &sqlx::SqlitePool, api_key: &str) -> Result<(), String> {
    let stakeholder = Stakeholder {
        id: "test".to_string(),
        name: "Test stakeholder".to_string(),
        display_order: 0,
        email: String::new(),
        role: String::new(),
        notes: String::new(),
        avatar_url: String::new(),
        created_at: crate::repo::now_utc(),
        updated_at: crate::repo::now_utc(),
    };
    generate_stakeholder_briefing(pool, api_key, &stakeholder, &[], &[], &[], None)
        .await
        .map(|_| ())
}

fn briefing_prompt(
    stakeholder: &Stakeholder,
    deliverables: &[Deliverable],
    recent_threads: &[GmailLocalThread],
    recent_meetings: &[Meeting],
    brain_summary: Option<&str>,
) -> String {
    let slim_deliverables: Vec<SlimDeliverable> = deliverables
        .iter()
        .map(|d| SlimDeliverable {
            title: &d.title,
            deliverable_type: &d.deliverable_type,
            state: &d.state,
            claim: &d.claim,
            deadline: d.deadline.as_deref(),
        })
        .collect();
    let deliverables_json =
        serde_json::to_string_pretty(&slim_deliverables).unwrap_or_else(|_| "[]".to_string());

    let slim_threads: Vec<SlimThread> = recent_threads
        .iter()
        .map(|t| SlimThread {
            subject: &t.subject,
            ai_title: t.ai_title.as_deref(),
            summary: t.summary.as_deref(),
            sentiment: t.sentiment.as_deref(),
            urgency: t.urgency.as_deref(),
            message_count: t.message_count,
        })
        .collect();
    let threads_json =
        serde_json::to_string_pretty(&slim_threads).unwrap_or_else(|_| "[]".to_string());

    let slim_meetings: Vec<SlimMeeting> = recent_meetings
        .iter()
        .map(|m| SlimMeeting {
            title: &m.title,
            date: &m.date,
            summary: m.summary.as_deref(),
            key_decisions: m.key_decisions.as_deref(),
        })
        .collect();
    let meetings_json =
        serde_json::to_string_pretty(&slim_meetings).unwrap_or_else(|_| "[]".to_string());

    let brain_section = brain_summary
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("\n### Knowledge graph context\n{s}"))
        .unwrap_or_default();

    format!(
        r#"You are a chief of staff preparing a product manager for their next interaction with {name} ({role}).

Extract structured intelligence from the evidence below. Do not invent facts. If a section has no evidence, return an empty array.

## Evidence

### Deliverables ({n_d} items)
{deliverables_json}

### Recent email threads ({n_t} threads)
{threads_json}

### Past meetings ({n_m} sessions)
{meetings_json}{brain_section}

---

Return JSON with exactly these fields:

- tldr: One tight sentence capturing the current relationship state and the single most important thing the PM needs to know right now.
- open_commitments: Things the PM has committed to, promised, or is accountable for that are not yet done. Derive from email/meeting context and in-review deliverables. Max 4 items. Each: {{ "text": "...", "source": "email"|"meeting"|"deliverable" }}.
- waiting_on_them: Things {name} said they'd do, provide, or decide but hasn't yet. Max 3 items. Each: {{ "text": "...", "source": "email"|"meeting" }}.
- recent_wins: Deliverables or work that shipped recently, worth referencing. Max 3 items. Each: {{ "text": "...", "source": "deliverable" }}.
- in_flight: Active work {name} will care about or needs to know is happening. Max 4 items. Each: {{ "text": "...", "source": "deliverable" }}.
- talking_points: Concrete, opinionated things to raise next ("Ask about the deadline on X", "Confirm if Y is approved") — not generic. Max 5 strings.
- watch_out: One sentence about any sensitivity, risk, or pattern to be careful about. Null if nothing notable.

Rules: Be direct and specific. No corporate filler. Return [] for sections with no evidence."#,
        name = stakeholder.name,
        role = if stakeholder.role.is_empty() {
            "stakeholder"
        } else {
            &stakeholder.role
        },
        n_d = deliverables.len(),
        n_t = recent_threads.len(),
        n_m = recent_meetings.len(),
    )
}

fn extraction_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "conversation": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "summary": { "type": "string" },
                    "occurred_at": { "type": "string" }
                },
                "required": ["title", "summary"]
            },
            "candidates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "type": {
                            "type": "string",
                            "enum": [
                                "deck",
                                "design_doc",
                                "prototype",
                                "analysis",
                                "framework",
                                "pitch",
                                "research",
                                "code",
                                "email",
                                "meeting_prep",
                                "other"
                            ]
                        },
                        "claim": { "type": "string" },
                        "artifact_url": { "type": "string" },
                        "stakeholder_name": { "type": "string" },
                        "initiative_titles": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["title", "type", "claim", "initiative_titles"]
                }
            }
        },
        "required": ["conversation", "candidates"]
    })
}

fn extraction_prompt(prompt_template: &str, chat_url: Option<&str>, pasted_text: &str) -> String {
    format!(
        "{prompt_template}\n\nClaude chat URL, if provided:\n{chat_url}\n\nPasted chat/export text:\n{pasted_text}",
        chat_url = chat_url.unwrap_or("not provided"),
    )
}

fn validate_extraction_output(
    output: GeminiExtractionOutput,
    source_chat_url: Option<String>,
    source_kind: &str,
) -> Result<ConversationExtractionResult, String> {
    let conversation = ExtractedConversation {
        title: required_text(output.conversation.title, "conversation title")?,
        summary: required_text(output.conversation.summary, "conversation summary")?,
        occurred_at: clean_optional(output.conversation.occurred_at),
    };

    let mut candidates = Vec::with_capacity(output.candidates.len());
    for candidate in output.candidates {
        candidates.push(ExtractedDeliverableCandidate {
            title: required_text(candidate.title, "deliverable title")?,
            deliverable_type: candidate.deliverable_type,
            claim: required_text(candidate.claim, "deliverable claim")?,
            artifact_url: clean_optional(candidate.artifact_url),
            stakeholder_name: clean_optional(candidate.stakeholder_name),
            initiative_titles: candidate
                .initiative_titles
                .into_iter()
                .map(|title| title.trim().to_string())
                .filter(|title| !title.is_empty())
                .collect(),
            validation_errors: Vec::new(),
        });
    }

    Ok(ConversationExtractionResult {
        conversation,
        candidates,
        source_chat_url,
        source_kind: source_kind.to_string(),
    })
}

fn required_text(value: String, label: &str) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(format!("{label} is required"))
    } else {
        Ok(value)
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

