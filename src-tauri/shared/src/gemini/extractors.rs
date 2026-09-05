//! Gemini-driven extractors: meeting minutes, weekly digest, and memory
//! candidate extraction.
//!
//! These all share the same shape — call `super::streaming::run_agentic_loop`
//! or the non-streaming `super::client::post_gemini_external` chokepoint with
//! a system prompt + tool subset, then parse the model's JSON response into a
//! typed struct from `crate::models`. They sit together because they're the
//! three "structured-output extraction" entry points, distinct from Ask
//! (which is conversational) and stakeholder briefing (which is summarization).

use serde_json::json;

use super::client::post_gemini_external;
use super::legacy::extract_text_from_response;
use super::prompts::{DIGEST_SYSTEM_PROMPT, MINUTES_SYSTEM_PROMPT_BASE};
use super::streaming::run_agentic_loop;
use super::tools::{build_digest_tool_declarations, build_minutes_tool_declarations};

use crate::models::ExtractedMemoriesPayload;


// ── process_minutes_agentic ───────────────────────────────────────────────────

pub async fn process_minutes_agentic(
    api_key: &str,
    input: &crate::models::MinutesInput,
    pool: &sqlx::SqlitePool,
    progress: Option<tokio::sync::mpsc::UnboundedSender<crate::models::AskProgressEvent>>,
) -> Result<crate::models::MinutesProcessingResult, String> {
    let today = chrono::Local::now().format("%Y-%m-%d (%A)").to_string();
    let system_prompt = MINUTES_SYSTEM_PROMPT_BASE.replace("{TODAY}", &today);

    let preamble = format!(
        "Today is {today}. File: \"{}\". Process all meeting notes below — extract the meeting date, title, and file every action item into the workspace.",
        input.filename
    );

    let initial_parts: Vec<serde_json::Value> = if let Some(pdf) = &input.pdf_base64 {
        vec![
            json!({ "text": preamble }),
            json!({ "inlineData": { "mimeType": "application/pdf", "data": pdf } }),
        ]
    } else {
        let text = input.text.as_deref().unwrap_or("(no content)");
        vec![json!({ "text": format!("{preamble}\n\n---\n\n{text}") })]
    };

    let initial_contents = vec![json!({ "role": "user", "parts": initial_parts })];

    let raw = run_agentic_loop(
        "meeting_minutes",
        api_key,
        &system_prompt,
        initial_contents,
        build_minutes_tool_declarations(),
        pool,
        progress,
    )
    .await?;

    parse_minutes_result(&raw)
}

fn parse_minutes_result(raw: &str) -> Result<crate::models::MinutesProcessingResult, String> {
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<crate::models::MinutesProcessingResult>(cleaned)
        .map_err(|e| format!("Minutes result parse failed: {e}"))
}

// ── generate_weekly_digest ────────────────────────────────────────────────────

pub async fn generate_weekly_digest(
    api_key: &str,
    pool: &sqlx::SqlitePool,
) -> Result<crate::models::WeeklyDigest, String> {
    let initial_contents = vec![json!({
        "role": "user",
        "parts": [{ "text": "Generate a weekly workspace health digest. Use the available tools to scan all relevant data." }]
    })];

    let raw = run_agentic_loop(
        "weekly_digest",
        api_key,
        DIGEST_SYSTEM_PROMPT,
        initial_contents,
        build_digest_tool_declarations(),
        pool,
        None,
    )
    .await?;

    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<crate::models::WeeklyDigest>(cleaned)
        .map_err(|e| format!("Digest parse failed: {e}"))
}

// ── Memory extraction ─────────────────────────────────────────────────────────

const MEMORY_EXTRACTION_MODEL: &str = "gemini-3-flash-preview";

pub async fn extract_memory_candidates(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    system_prompt: &str,
    source: &str,
) -> Result<ExtractedMemoriesPayload, String> {
    extract_memory_candidates_with_origin(pool, api_key, system_prompt, source, "memory", None)
        .await
}

pub async fn extract_memory_candidates_with_origin(
    pool: &sqlx::SqlitePool,
    api_key: &str,
    system_prompt: &str,
    source: &str,
    source_kind: &str,
    source_id: Option<&str>,
) -> Result<ExtractedMemoriesPayload, String> {
    let trimmed_source = source.trim();
    if trimmed_source.is_empty() {
        return Ok(ExtractedMemoriesPayload {
            memories: Vec::new(),
        });
    }

    let sanitized = crate::prompt_safety::sanitize_plain_text(
        trimmed_source,
        crate::prompt_safety::MEMORY_SOURCE_CAP,
    );
    crate::prompt_safety::log_if_noteworthy(
        Some(pool),
        "memory",
        source_kind,
        source_id,
        None,
        trimmed_source,
        &sanitized,
    )
    .await;
    let wrapped_source =
        crate::prompt_safety::wrap_memory_source(source_kind, source_id, &sanitized);

    let schema = json!({
        "type": "object",
        "properties": {
            "memories": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": ["episodic", "semantic", "procedural"] },
                        "title": { "type": "string" },
                        "body": { "type": "string" },
                        "scope": { "type": "string", "enum": ["global", "project", "session"] },
                        "tags": { "type": "array", "items": { "type": "string" } },
                        "confidence": { "type": "number" },
                        "importance": { "type": "number" },
                        "sensitivity": { "type": "string", "enum": ["normal", "pii", "sensitive"] },
                        "evidence": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["kind", "title", "body"]
                }
            }
        },
        "required": ["memories"]
    });

    let body = json!({
        "systemInstruction": {
            "role": "system",
            "parts": [{ "text": system_prompt }]
        },
        "contents": [{
            "role": "user",
            "parts": [{
                "text": format!(
                    "Extract durable memories from the following source text.\n\nSOURCE TEXT:\n---\n{wrapped_source}\n---"
                )
            }]
        }],
        "generationConfig": {
            "temperature": 0.2,
            "responseMimeType": "application/json",
            "responseJsonSchema": schema
        }
    });

    let raw = post_gemini_external(
        Some(pool),
        "memory_extract",
        MEMORY_EXTRACTION_MODEL,
        api_key,
        &body,
    )
    .await?;
    let text = extract_text_from_response(raw)?;

    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<ExtractedMemoriesPayload>(cleaned)
        .map_err(|error| format!("Gemini memory extraction parse failed: {error}"))
}
