//! Non-streaming Ask surface.
//!
//! `ask_search` is the synchronous one-shot Ask entry point used by the
//! eval-runner CLI and other batch callers — it runs the same agentic loop
//! as `super::streaming::ask_search_stream` but accumulates all text into a
//! single `AskSearchResult` instead of emitting `AskRunEvent`s.
//!
//! `parse_agentic_answer`, `split_answer_and_metadata`, and
//! `parse_trace_metadata` are the shared response parsers: they understand
//! both the "markdown answer + ```trace-meta JSON trailer" format used by
//! the current ASK_SYSTEM_PROMPT and the legacy bare-JSON fallback.
//! `super::streaming::ask_search_stream_inner` imports
//! `parse_agentic_answer` for the streaming Done event.

use std::path::Path;

use serde_json::json;

use super::client::post_gemini;
use super::legacy::{GeminiPart, GeminiResponse};
use super::prompts::ASK_SYSTEM_PROMPT;
use super::streaming::{ask_user_prompt, prune_old_tool_responses, tool_label, ASK_MODEL};
use super::tools::{build_tool_declarations, dispatch_tool};

use crate::models::{AskSearchResult, GeminiAskOutput, SearchResult};

pub async fn ask_search(
    api_key: &str,
    question: &str,
    context: Option<&str>,
    pool: &sqlx::SqlitePool,
    brain_path: Option<&Path>,
    progress: Option<tokio::sync::mpsc::UnboundedSender<crate::models::AskProgressEvent>>,
) -> Result<AskSearchResult, String> {
    let tools = json!([{ "functionDeclarations": build_tool_declarations("ask") }]);
    let user_prompt = ask_user_prompt(question, context);

    let mut contents: Vec<serde_json::Value> = vec![json!({
        "role": "user",
        "parts": [{ "text": user_prompt }]
    })];

    // Cache the (ASK_SYSTEM_PROMPT + ask tools) preamble so each turn after
    // the first only pays the cached rate for ~1.4k–5k preamble tokens.
    // Falls back to inline if the ensemble is below Gemini's cache minimum.
    let cache_name =
        crate::gemini_cache::ensure_cache(api_key, ASK_MODEL, ASK_SYSTEM_PROMPT, &tools, 3600)
            .await
            .unwrap_or(None);

    for _ in 0..10u32 {
        // Cap accumulating tool responses — keep only the two most recent
        // iterations full-fidelity; older ones become summary stubs. See
        // `prune_old_tool_responses` for rationale.
        prune_old_tool_responses(&mut contents, 2);
        // Gemini rejects requests that set `system_instruction`, `tools`, or
        // `tool_config` alongside `cachedContent` — those must live in the
        // cache itself. AUTO is Gemini's default function-calling mode when
        // `tool_config` is omitted, so dropping it on the cached path is a
        // no-op behaviourally.
        let body = if let Some(name) = cache_name.as_ref() {
            json!({
                "cachedContent": name,
                "contents": contents,
                "generationConfig": { "temperature": 0.2 }
            })
        } else {
            json!({
                "systemInstruction": { "parts": [{ "text": ASK_SYSTEM_PROMPT }] },
                "tools": tools,
                "toolConfig": { "functionCallingConfig": { "mode": "AUTO" } },
                "contents": contents,
                "generationConfig": { "temperature": 0.2 }
            })
        };

        let raw_json = post_gemini(Some(pool), "ask", ASK_MODEL, api_key, &body).await?;

        // Extract model content for history — push as-is so no fields are lost.
        let model_content = raw_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .cloned()
            .ok_or_else(|| "No candidates in Gemini response".to_string())?;

        // Also parse into typed structs for function-call dispatch logic.
        let gemini_resp = serde_json::from_value::<GeminiResponse>(raw_json)
            .map_err(|e| format!("Gemini response failed typed parse: {e}"))?;

        let parts = gemini_resp
            .candidates
            .and_then(|mut c| c.pop())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .unwrap_or_default();

        let fn_calls: Vec<&GeminiPart> =
            parts.iter().filter(|p| p.function_call.is_some()).collect();

        if fn_calls.is_empty() {
            let raw = parts
                .iter()
                .filter_map(|p| p.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n");
            return parse_agentic_answer(&raw);
        }

        // Emit one progress event per tool call before executing
        if let Some(ref tx) = progress {
            for part in &fn_calls {
                if let Some(fc) = &part.function_call {
                    let _ = tx.send(crate::models::AskProgressEvent {
                        kind: "tool_call".to_string(),
                        tool: fc.name.clone(),
                        label: tool_label(&fc.name, &fc.args),
                    });
                }
            }
        }

        // Push the raw model content verbatim so thought_signature is preserved.
        contents.push(model_content);

        // Execute all tool calls and collect responses
        let mut response_parts: Vec<serde_json::Value> = Vec::new();
        for part in &fn_calls {
            if let Some(fc) = &part.function_call {
                let result =
                    dispatch_tool(pool, brain_path, api_key, &fc.name, &fc.args, None).await;
                response_parts.push(json!({
                    "functionResponse": {
                        "name": fc.name,
                        "response": { "result": result }
                    }
                }));
            }
        }
        contents.push(json!({ "role": "user", "parts": response_parts }));
    }

    Err("Agentic search did not converge within the iteration limit".to_string())
}

pub(super) fn parse_agentic_answer(raw: &str) -> Result<AskSearchResult, String> {
    // Format A (new): markdown answer + ```trace-meta JSON trailer.
    if let Some((answer, meta)) = split_answer_and_metadata(raw) {
        let trimmed_answer = answer.trim().to_string();
        if !trimmed_answer.is_empty() {
            let (refs, questions) = parse_trace_metadata(&meta).unwrap_or_default();
            return Ok(AskSearchResult {
                answer: trimmed_answer,
                refs,
                questions,
                scored_nodes: Vec::new(),
                retrieval_query: None,
            });
        }
    }

    // Format B (legacy): bare JSON object.
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(parsed) = serde_json::from_str::<GeminiAskOutput>(cleaned) {
        let refs = parsed
            .refs
            .into_iter()
            .map(|r| SearchResult {
                kind: r.kind,
                entity_id: r.entity_id,
                title: r.title,
                subtitle: None,
                route: r.route,
                state: None,
            })
            .collect();
        return Ok(AskSearchResult {
            answer: parsed.answer,
            refs,
            questions: parsed.questions,
            scored_nodes: Vec::new(),
            retrieval_query: None,
        });
    }

    // Fallback: treat the entire response as a plain-text answer.
    if !cleaned.is_empty() {
        return Ok(AskSearchResult {
            answer: cleaned.to_string(),
            refs: Vec::new(),
            questions: Vec::new(),
            scored_nodes: Vec::new(),
            retrieval_query: None,
        });
    }

    Err("Gemini returned an empty answer".to_string())
}

/// If `raw` contains a ```trace-meta fenced block, return (text-before, metadata-json).
pub(crate) fn split_answer_and_metadata(raw: &str) -> Option<(String, String)> {
    let fence = "```trace-meta";
    let start = raw.find(fence)?;
    let after = &raw[start + fence.len()..];
    // Skip optional language identifier or whitespace up to a newline.
    let body_start = after.find('\n')? + 1;
    let body = &after[body_start..];
    let end = body.find("```")?;
    let answer_text = raw[..start].to_string();
    let meta_text = body[..end].trim().to_string();
    Some((answer_text, meta_text))
}

fn parse_trace_metadata(
    meta: &str,
) -> Option<(Vec<SearchResult>, Vec<crate::models::AskUserQuestion>)> {
    let parsed = serde_json::from_str::<GeminiAskOutput>(meta).ok()?;
    let refs = parsed
        .refs
        .into_iter()
        .map(|r| SearchResult {
            kind: r.kind,
            entity_id: r.entity_id,
            title: r.title,
            subtitle: None,
            route: r.route,
            state: None,
        })
        .collect();
    Some((refs, parsed.questions))
}
