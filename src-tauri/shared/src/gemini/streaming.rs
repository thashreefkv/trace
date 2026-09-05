//! Gemini streaming Ask surface.
//!
//! `ask_search_stream` runs the streaming agentic loop that powers the Ask
//! page — it opens an SSE connection against Gemini's `streamGenerateContent`
//! endpoint, dispatches tool calls in parallel via `super::tools::dispatch_tool`,
//! and emits structured `AskRunEvent`s as the agent thinks. `run_agentic_loop`
//! is the non-streaming sibling used by minutes processing and the weekly
//! digest (F8 extractors).
//!
//! Supporting helpers (`prune_old_tool_responses`, `summarize_tool_result`,
//! `ask_user_prompt`, `tool_label`) are kept here too — they live in the
//! streaming surface but are also reused by the non-streaming `ask_search`
//! (now in `super::legacy`, future F7 `ask.rs`), so they expose `pub(super)`.

use std::path::Path;

use serde_json::json;

use super::ask::parse_agentic_answer;
use super::client::{post_gemini, post_gemini_external};
use super::legacy::{GeminiPart, GeminiResponse};
use super::prompts::ASK_SYSTEM_PROMPT;
use super::tools::{build_tool_declarations, dispatch_tool};

use crate::models::{AskAttachment, AskRunEvent, AskSearchResult};

/// Pre-create the Gemini `cachedContents` entry for the ASK ensemble
/// (system prompt + ask-mode tool declarations) so the first Ask of the
/// session lands a cache hit. Errors are swallowed — if Gemini rejects
/// the cache (e.g. content too small) the ASK path falls back to inline.
pub async fn warm_ask_cache(api_key: &str) {
    if api_key.is_empty() {
        return;
    }
    let tools = json!([{ "functionDeclarations": build_tool_declarations("ask") }]);
    let _ = crate::gemini_cache::ensure_cache(api_key, ASK_MODEL, ASK_SYSTEM_PROMPT, &tools, 3600)
        .await;
}

pub(super) const ASK_MODEL: &str = "gemini-3-flash-preview";

/// Streaming variant of [`ask_search`].
///
/// Emits structured `AskRunEvent`s on `events` as the agent thinks. Each iteration of
/// the loop opens an SSE stream against Gemini's `streamGenerateContent` endpoint:
///
/// - Text parts → `AskRunEvent::TextDelta` (final-turn answer streams to the user).
/// - Function-call parts → executed in parallel, with `ToolCallStarted` / `ToolCallDone`
///   events surfacing each tool's rationale and result summary.
/// - The `cancel` token is checked between SSE chunks and tool calls; if cancelled
///   we emit `Cancelled` and bail.
///
/// On success the final assistant turn's accumulated text is parsed via
/// `parse_agentic_answer` (markdown answer + `trace-meta` JSON trailer) and emitted
/// as `Done`.
pub async fn ask_search_stream(
    api_key: &str,
    run_id: &str,
    question: &str,
    context: Option<&str>,
    mode: &str,
    attachments: &[AskAttachment],
    pool: &sqlx::SqlitePool,
    brain_path: Option<&Path>,
    app_support_dir: Option<&Path>,
    events: tokio::sync::mpsc::UnboundedSender<AskRunEvent>,
    cancel: tokio_util::sync::CancellationToken,
    controls: crate::models::AskRunControls,
) {
    let _ = events.send(AskRunEvent::Started {
        run_id: run_id.to_string(),
    });

    match ask_search_stream_inner(
        api_key,
        run_id,
        question,
        context,
        mode,
        attachments,
        pool,
        brain_path,
        app_support_dir,
        events.clone(),
        cancel.clone(),
        controls,
    )
    .await
    {
        Ok(result) => {
            if cancel.is_cancelled() {
                let _ = events.send(AskRunEvent::Cancelled {
                    run_id: run_id.to_string(),
                });
            } else {
                let _ = events.send(AskRunEvent::Done {
                    run_id: run_id.to_string(),
                    result,
                });
            }
        }
        Err(error) => {
            if cancel.is_cancelled() {
                let _ = events.send(AskRunEvent::Cancelled {
                    run_id: run_id.to_string(),
                });
            } else {
                let _ = events.send(AskRunEvent::Error {
                    run_id: run_id.to_string(),
                    message: error,
                });
            }
        }
    }
}

async fn ask_search_stream_inner(
    api_key: &str,
    run_id: &str,
    question: &str,
    context: Option<&str>,
    mode: &str,
    attachments: &[AskAttachment],
    pool: &sqlx::SqlitePool,
    brain_path: Option<&Path>,
    app_support_dir: Option<&Path>,
    events: tokio::sync::mpsc::UnboundedSender<AskRunEvent>,
    cancel: tokio_util::sync::CancellationToken,
    controls: crate::models::AskRunControls,
) -> Result<AskSearchResult, String> {
    let tools = json!([{ "functionDeclarations": build_tool_declarations(mode) }]);
    // Caching is keyed by (model, system_prompt, tools) — different modes
    // produce different tool sets, so they each end up with their own cache
    // entry. attachments live in `contents` and don't affect the cache key.
    let cache_name =
        crate::gemini_cache::ensure_cache(api_key, ASK_MODEL, ASK_SYSTEM_PROMPT, &tools, 3600)
            .await
            .unwrap_or(None);
    let user_prompt = ask_user_prompt(question, context);

    // Compose user parts: attachments first so the model has visual context, then text.
    let mut user_parts: Vec<serde_json::Value> = Vec::with_capacity(attachments.len() + 1);
    for attachment in attachments {
        match attachment {
            AskAttachment::Image {
                mime_type, data, ..
            } => {
                user_parts.push(json!({
                    "inlineData": { "mimeType": mime_type, "data": data }
                }));
            }
        }
    }
    user_parts.push(json!({ "text": user_prompt }));

    let mut contents: Vec<serde_json::Value> = vec![json!({
        "role": "user",
        "parts": user_parts
    })];

    // Section 6.2 — "Why this answer?". We accumulate the per-node retrieval
    // signal breakdown from any `retrieve_brain_context` tool calls inside this
    // run, then inject the most-recent snapshot into the final `AskSearchResult`
    // before returning. The model itself receives the same data through the
    // normal tool-response channel; this is purely a side-channel capture for
    // the UI's score-breakdown expander.
    let mut latest_scored_nodes: Vec<crate::models::ScoredBrainNode> = Vec::new();
    let mut latest_retrieval_query: Option<String> = None;

    for iteration in 0..10_i32 {
        if cancel.is_cancelled() {
            return Err("cancelled".to_string());
        }

        // Cap accumulating tool responses — keep only the two most recent
        // iterations full-fidelity; older ones become summary stubs.
        prune_old_tool_responses(&mut contents, 2);

        // See note above: `tool_config` must be omitted when `cachedContent`
        // is set. AUTO is the default mode.
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

        let raw_json: serde_json::Value = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err("cancelled".to_string()),
            r = post_gemini(Some(pool), "ask", ASK_MODEL, api_key, &body) => r?,
        };

        let block_reason = raw_json
            .get("promptFeedback")
            .and_then(|f| f.get("blockReason"))
            .and_then(|r| r.as_str())
            .map(str::to_string);

        let candidate = raw_json.get("candidates").and_then(|c| c.get(0)).cloned();

        let finish_reason = candidate
            .as_ref()
            .and_then(|c| c.get("finishReason"))
            .and_then(|r| r.as_str())
            .filter(|r| !r.is_empty() && *r != "STOP")
            .map(str::to_string);

        let model_content = candidate.as_ref().and_then(|c| c.get("content")).cloned();

        let parts: Vec<serde_json::Value> = model_content
            .as_ref()
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();

        // Collect text + thoughts + function calls. We don't filter thought parts
        // out of `text` — better to surface thinking-as-answer than to leave the
        // user with an empty bubble if the model returned only thought parts.
        let mut text_buf = String::new();
        let mut reasoning_buf = String::new();
        let mut function_calls: Vec<(String, serde_json::Value)> = Vec::new();
        for part in &parts {
            let is_thought = part
                .get("thought")
                .and_then(|t| t.as_bool())
                .unwrap_or(false);
            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                if is_thought {
                    reasoning_buf.push_str(text);
                } else {
                    text_buf.push_str(text);
                }
            } else if let Some(fc) = part.get("functionCall") {
                if let (Some(name), Some(args)) =
                    (fc.get("name").and_then(|n| n.as_str()), fc.get("args"))
                {
                    function_calls.push((name.to_string(), args.clone()));
                }
            }
        }

        if !reasoning_buf.is_empty() {
            let _ = events.send(AskRunEvent::ReasoningDelta {
                run_id: run_id.to_string(),
                delta: reasoning_buf.clone(),
            });
        }

        if function_calls.is_empty() {
            // Final turn — emit text and parse.
            let mut effective_text = text_buf.clone();
            if effective_text.trim().is_empty() && !reasoning_buf.trim().is_empty() {
                // Last-resort: surface thinking as the answer rather than nothing.
                effective_text = reasoning_buf.clone();
            }
            if effective_text.trim().is_empty() {
                if let Some(reason) = block_reason.or(finish_reason) {
                    return Err(format!("Gemini blocked the response ({reason})."));
                }
                return Err(format!(
                    "Gemini returned no content (parts={}). Try retrying or simplifying the question.",
                    parts.len()
                ));
            }

            // Stream the answer text in word-sized chunks so the UI feels live
            // even though the underlying request is non-streaming.
            stream_text_in_chunks(run_id, &events, &effective_text).await;

            let _ = events.send(AskRunEvent::TurnComplete {
                run_id: run_id.to_string(),
                iteration,
            });
            return parse_agentic_answer(&effective_text).map(|mut result| {
                // Section 6.2 — propagate the per-node retrieval breakdown
                // captured during this run into the final result so the
                // frontend's "Why this answer?" expander has data to render.
                if !latest_scored_nodes.is_empty() {
                    result.scored_nodes = std::mem::take(&mut latest_scored_nodes);
                }
                if latest_retrieval_query.is_some() {
                    result.retrieval_query = latest_retrieval_query.take();
                }
                result
            });
        }

        // Push assistant content verbatim so thought_signature etc. is preserved.
        if let Some(content) = model_content {
            contents.push(content);
        } else {
            contents.push(json!({ "role": "model", "parts": parts }));
        }

        // Announce + execute tool calls in parallel, with progress events.
        let mut handles = Vec::new();
        for (idx, (name, args)) in function_calls.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err("cancelled".to_string());
            }
            let call_id = format!("{run_id}-it{iteration}-{idx}");
            let label = tool_label(name, args);
            let args_preview = serde_json::to_string(args).ok().map(|s| {
                if s.len() > 240 {
                    format!("{}…", &s[..240])
                } else {
                    s
                }
            });
            let _ = events.send(AskRunEvent::ToolCallStarted {
                run_id: run_id.to_string(),
                call_id: call_id.clone(),
                tool: name.clone(),
                label,
                rationale: None,
                args_preview,
            });

            let pool_clone = pool.clone();
            let name_clone = name.clone();
            let args_clone = args.clone();
            let cancel_clone = cancel.clone();
            let brain_path_clone = brain_path.map(Path::to_path_buf);
            let api_key_clone = api_key.to_string();
            let app_support_dir_clone = app_support_dir.map(Path::to_path_buf);

            // Section 7: gate destructive tool calls behind explicit user
            // confirmation unless the run is in `auto_safe` mode or the tool
            // is on the user's per-tool allowlist.
            let category = crate::rate_limit::classify_tool(&name_clone);
            let permission_mode = controls
                .permission_mode
                .clone()
                .unwrap_or_else(|| "confirm".to_string());
            let in_allowlist = controls
                .auto_confirmed_tools
                .iter()
                .any(|t| t == &name_clone);
            let needs_confirmation = category == "destructive"
                && permission_mode != "auto_safe"
                && !in_allowlist;

            let confirmations_map = controls.confirmations.clone();
            let events_for_task = events.clone();
            let run_id_owned = run_id.to_string();
            let args_preview_for_event = serde_json::to_string(&args_clone).ok().map(|s| {
                if s.len() > 240 {
                    format!("{}…", &s[..240])
                } else {
                    s
                }
            });
            let tool_label_for_event = tool_label(&name_clone, &args_clone);
            let summary_for_event = format!("{} (destructive)", tool_label_for_event);
            let risk_reason_for_event = format!("destructive ({category})");

            handles.push(crate::runtime::spawn(async move {
                let started = std::time::Instant::now();

                // ---- Confirmation gate ----
                if needs_confirmation {
                    if let Some(map) = confirmations_map.clone() {
                        let key = format!("{run_id_owned}:{call_id}");
                        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                        if let Ok(mut guard) = map.lock() {
                            guard.insert(key.clone(), tx);
                        }
                        let _ = events_for_task.send(AskRunEvent::AwaitingConfirmation {
                            run_id: run_id_owned.clone(),
                            call_id: call_id.clone(),
                            tool: name_clone.clone(),
                            label: tool_label_for_event.clone(),
                            summary: summary_for_event.clone(),
                            args_preview: args_preview_for_event.clone(),
                            risk_reason: risk_reason_for_event.clone(),
                        });
                        let approved = tokio::select! {
                            biased;
                            _ = cancel_clone.cancelled() => {
                                if let Ok(mut guard) = map.lock() {
                                    guard.remove(&key);
                                }
                                false
                            },
                            decision = rx => decision.unwrap_or(false),
                        };
                        if !approved {
                            let reason = if cancel_clone.is_cancelled() {
                                "run cancelled before confirmation".to_string()
                            } else {
                                "user rejected".to_string()
                            };
                            let _ = events_for_task.send(AskRunEvent::ToolDenied {
                                run_id: run_id_owned.clone(),
                                call_id: call_id.clone(),
                                tool: name_clone.clone(),
                                reason: reason.clone(),
                            });
                            crate::prompt_injection_log::record(
                                &pool_clone,
                                crate::prompt_injection_log::RecordInput {
                                    source: "tool_reject",
                                    origin_kind: Some("ask"),
                                    origin_id: None,
                                    run_id: Some(&run_id_owned),
                                    call_id: Some(&call_id),
                                    tool: Some(&name_clone),
                                    action_taken: "rejected",
                                    reason: &reason,
                                    flags_json: "[]",
                                    content_excerpt: args_preview_for_event
                                        .as_deref()
                                        .unwrap_or(""),
                                    original_bytes: 0,
                                    sanitized_bytes: 0,
                                },
                            )
                            .await;
                            let latency_ms = started.elapsed().as_millis() as i64;
                            return (
                                call_id,
                                name_clone,
                                args_clone,
                                json!({ "ok": false, "error": "tool call rejected by user" }),
                                latency_ms,
                                pool_clone,
                            );
                        }
                        crate::prompt_injection_log::record(
                            &pool_clone,
                            crate::prompt_injection_log::RecordInput {
                                source: "tool_confirm",
                                origin_kind: Some("ask"),
                                origin_id: None,
                                run_id: Some(&run_id_owned),
                                call_id: Some(&call_id),
                                tool: Some(&name_clone),
                                action_taken: "confirmed",
                                reason: "user approved",
                                flags_json: "[]",
                                content_excerpt: args_preview_for_event
                                    .as_deref()
                                    .unwrap_or(""),
                                original_bytes: 0,
                                sanitized_bytes: 0,
                            },
                        )
                        .await;
                    }
                    // If `controls.confirmations` is None (eval-runner / CLI),
                    // skip the gate — the host doesn't have a UI to confirm.
                } else if category == "destructive" {
                    // Permission-mode bypass — log it so the audit panel still
                    // captures the destructive call.
                    crate::prompt_injection_log::record(
                        &pool_clone,
                        crate::prompt_injection_log::RecordInput {
                            source: "tool_confirm",
                            origin_kind: Some("ask"),
                            origin_id: None,
                            run_id: Some(&run_id_owned),
                            call_id: Some(&call_id),
                            tool: Some(&name_clone),
                            action_taken: "confirmed",
                            reason: if in_allowlist {
                                "per_tool_allowlist"
                            } else {
                                "permission_mode_auto_safe"
                            },
                            flags_json: "[]",
                            content_excerpt: args_preview_for_event
                                .as_deref()
                                .unwrap_or(""),
                            original_bytes: 0,
                            sanitized_bytes: 0,
                        },
                    )
                    .await;
                }
                // ---- end gate ----

                let value = tokio::select! {
                    biased;
                    _ = cancel_clone.cancelled() => json!({ "cancelled": true }),
                    v = dispatch_tool(&pool_clone, brain_path_clone.as_deref(), &api_key_clone, &name_clone, &args_clone, app_support_dir_clone.as_deref()) => v,
                };
                let latency_ms = started.elapsed().as_millis() as i64;
                (call_id, name_clone, args_clone, value, latency_ms, pool_clone)
            }));
        }

        let mut response_parts: Vec<serde_json::Value> = Vec::new();
        for handle in handles {
            let (call_id, name, args, result, latency_ms, pool_for_log) =
                handle.await.map_err(|e| format!("tool task failed: {e}"))?;
            let summary = summarize_tool_result(&name, &result);
            let ok = result.get("error").is_none();
            // Section 6.2 — capture retrieval scores for the "Why this answer?"
            // expander. Most-recent retrieval wins, by design.
            if name == "retrieve_brain_context" && ok {
                if let Some(inner) = result.get("result") {
                    if let Some(nodes_value) = inner.get("scored_nodes") {
                        if let Ok(parsed) = serde_json::from_value::<
                            Vec<crate::models::ScoredBrainNode>,
                        >(nodes_value.clone())
                        {
                            if !parsed.is_empty() {
                                latest_scored_nodes = parsed;
                            }
                        }
                    }
                    if let Some(q) = inner.get("query").and_then(|v| v.as_str()) {
                        if !q.is_empty() {
                            latest_retrieval_query = Some(q.to_string());
                        }
                    }
                }
            }
            let _ = events.send(AskRunEvent::ToolCallDone {
                run_id: run_id.to_string(),
                call_id: call_id.clone(),
                tool: name.clone(),
                ok,
                summary: summary.clone(),
            });
            let args_json = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
            let result_json = serde_json::to_string(&result).ok();
            let error_str = result
                .get("error")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            crate::tool_audit::record(
                &pool_for_log,
                crate::tool_audit::RecordInput {
                    source: "ask",
                    run_id: Some(run_id),
                    call_id: Some(&call_id),
                    tool: &name,
                    args_json: &args_json,
                    result_summary: Some(summary.as_str()),
                    result_json: result_json.as_deref(),
                    ok,
                    error: error_str.as_deref(),
                    latency_ms,
                },
            )
            .await;
            response_parts.push(json!({
                "functionResponse": {
                    "name": name,
                    "response": { "result": result }
                }
            }));
        }
        contents.push(json!({ "role": "user", "parts": response_parts }));

        let _ = events.send(AskRunEvent::TurnComplete {
            run_id: run_id.to_string(),
            iteration,
        });
    }

    Err("Agentic search did not converge within the iteration limit".to_string())
}

/// Dribble a complete answer to the UI as a series of `TextDelta` events. Uses
/// word-sized chunks with tiny pauses so the bubble feels alive even when the
/// upstream call is non-streaming. Cheap; runs in the same task that
/// produced the answer.
async fn stream_text_in_chunks(
    run_id: &str,
    events: &tokio::sync::mpsc::UnboundedSender<AskRunEvent>,
    text: &str,
) {
    // Find the trace-meta fence (if present) so we don't dribble metadata.
    let visible_end = text.find("```trace-meta").unwrap_or(text.len());
    let visible = &text[..visible_end];
    if visible.is_empty() {
        return;
    }

    // Split on whitespace boundaries; group ~5 words per delta so it feels
    // like typing without spamming React with too many state updates.
    const WORDS_PER_CHUNK: usize = 5;
    let mut buf = String::new();
    let mut emitted = false;
    let mut word_count = 0;
    for ch in visible.chars() {
        buf.push(ch);
        if ch.is_whitespace() {
            word_count += 1;
            if word_count >= WORDS_PER_CHUNK {
                let _ = events.send(AskRunEvent::TextDelta {
                    run_id: run_id.to_string(),
                    delta: std::mem::take(&mut buf),
                });
                word_count = 0;
                emitted = true;
                tokio::time::sleep(std::time::Duration::from_millis(18)).await;
            }
        }
    }
    if !buf.is_empty() {
        let _ = events.send(AskRunEvent::TextDelta {
            run_id: run_id.to_string(),
            delta: buf,
        });
        emitted = true;
    }
    let _ = emitted;
}

/// Replace older `functionResponse` payloads with a compact stub so the agent
/// loop's `contents` doesn't accumulate unboundedly. Cached preamble covers
/// `system_instruction + tools` but raw tool responses re-send every turn at
/// the full input rate; one long Ask was observed sending 165k prompt tokens
/// for a 23-token answer because of this.
///
/// `keep_recent` controls how many tool-response batches stay full-fidelity at
/// the tail. Pass 2 to give the model the last two iterations' raw outputs.
pub(super) fn prune_old_tool_responses(contents: &mut [serde_json::Value], keep_recent: usize) {
    // Collect indices of `user` entries that carry `functionResponse` parts,
    // oldest → newest.
    let mut indices: Vec<usize> = Vec::new();
    for (idx, entry) in contents.iter().enumerate() {
        if entry.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        let Some(parts) = entry.get("parts").and_then(|p| p.as_array()) else {
            continue;
        };
        if parts.iter().any(|p| p.get("functionResponse").is_some()) {
            indices.push(idx);
        }
    }
    if indices.len() <= keep_recent {
        return;
    }
    let cutoff = indices.len() - keep_recent;
    for &idx in &indices[..cutoff] {
        let Some(entry) = contents.get_mut(idx) else { continue };
        let Some(parts) = entry.get_mut("parts").and_then(|p| p.as_array_mut()) else {
            continue;
        };
        for part in parts.iter_mut() {
            let Some(function_response) = part.get_mut("functionResponse") else {
                continue;
            };
            let name = function_response
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("tool")
                .to_string();
            let summary = function_response
                .get("response")
                .and_then(|r| r.get("result"))
                .map(|result| summarize_tool_result(&name, result))
                .unwrap_or_else(|| "ok".to_string());
            let Some(obj) = function_response.as_object_mut() else {
                continue;
            };
            // Skip already-pruned payloads so this is idempotent across
            // iterations.
            let already_pruned = obj
                .get("response")
                .and_then(|r| r.get("result"))
                .and_then(|r| r.get("truncated"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if already_pruned {
                continue;
            }
            obj.insert(
                "response".to_string(),
                json!({
                    "result": {
                        "summary": summary,
                        "truncated": true,
                    }
                }),
            );
        }
    }
}

fn summarize_tool_result(tool: &str, result: &serde_json::Value) -> String {
    if let Some(error) = result.get("error").and_then(|e| e.as_str()) {
        return format!("error: {error}");
    }
    if let Some(items) = result.get("items").and_then(|i| i.as_array()) {
        return format!(
            "{} item{}",
            items.len(),
            if items.len() == 1 { "" } else { "s" }
        );
    }
    if let Some(arr) = result.as_array() {
        return format!("{} entries", arr.len());
    }
    if result.is_object() {
        return format!(
            "{} fields",
            result.as_object().map(|m| m.len()).unwrap_or(0)
        );
    }
    if let Some(saved) = result.get("saved").and_then(|v| v.as_bool()) {
        return if saved {
            "saved".to_string()
        } else {
            "not saved".to_string()
        };
    }
    let _ = tool;
    "ok".to_string()
}

pub(super) fn ask_user_prompt(question: &str, context: Option<&str>) -> String {
    let question = question.trim();
    match context.map(str::trim).filter(|value| !value.is_empty()) {
        Some(context) => format!(
            "Ask context for follow-up resolution, memory, and mode selection:\n{context}\n\nCurrent user message:\n{question}"
        ),
        None => question.to_string(),
    }
}

pub(super) fn tool_label(name: &str, args: &serde_json::Value) -> String {
    if name == "flag_new_deliverable" {
        return format!(
            "Flagging new candidate: \"{}\"",
            args["title"].as_str().unwrap_or("")
        );
    }
    match name {
        "get_workspace_summary" => "Reviewing workspace overview".to_string(),
        "search_deliverables" => format!(
            "Searching deliverables for \"{}\"",
            args["query"].as_str().unwrap_or("")
        ),
        "get_deliverable_detail" => "Reading deliverable details".to_string(),
        "get_deliverables_by_state" => format!(
            "Loading {} deliverables",
            args["state"].as_str().unwrap_or("all")
        ),
        "get_high_priority_deliverables" => "Ranking by priority".to_string(),
        "get_blocked_deliverables" => "Checking blocked deliverables".to_string(),
        "list_initiatives" => match args["query"].as_str() {
            Some(q) if !q.is_empty() => format!("Searching initiatives for \"{q}\""),
            _ => "Scanning all initiatives".to_string(),
        },
        "get_initiative_detail" => "Reading initiative details".to_string(),
        "search_meetings" => format!(
            "Searching meetings for \"{}\"",
            args["query"].as_str().unwrap_or("")
        ),
        "get_meeting_detail" => "Reading meeting transcript".to_string(),
        "get_stakeholders" => "Loading stakeholder overview".to_string(),
        "get_stakeholder_deliverables" => "Loading stakeholder's deliverables".to_string(),
        "search_captures" => format!(
            "Searching captures for \"{}\"",
            args["query"].as_str().unwrap_or("")
        ),
        "search_conversations" => format!(
            "Searching conversations for \"{}\"",
            args["query"].as_str().unwrap_or("")
        ),
        "get_conversation_detail" => "Reading conversation detail".to_string(),
        "get_work_graph_context" => "Reading work graph memory".to_string(),
        "retrieve_brain_context" => format!(
            "Retrieving brain graph context for \"{}\"",
            args["query"].as_str().unwrap_or("")
        ),
        "query_brain_cypher" => "Querying brain graph".to_string(),
        "run_brain_template" => format!(
            "Running brain template {}",
            args["template"].as_str().unwrap_or("")
        ),
        "get_daily_brain_brief" => "Generating daily brain brief".to_string(),
        "record_brain_feedback" => "Recording brain feedback".to_string(),
        "record_brain_learning_event" => "Recording brain learning signal".to_string(),
        "get_brain_learning_snapshot" => "Inspecting brain learning state".to_string(),
        "retrieve_memory" => format!(
            "Retrieving durable memory for \"{}\"",
            args["query"].as_str().unwrap_or("")
        ),
        "save_memory" => format!("Saving memory: {}", args["title"].as_str().unwrap_or("")),
        "ask_user_question" => "Asking for clarification".to_string(),
        "search_email_threads" => {
            let q = args["query"].as_str().unwrap_or("");
            let cat = args.get("category").and_then(|v| v.as_str());
            match (q.is_empty(), cat) {
                (true, Some(c)) => format!("Searching email in category \"{c}\""),
                (false, Some(c)) => format!("Searching email for \"{q}\" in \"{c}\""),
                _ => format!("Searching email for \"{q}\""),
            }
        }
        "get_email_category_summary" => "Loading email category counts".to_string(),
        "get_email_thread" => "Reading email thread".to_string(),
        "create_deliverable_from_email" => {
            format!(
                "Creating deliverable from email: {}",
                args["title"].as_str().unwrap_or("")
            )
        }
        "link_email_thread_to_deliverable" => "Linking email to deliverable".to_string(),
        "link_email_thread_to_initiative" => "Linking email to initiative".to_string(),
        "capture_email_thread" => "Saving email to capture inbox".to_string(),
        "get_current_week" => "Checking week plan".to_string(),
        "get_recent_activity" => "Reviewing recent activity".to_string(),
        "add_deliverable_note" => "Adding note to deliverable".to_string(),
        "add_initiative_note" => "Adding note to initiative".to_string(),
        "create_capture" => "Saving to capture inbox".to_string(),
        "update_deliverable_state" => format!(
            "Marking deliverable as {}",
            args["state"].as_str().unwrap_or("")
        ),
        "set_deliverable_focus" => {
            if args["focused"].as_bool().unwrap_or(true) {
                "Setting as focused deliverable".to_string()
            } else {
                "Clearing focus".to_string()
            }
        }
        "list_pending_tasks" => "Reading pending tasks".to_string(),
        "add_deliverable_task" => format!("Adding task: {}", args["title"].as_str().unwrap_or("")),
        "update_task_status" => {
            format!("Marking task as {}", args["status"].as_str().unwrap_or(""))
        }
        "update_deliverable_metadata" => "Updating deliverable metadata".to_string(),
        "search_files" => format!(
            "Searching files for \"{}\"",
            args["query"].as_str().unwrap_or("")
        ),
        "list_files_for_entity" => format!(
            "Loading files for {} {}",
            args["entity_kind"].as_str().unwrap_or("entity"),
            args["entity_id"].as_str().unwrap_or("")
        ),
        "get_file_detail" => "Reading file details".to_string(),
        "search_web" => format!(
            "Searching the web for \"{}\"",
            args["query"].as_str().unwrap_or("")
        ),
        "fetch_url" => format!("Fetching {}", args["url"].as_str().unwrap_or("URL")),
        other => format!("Calling {other}"),
    }
}


// ── Shared agentic loop ────────────────────────────────────────────────────────

pub(super) async fn run_agentic_loop(
    feature: &str,
    api_key: &str,
    system_prompt: &str,
    initial_contents: Vec<serde_json::Value>,
    tool_declarations: serde_json::Value,
    pool: &sqlx::SqlitePool,
    progress: Option<tokio::sync::mpsc::UnboundedSender<crate::models::AskProgressEvent>>,
) -> Result<String, String> {
    let tools = json!([{ "functionDeclarations": tool_declarations }]);
    let mut contents = initial_contents;

    // Try the explicit-cache path. ensure_cache returns None if Gemini
    // rejects the cache (e.g. content under the minimum token count) — we
    // fall back to inline systemInstruction + tools in that case.
    let cache_name =
        crate::gemini_cache::ensure_cache(api_key, ASK_MODEL, system_prompt, &tools, 3600)
            .await
            .unwrap_or(None);

    for _ in 0..14u32 {
        // Cap accumulating tool responses — keep only the two most recent
        // iterations full-fidelity; older ones become summary stubs.
        prune_old_tool_responses(&mut contents, 2);

        // Same caching constraint: omit `tool_config` on the cached path.
        let body = if let Some(name) = cache_name.as_ref() {
            json!({
                "cachedContent": name,
                "contents": contents,
                "generationConfig": { "temperature": 0.2 }
            })
        } else {
            json!({
                "systemInstruction": { "parts": [{ "text": system_prompt }] },
                "tools": tools,
                "toolConfig": { "functionCallingConfig": { "mode": "AUTO" } },
                "contents": contents,
                "generationConfig": { "temperature": 0.2 }
            })
        };

        let raw_json = post_gemini_external(Some(pool), feature, ASK_MODEL, api_key, &body).await?;

        let model_content = raw_json
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .cloned()
            .ok_or_else(|| "No candidates in Gemini response".to_string())?;

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
            return Ok(raw);
        }

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

        contents.push(model_content);

        let mut response_parts: Vec<serde_json::Value> = Vec::new();
        for part in &fn_calls {
            if let Some(fc) = &part.function_call {
                let result = dispatch_tool(pool, None, api_key, &fc.name, &fc.args, None).await;
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

    Err("Agentic loop did not converge within iteration limit".to_string())
}

// ── Minutes tool declarations ──────────────────────────────────────────────────


