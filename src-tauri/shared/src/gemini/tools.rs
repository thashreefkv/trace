//! Gemini agentic tool surface.
//!
//! Three concerns live here:
//! 1. **Tool schema declarations** — `build_tool_declarations` (the full Ask
//!    surface, gated by mode), `build_base_tool_declarations` (the source of
//!    truth for every tool's JSON schema), plus the minutes-mode and
//!    digest-mode subset builders that filter the base set.
//! 2. **Dispatch** — `dispatch_tool` is the single match-on-name that converts
//!    a `(name, args)` pair into a concrete call against `repo`, `brain`,
//!    `files`, `gmail`, `google_*`, `capture_promotion`, etc.
//! 3. **Side tools** — `tool_search_web` (Google Search grounding) and
//!    `tool_fetch_url` (clean fetch with prompt-safety sanitization) are
//!    research-mode helpers that go through their own circuit-breaker bucket.
//!
//! Functions callable from sibling submodules (`legacy`, eventually `streaming`
//! and `ask`) are `pub(super)`; everything else is private to this module.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::Path,
};

use futures_util::StreamExt;
use serde_json::json;
use url::{Host, Url};

use super::client::post_gemini_external;

pub(super) async fn dispatch_tool(
    pool: &sqlx::SqlitePool,
    brain_path: Option<&Path>,
    api_key: &str,
    name: &str,
    args: &serde_json::Value,
    app_support_dir: Option<&Path>,
) -> serde_json::Value {
    use crate::{brain, repo};

    // Rate limit: a per-mode bucket plus a tighter per-category bucket so
    // runaway loops cannot hammer destructive paths.
    let limiter = crate::rate_limit::app_limiter();
    if let Err(error) = limiter.try_acquire("ask", 1.0) {
        return json!({
            "ok": false,
            "error": format!("rate limited: retry in {}s", error.retry_after_secs),
            "retry_after_secs": error.retry_after_secs,
        });
    }
    let category = crate::rate_limit::classify_tool(name);
    if category != "read" {
        if let Err(error) = limiter.try_acquire(category, 1.0) {
            return json!({
                "ok": false,
                "error": format!("rate limited on {} ops: retry in {}s", category, error.retry_after_secs),
                "retry_after_secs": error.retry_after_secs,
            });
        }
    }

    let result = match name {
        "search_deliverables" => {
            let query = args["query"].as_str().unwrap_or("");
            let state = args["state"].as_str();
            repo::tool_search_deliverables(pool, query, state).await
        }
        "get_deliverable_detail" => {
            let id = args["id"].as_str().unwrap_or("");
            repo::tool_get_deliverable_detail(pool, id).await
        }
        "list_initiatives" => {
            let query = args["query"].as_str();
            repo::tool_list_initiatives(pool, query).await
        }
        "get_initiative_detail" => {
            let id = args["id"].as_str().unwrap_or("");
            repo::tool_get_initiative_detail(pool, id).await
        }
        "search_meetings" => {
            let query = args["query"].as_str().unwrap_or("");
            repo::tool_search_meetings(pool, query).await
        }
        "get_meeting_detail" => {
            let id = args["id"].as_str().unwrap_or("");
            repo::tool_get_meeting_detail(pool, id).await
        }
        "get_stakeholders" => repo::tool_get_stakeholders(pool).await,
        "get_stakeholder_deliverables" => {
            let id = args["stakeholder_id"].as_str().unwrap_or("");
            repo::tool_get_stakeholder_deliverables(pool, id).await
        }
        "search_captures" => {
            let query = args["query"].as_str().unwrap_or("");
            repo::tool_search_captures(pool, query).await
        }
        "search_conversations" => {
            let query = args["query"].as_str().unwrap_or("");
            repo::tool_search_conversations(pool, query).await
        }
        "get_conversation_detail" => {
            let id = args["id"].as_str().unwrap_or("");
            repo::tool_get_conversation_detail(pool, id).await
        }
        "get_work_graph_context" => {
            if let Some(path) = brain_path {
                match brain::get_brain_graph(pool, path, Default::default()).await {
                    Ok(graph) => json!({
                        "ok": true,
                        "summary": graph.ai_context,
                        "node_count": graph.nodes.len(),
                        "edge_count": graph.edges.len(),
                        "graph": graph,
                    }),
                    Err(error) => json!({ "ok": false, "error": error }),
                }
            } else {
                repo::tool_get_work_graph_context(pool).await
            }
        }
        "retrieve_brain_context" => {
            let query = args["query"].as_str().unwrap_or("");
            let focus_entity_id = args
                .get("focus_entity_id")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let max_hops = args
                .get("max_hops")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize);
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize);
            if let Some(path) = brain_path {
                brain::tool_retrieve_brain_context(
                    pool,
                    path,
                    query,
                    focus_entity_id,
                    max_hops,
                    limit,
                )
                .await
            } else {
                json!({ "ok": false, "error": "brain graph is not available in this context" })
            }
        }
        "query_brain_cypher" => {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let params = args.get("params").filter(|value| !value.is_null()).cloned();
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize);
            if let Some(path) = brain_path {
                if !path.exists() {
                    let _ = brain::rebuild_brain(pool, path).await;
                }
                brain::tool_query_brain_cypher(
                    path,
                    crate::models::BrainCypherInput {
                        query,
                        params,
                        limit,
                    },
                )
                .await
            } else {
                json!({ "ok": false, "error": "brain graph is not available in this context" })
            }
        }
        "run_brain_template" => {
            if let Some(path) = brain_path {
                match serde_json::from_value::<crate::models::BrainTemplateInput>(args.clone()) {
                    Ok(input) => brain::tool_run_brain_template(pool, path, input).await,
                    Err(error) => {
                        json!({ "ok": false, "error": format!("invalid brain template input: {error}") })
                    }
                }
            } else {
                json!({ "ok": false, "error": "brain graph is not available in this context" })
            }
        }
        "get_daily_brain_brief" => {
            if let Some(path) = brain_path {
                brain::tool_get_daily_brain_brief(pool, path).await
            } else {
                json!({ "ok": false, "error": "brain graph is not available in this context" })
            }
        }
        "record_brain_feedback" => {
            match serde_json::from_value::<crate::models::BrainFeedbackInput>(args.clone()) {
                Ok(input) => match brain::record_brain_feedback(pool, input).await {
                    Ok(()) => json!({ "ok": true }),
                    Err(error) => json!({ "ok": false, "error": error }),
                },
                Err(error) => {
                    json!({ "ok": false, "error": format!("invalid brain feedback input: {error}") })
                }
            }
        }
        "record_brain_learning_event" => {
            match serde_json::from_value::<crate::models::BrainLearningEventInput>(args.clone()) {
                Ok(input) => brain::tool_record_brain_learning_event(pool, input).await,
                Err(error) => {
                    json!({ "ok": false, "error": format!("invalid brain learning event input: {error}") })
                }
            }
        }
        "get_brain_learning_snapshot" => {
            let template = args
                .get("template")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let limit = args
                .get("limit")
                .and_then(|value| value.as_u64())
                .map(|value| value as usize);
            brain::tool_get_brain_learning_snapshot(pool, template, limit).await
        }
        "retrieve_memory" => {
            let query = args["query"].as_str().unwrap_or("");
            repo::tool_retrieve_memory(pool, query).await
        }
        "save_memory" => {
            let kind = args["kind"].as_str().unwrap_or("semantic");
            let title = args["title"].as_str().unwrap_or("");
            let body = args["body"].as_str().unwrap_or("");
            let tags = args["tags"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            repo::tool_save_memory(pool, kind, title, body, &tags).await
        }
        "ask_user_question" => json!({
            "status": "awaiting_user_response",
            "instruction": "Return this question in the final JSON questions array and wait for the user's follow-up before taking action.",
            "question": args
        }),
        "search_email_threads" => {
            let query = args["query"].as_str().unwrap_or("");
            let category = args.get("category").and_then(|v| v.as_str());
            let limit = args.get("limit").and_then(|v| v.as_i64());
            repo::tool_search_email_threads(pool, query, category, limit).await
        }
        "get_email_category_summary" => repo::tool_get_email_category_summary(pool).await,
        "get_email_thread" => {
            let thread_id = args["thread_id"].as_str().unwrap_or("");
            repo::tool_get_email_thread(pool, thread_id).await
        }
        "create_deliverable_from_email" => {
            let thread_id = args["thread_id"].as_str().unwrap_or("");
            let title = args["title"].as_str().unwrap_or("");
            let claim = args["claim"].as_str().unwrap_or("");
            let deliverable_type = args["type"].as_str().unwrap_or("email");
            let initiative_id = args.get("initiative_id").and_then(|v| v.as_str());
            repo::tool_create_deliverable_from_email(
                pool,
                thread_id,
                title,
                claim,
                deliverable_type,
                initiative_id,
            )
            .await
        }
        "link_email_thread_to_deliverable" => {
            let thread_id = args["thread_id"].as_str().unwrap_or("");
            let deliverable_id = args["deliverable_id"].as_str().unwrap_or("");
            repo::tool_link_email_thread_to_deliverable(pool, thread_id, deliverable_id).await
        }
        "link_email_thread_to_initiative" => {
            let thread_id = args["thread_id"].as_str().unwrap_or("");
            let initiative_id = args["initiative_id"].as_str().unwrap_or("");
            repo::tool_link_email_thread_to_initiative(pool, thread_id, initiative_id).await
        }
        "capture_email_thread" => {
            let thread_id = args["thread_id"].as_str().unwrap_or("");
            repo::tool_capture_email_thread(pool, thread_id).await
        }
        "get_blocked_deliverables" => repo::tool_get_blocked_deliverables(pool).await,
        "get_deliverables_by_state" => {
            let state = args["state"].as_str().unwrap_or("drafting");
            repo::tool_get_deliverables_by_state(pool, state).await
        }
        "get_high_priority_deliverables" => repo::tool_get_high_priority_deliverables(pool).await,
        "get_current_week" => repo::tool_get_current_week(pool).await,
        "get_recent_activity" => repo::tool_get_recent_activity(pool).await,
        "get_workspace_summary" => repo::tool_get_workspace_summary(pool).await,

        // ── Write tools ──────────────────────────────────────────────────────
        "add_deliverable_note" => {
            let id = args["deliverable_id"].as_str().unwrap_or("");
            let body = args["body"].as_str().unwrap_or("");
            repo::tool_add_deliverable_note(pool, id, body).await
        }
        "add_initiative_note" => {
            let id = args["initiative_id"].as_str().unwrap_or("");
            let body = args["body"].as_str().unwrap_or("");
            repo::tool_add_initiative_note(pool, id, body).await
        }
        "create_capture" => {
            let body = args["body"].as_str().unwrap_or("");
            repo::tool_create_capture(pool, body).await
        }
        "update_deliverable_state" => {
            let id = args["id"].as_str().unwrap_or("");
            let state = args["state"].as_str().unwrap_or("");
            repo::tool_update_deliverable_state(pool, id, state).await
        }
        "set_deliverable_focus" => {
            let id = args["id"].as_str().unwrap_or("");
            let focused = args["focused"].as_bool().unwrap_or(true);
            repo::tool_set_deliverable_focus(pool, id, focused).await
        }
        "list_pending_tasks" => repo::tool_list_pending_tasks(pool).await,
        "add_deliverable_task" => {
            let deliverable_id = args["deliverable_id"].as_str().unwrap_or("");
            let title = args["title"].as_str().unwrap_or("");
            let due_date = args["due_date"].as_str();
            let notes = args["notes"].as_str();
            let url = args["url"].as_str();
            repo::tool_add_deliverable_task(pool, deliverable_id, title, due_date, notes, url).await
        }
        "update_task_status" => {
            let task_id = args["task_id"].as_str().unwrap_or("");
            let status = args["status"].as_str().unwrap_or("");
            repo::tool_update_task_status(pool, task_id, status).await
        }
        "update_deliverable_metadata" => {
            let id = args["id"].as_str().unwrap_or("");
            let deadline = args.get("deadline").and_then(|v| v.as_str());
            let effort = args.get("effort").and_then(|v| v.as_i64());
            let impact = args.get("impact").and_then(|v| v.as_i64());
            let blocker = args.get("blocker_reason").and_then(|v| v.as_str());
            repo::tool_update_deliverable_metadata(pool, id, deadline, effort, impact, blocker)
                .await
        }

        "flag_new_deliverable" => {
            let title = args["title"].as_str().unwrap_or("");
            let claim = args["claim"].as_str().unwrap_or("");
            let suggested_type = args["suggested_type"].as_str().unwrap_or("");
            let suggested_initiative = args["suggested_initiative"].as_str().unwrap_or("");
            repo::tool_flag_new_deliverable(
                pool,
                title,
                claim,
                suggested_type,
                suggested_initiative,
            )
            .await
        }

        "search_files" => {
            let query = args["query"].as_str().unwrap_or("");
            crate::files::tool_search_files_semantic(pool, api_key, query).await
        }
        "list_files_for_entity" => {
            let entity_kind = args["entity_kind"].as_str().unwrap_or("");
            let entity_id = args["entity_id"].as_str().unwrap_or("");
            crate::files::tool_list_files_for_entity(pool, entity_kind, entity_id).await
        }
        "get_file_detail" => {
            let file_id = args["file_id"].as_str().unwrap_or("");
            crate::files::tool_get_file_detail(pool, file_id).await
        }

        // ── Calendar read tools ──────────────────────────────────────────────
        "get_calendar_events" => {
            let date = args["date"].as_str().unwrap_or("");
            repo::tool_get_calendar_events(pool, date).await
        }
        "get_calendar_week" => {
            let week_start = args["week_start"].as_str().unwrap_or("");
            repo::tool_get_calendar_week(pool, week_start).await
        }
        "get_upcoming_events" => {
            let days = args.get("days").and_then(|v| v.as_i64()).unwrap_or(7);
            repo::tool_get_upcoming_events(pool, days).await
        }
        "search_calendar_events" => {
            let query = args["query"].as_str().unwrap_or("");
            repo::tool_search_calendar_events(pool, query).await
        }
        "find_free_slots" => {
            let date = args["date"].as_str().unwrap_or("");
            repo::tool_find_free_slots(pool, date).await
        }

        // ── Calendar write tools ─────────────────────────────────────────────
        "create_calendar_event" => {
            if let Some(dir) = app_support_dir {
                let title = args["title"].as_str().unwrap_or("");
                let date = args["date"].as_str().unwrap_or("");
                let description = args.get("description").and_then(|v| v.as_str());
                let start_time = args.get("start_time").and_then(|v| v.as_str());
                let end_time = args.get("end_time").and_then(|v| v.as_str());
                let time_zone = args.get("time_zone").and_then(|v| v.as_str());
                let attendees = args.get("attendees").and_then(|v| v.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                });
                crate::google_calendar::tool_create_calendar_event(
                    pool,
                    dir,
                    title,
                    description,
                    date,
                    start_time,
                    end_time,
                    time_zone,
                    attendees,
                )
                .await
            } else {
                json!({ "ok": false, "error": "calendar not available in this context" })
            }
        }
        "update_calendar_event" => {
            if let Some(dir) = app_support_dir {
                let event_id = args["event_id"].as_str().unwrap_or("");
                let title = args.get("title").and_then(|v| v.as_str());
                let description = args.get("description").and_then(|v| v.as_str());
                let date = args.get("date").and_then(|v| v.as_str());
                let start_time = args.get("start_time").and_then(|v| v.as_str());
                let end_time = args.get("end_time").and_then(|v| v.as_str());
                crate::google_calendar::tool_update_calendar_event(
                    pool,
                    dir,
                    event_id,
                    title,
                    description,
                    date,
                    start_time,
                    end_time,
                )
                .await
            } else {
                json!({ "ok": false, "error": "calendar not available in this context" })
            }
        }
        "delete_calendar_event" => {
            if let Some(dir) = app_support_dir {
                let event_id = args["event_id"].as_str().unwrap_or("");
                crate::google_calendar::tool_delete_calendar_event(pool, dir, event_id).await
            } else {
                json!({ "ok": false, "error": "calendar not available in this context" })
            }
        }

        // ── Web tools (research mode only) ───────────────────────────────────
        "search_web" => {
            let query = args["query"].as_str().unwrap_or("");
            tool_search_web(pool, api_key, query).await
        }
        "fetch_url" => {
            let url = args["url"].as_str().unwrap_or("");
            let extract_what = args
                .get("extract_what")
                .and_then(|v| v.as_str())
                .unwrap_or("key information from this page");
            tool_fetch_url(Some(pool), url, extract_what).await
        }

        _ => json!({ "error": format!("Unknown tool: {name}") }),
    };

    if is_brain_mutating_tool(name) {
        if let Some(path) = brain_path {
            if result
                .get("ok")
                .and_then(|value| value.as_bool())
                .unwrap_or(true)
            {
                // Fire-and-forget with coalescing — see `brain::request_rebuild`.
                // The previous implementation awaited the rebuild inline,
                // which added 30-90s per brain-mutating tool to Ask turns.
                brain::request_rebuild(pool.clone(), path.to_path_buf());
            }
        }
    }

    result
}

fn is_brain_mutating_tool(name: &str) -> bool {
    matches!(
        name,
        "save_memory"
            | "create_deliverable_from_email"
            | "link_email_thread_to_deliverable"
            | "link_email_thread_to_initiative"
            | "capture_email_thread"
            | "add_deliverable_note"
            | "add_initiative_note"
            | "create_capture"
            | "update_deliverable_state"
            | "set_deliverable_focus"
            | "add_deliverable_task"
            | "update_task_status"
            | "update_deliverable_metadata"
            | "flag_new_deliverable"
            | "create_calendar_event"
            | "update_calendar_event"
            | "delete_calendar_event"
            | "record_brain_feedback"
            | "record_brain_learning_event"
    )
}

// ── Web tools (research mode) ─────────────────────────────────────────────────

/// Model used for the search-grounding sub-call. Must support `googleSearch`.
const WEB_SEARCH_MODEL: &str = "gemini-3-flash-preview";

/// Use Gemini's native Google Search grounding to answer a web query.
/// Returns the grounded answer text plus the web sources Gemini cited.
async fn tool_search_web(pool: &sqlx::SqlitePool, api_key: &str, query: &str) -> serde_json::Value {
    if query.trim().is_empty() {
        return json!({ "error": "query must not be empty" });
    }

    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [{
                "text": format!(
                    "Search for information about: {query}\n\nReturn a comprehensive, factual answer. Include key facts, numbers, dates, and specifics relevant to the query. Cite your sources."
                )
            }]
        }],
        "tools": [{ "google_search": {} }],
        "generationConfig": { "temperature": 0.1 }
    });

    // Preserve the legacy "graceful error in JSON" contract by mapping any
    // failure from the chokepoint into the JSON envelope the agentic loop
    // expects, rather than propagating an Err.
    let raw = match post_gemini_external(Some(pool), "web_search", WEB_SEARCH_MODEL, api_key, &body)
        .await
    {
        Ok(v) => v,
        Err(message) => return json!({ "error": message }),
    };

    let candidate = raw.get("candidates").and_then(|c| c.get(0));

    let answer_text = candidate
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .and_then(|arr| {
            arr.iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .reduce(|a, b| if a.len() >= b.len() { a } else { b })
        })
        .unwrap_or("")
        .to_string();

    let sources: Vec<serde_json::Value> = candidate
        .and_then(|c| c.get("groundingMetadata"))
        .and_then(|m| m.get("groundingChunks"))
        .and_then(|chunks| chunks.as_array())
        .map(|chunks| {
            chunks
                .iter()
                .filter_map(|chunk| {
                    let web = chunk.get("web")?;
                    let title = web.get("title").and_then(|t| t.as_str()).unwrap_or("");
                    let url = web.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                    if url.is_empty() {
                        return None;
                    }
                    Some(json!({ "title": title, "url": url }))
                })
                .collect()
        })
        .unwrap_or_default();

    if answer_text.is_empty() {
        return json!({
            "error": "Web search returned no content",
            "query": query
        });
    }

    json!({
        "query": query,
        "answer": answer_text,
        "sources": sources
    })
}

/// Fetch a public URL and return its readable text content.
/// HTML is stripped to plain text via the shared prompt-safety sanitizer, content is
/// truncated, and the result is wrapped in `<web_content>` provenance tags so
/// the model treats it as data rather than instructions.
async fn tool_fetch_url(
    pool: Option<&sqlx::SqlitePool>,
    url: &str,
    extract_what: &str,
) -> serde_json::Value {
    const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
    const MAX_REDIRECTS: usize = 5;
    let url = url.trim();

    if url.is_empty() {
        return json!({ "error": "url must not be empty" });
    }
    let mut current_url = match validate_public_url(url) {
        Ok(url) => url,
        Err(error) => return json!({ "error": error }),
    };

    let mut redirect_count = 0_usize;
    let response = loop {
        let port = match current_url.port_or_known_default() {
            Some(port) => port,
            None => return json!({ "error": "URL uses an unsupported port" }),
        };
        let (host, is_domain, resolved): (String, bool, Vec<SocketAddr>) = match current_url.host()
        {
            Some(Host::Domain(host)) => {
                let host = host.to_string();
                let addresses = match tokio::net::lookup_host((host.as_str(), port)).await {
                    Ok(addresses) => addresses.collect(),
                    Err(_) => return json!({ "error": "URL host could not be resolved" }),
                };
                (host, true, addresses)
            }
            Some(Host::Ipv4(ip)) => (
                ip.to_string(),
                false,
                vec![SocketAddr::new(IpAddr::V4(ip), port)],
            ),
            Some(Host::Ipv6(ip)) => (
                ip.to_string(),
                false,
                vec![SocketAddr::new(IpAddr::V6(ip), port)],
            ),
            None => return json!({ "error": "URL must include a public host" }),
        };
        if resolved.is_empty() || resolved.iter().any(|address| !is_public_ip(address.ip())) {
            return json!({ "error": "URL resolves to a private or reserved network" });
        }

        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (compatible; Trace/1.0)");
        if is_domain {
            builder = builder.resolve(&host, resolved[0]);
        }
        let client = match builder.build() {
            Err(_) => return json!({ "error": "HTTP client could not be created" }),
            Ok(client) => client,
        };
        let response = match client.get(current_url.clone()).send().await {
            Err(_) => return json!({ "error": "URL fetch failed" }),
            Ok(response) => response,
        };

        if response.status().is_redirection() {
            if redirect_count >= MAX_REDIRECTS {
                return json!({ "error": "URL redirect limit exceeded" });
            }
            redirect_count += 1;
            let Some(location) = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
            else {
                return json!({ "error": "URL returned an invalid redirect" });
            };
            let next_url = match current_url
                .join(location)
                .map_err(|_| "URL returned an invalid redirect".to_string())
                .and_then(|url| validate_public_url(url.as_str()))
            {
                Ok(url) => url,
                Err(error) => return json!({ "error": error }),
            };
            if current_url.scheme() == "https" && next_url.scheme() != "https" {
                return json!({ "error": "HTTPS redirects may not downgrade to HTTP" });
            }
            current_url = next_url;
            continue;
        }
        break response;
    };

    let status = response.status().as_u16();
    if !response.status().is_success() {
        return json!({ "error": format!("HTTP {status}"), "url": current_url.as_str() });
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let allowed_content_type = content_type.starts_with("text/")
        || content_type.starts_with("application/json")
        || content_type.starts_with("application/xml")
        || content_type.starts_with("application/xhtml+xml");
    if !allowed_content_type {
        return json!({ "error": "URL did not return readable text content" });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return json!({ "error": "URL response is too large" });
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(_) => return json!({ "error": "Failed to read URL response" }),
        };
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return json!({ "error": "URL response is too large" });
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8_lossy(&bytes).into_owned();

    let sanitized = crate::prompt_safety::sanitize_web_content(&body);
    let fetched_at = chrono::Utc::now().to_rfc3339();
    let wrapped =
        crate::prompt_safety::wrap_web_content(current_url.as_str(), &fetched_at, &sanitized);

    crate::prompt_safety::log_if_noteworthy(
        pool,
        "web",
        "fetch_url",
        Some(current_url.as_str()),
        None,
        &body,
        &sanitized,
    )
    .await;

    json!({
        "url": current_url.as_str(),
        "extract_what": extract_what,
        "content_type": content_type,
        "content": wrapped,
        "truncated": sanitized.truncated
    })
}

fn validate_public_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "URL is invalid".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("URL must use http:// or https://".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("URL credentials are not allowed".to_string());
    }
    match url.host() {
        Some(Host::Domain(host)) => {
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            if host == "localhost"
                || host.ends_with(".localhost")
                || host.ends_with(".local")
                || host.ends_with(".internal")
            {
                return Err("URL must use a public host".to_string());
            }
        }
        Some(Host::Ipv4(ip)) if !is_public_ipv4(ip) => {
            return Err("URL must use a public network address".to_string());
        }
        Some(Host::Ipv6(ip)) if !is_public_ipv6(ip) => {
            return Err("URL must use a public network address".to_string());
        }
        Some(_) => {}
        None => return Err("URL must include a public host".to_string()),
    }
    Ok(url)
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 168)
        || (a == 198 && (18..=19).contains(&b))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    if ip.is_unspecified() || ip.is_loopback() || ip.is_multicast() {
        return false;
    }
    let segments = ip.segments();
    if segments[0] & 0xfe00 == 0xfc00
        || segments[0] & 0xffc0 == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
    {
        return false;
    }
    if segments[..5] == [0, 0, 0, 0, 0] && segments[5] == 0xffff {
        return is_public_ipv4(Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ));
    }
    true
}

/// Strip HTML tags and return readable plain text.
/// Skips `<script>` and `<style>` blocks entirely.
/// Injects newlines at block-level elements for readability.
fn strip_html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut tag_buf = String::new();
    let mut skip_block = false; // inside <script> or <style>

    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag_buf.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let tag = tag_buf.trim().to_lowercase();
                let tag_name = tag
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                match tag_name {
                    "script" | "style" => skip_block = !tag.starts_with('/'),
                    "/script" | "/style" => skip_block = false,
                    "p" | "div" | "br" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "tr"
                    | "blockquote" | "article" | "section" | "header" | "footer" | "nav"
                    | "aside" => {
                        if !skip_block && !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            _ if in_tag => tag_buf.push(ch),
            _ if !skip_block => out.push(ch),
            _ => {}
        }
    }

    // Decode common HTML entities
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ");

    // Collapse runs of whitespace / blank lines
    let mut result = String::with_capacity(out.len());
    let mut prev_newline = false;
    let mut blank_lines = 0u8;
    for ch in out.chars() {
        if ch == '\n' || ch == '\r' {
            if prev_newline {
                blank_lines += 1;
                if blank_lines < 2 {
                    result.push('\n');
                }
            } else {
                result.push('\n');
                prev_newline = true;
                blank_lines = 0;
            }
        } else if ch == ' ' || ch == '\t' {
            if !result.ends_with(' ') && !result.ends_with('\n') {
                result.push(' ');
            }
            prev_newline = false;
        } else {
            result.push(ch);
            prev_newline = false;
            blank_lines = 0;
        }
    }

    result.trim().to_string()
}

pub(super) fn build_tool_declarations(mode: &str) -> serde_json::Value {
    let mut base = build_base_tool_declarations();
    if mode == "research" {
        if let Some(arr) = base.as_array_mut() {
            arr.push(json!({
                "name": "search_web",
                "description": "Search the public web using Google Search and return a synthesized answer with cited sources. Use in research mode for current events, public documentation, technical references, pricing, company info, or anything not in the workspace. Prefer workspace tools first; use this for information that cannot come from the workspace.",
                "parameters": {
                    "type": "OBJECT",
                    "properties": {
                        "query": {
                            "type": "STRING",
                            "description": "The search query. Be specific — include names, product names, version numbers, and relevant context to get precise results."
                        }
                    },
                    "required": ["query"]
                }
            }));
            arr.push(json!({
                "name": "fetch_url",
                "description": "Fetch and extract readable content from a public URL — documentation pages, articles, GitHub READMEs, release notes, pricing pages, etc. Do NOT use for authenticated or private URLs (Google Docs, Confluence, internal tools). Use search_web first to find the URL, then fetch_url to read the full content.",
                "parameters": {
                    "type": "OBJECT",
                    "properties": {
                        "url": {
                            "type": "STRING",
                            "description": "The fully-qualified public URL to fetch (must start with https:// or http://)"
                        },
                        "extract_what": {
                            "type": "STRING",
                            "description": "What you want to extract or understand from the page (e.g. 'pricing tiers', 'API rate limits', 'installation steps')"
                        }
                    },
                    "required": ["url", "extract_what"]
                }
            }));
        }
    }
    base
}

fn build_base_tool_declarations() -> serde_json::Value {
    json!([
        {
            "name": "get_workspace_summary",
            "description": "Get a high-level count summary of the workspace: live initiatives, deliverables by state, recent shipments, blocked items, and inbox captures. Use this first to orient before drilling down.",
            "parameters": { "type": "OBJECT", "properties": {} }
        },
        {
            "name": "search_deliverables",
            "description": "Full-text search for deliverables (work items) by keyword. Returns id, title, state, claim, deadline, and blocker_reason. Use this to find deliverables related to a topic, person, or initiative.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Search keywords" },
                    "state": {
                        "type": "STRING",
                        "description": "Optional state filter",
                        "enum": ["drafting", "in_review", "shipped", "killed"]
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_deliverable_detail",
            "description": "Get complete information for a specific deliverable by ID: full metadata, all tasks (todo/doing/done), all notes, linked stakeholders and initiatives. Use this after finding a relevant deliverable via search.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "id": { "type": "STRING", "description": "Deliverable ID" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "get_deliverables_by_state",
            "description": "List all deliverables in a given state. Useful for 'what's in review?', 'what shipped recently?', 'what's being drafted?'",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "state": {
                        "type": "STRING",
                        "description": "State to filter by",
                        "enum": ["drafting", "in_review", "shipped", "killed"]
                    }
                },
                "required": ["state"]
            }
        },
        {
            "name": "get_high_priority_deliverables",
            "description": "Get in-flight deliverables ranked by priority (focused first, then by impact/effort score and deadline). Use for 'what should I work on?', 'what are the most important things right now?'",
            "parameters": { "type": "OBJECT", "properties": {} }
        },
        {
            "name": "get_blocked_deliverables",
            "description": "Get all deliverables that have a blocker set. Use for 'what's blocked?', 'what's stuck?', 'what needs unblocking?'",
            "parameters": { "type": "OBJECT", "properties": {} }
        },
        {
            "name": "list_initiatives",
            "description": "List all strategic initiatives with their framing and status. Optionally filter by keyword. Use to understand the strategic landscape or find a specific initiative.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Optional keyword to filter by title or framing" }
                }
            }
        },
        {
            "name": "get_initiative_detail",
            "description": "Get complete details for a specific initiative by ID: framing, status, all linked deliverables, and notes. Use after finding a relevant initiative via list_initiatives.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "id": { "type": "STRING", "description": "Initiative ID" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "search_meetings",
            "description": "Search meetings by keyword across title, summary, and transcript. Returns id, title, date, and summary snippet. Use for 'what was discussed about X?', 'when did we meet about Y?'",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Search keywords" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_meeting_detail",
            "description": "Get the full record for a specific meeting by ID, including complete transcript, summary, key decisions, and action items. Use after finding a relevant meeting via search_meetings.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "id": { "type": "STRING", "description": "Meeting ID" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "get_stakeholders",
            "description": "Get all stakeholders with their roles, notes, and delivery stats (total deliverables, shipped, in-flight, days since last delivery). Use for 'who is working on what?', 'stakeholder status questions.'",
            "parameters": { "type": "OBJECT", "properties": {} }
        },
        {
            "name": "get_stakeholder_deliverables",
            "description": "Get all deliverables associated with a specific stakeholder. Use after finding a stakeholder via get_stakeholders.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "stakeholder_id": { "type": "STRING", "description": "Stakeholder ID" }
                },
                "required": ["stakeholder_id"]
            }
        },
        {
            "name": "search_captures",
            "description": "Search through captured thoughts, Claude links, and artifact links by keyword. Captures are raw ideas and notes that haven't been promoted to deliverables yet.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Search keywords" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "search_conversations",
            "description": "Search ingested Claude/work conversations by title or summary. Use when the user asks about past Claude work, backfilled conversations, or source discussion history.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Search keywords" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_conversation_detail",
            "description": "Get a specific ingested conversation plus deliverables that were created from or linked to it. Use after search_conversations when the original context matters.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "id": { "type": "STRING", "description": "Conversation ID" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "get_work_graph_context",
            "description": "Get a compact graph-memory view of durable memory, initiatives, deliverables, stakeholders, conversations, captures, and their relationships. Use when the user asks how work connects, what depends on what, or needs broad context.",
            "parameters": { "type": "OBJECT", "properties": {} }
        },
        {
            "name": "retrieve_brain_context",
            "description": "Retrieve ranked local Kuzu brain graph context for the current question, including connected entities and relation summaries across tasks, meetings, email threads, memories, Ask chats, captures, and deliverables. Use alongside retrieve_memory for non-trivial workspace questions.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "The user's question or a focused graph retrieval query" },
                    "focus_entity_id": { "type": "STRING", "description": "Optional graph node id or source entity id to expand around" },
                    "max_hops": { "type": "INTEGER", "description": "Optional neighborhood depth, default 2" },
                    "limit": { "type": "INTEGER", "description": "Optional node cap, default 24" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "query_brain_cypher",
            "description": "Advanced read-only Cypher query over the local Kuzu brain graph. Use only when retrieve_brain_context is not precise enough. Mutating keywords are rejected and results are bounded.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Read-only Cypher beginning with MATCH, RETURN, WITH, or UNWIND" },
                    "params": { "type": "OBJECT", "description": "Optional Cypher parameters" },
                    "limit": { "type": "INTEGER", "description": "Optional row cap, default 100, max 500" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "run_brain_template",
            "description": "Run a deterministic second-brain graph template for common questions. Prefer this over raw Cypher for focus today, blocked work, email follow-ups, stale work, and stakeholder context because it uses the graph topology and returns the exact Cypher shape.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "template": {
                        "type": "STRING",
                        "enum": ["focus_today", "blocked_work", "email_followups", "stale_work", "stakeholder_context"]
                    },
                    "focus_entity_id": {
                        "type": "STRING",
                        "description": "Optional graph node id or source entity id, especially for stakeholder_context"
                    },
                    "limit": {
                        "type": "INTEGER",
                        "description": "Optional row/node cap"
                    }
                },
                "required": ["template"]
            }
        },
        {
            "name": "get_daily_brain_brief",
            "description": "Generate the daily chief-of-staff brain brief: focus today, blocked/waiting work, email follow-ups, stale important work, and inferred links awaiting review.",
            "parameters": { "type": "OBJECT", "properties": {} }
        },
        {
            "name": "record_brain_feedback",
            "description": "Record whether a graph answer was useful, wrong, or ignored, optionally with corrected relationships or inference ids to accept/reject.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "question": { "type": "STRING" },
                    "template": { "type": "STRING" },
                    "feedback": {
                        "type": "STRING",
                        "enum": ["useful", "wrong", "ignored"]
                    },
                    "corrected": {
                        "type": "OBJECT",
                        "description": "Optional correction. Supports inference_id, accepted_inference_ids, rejected_inference_ids, or corrected_relationship."
                    }
                },
                "required": ["question", "feedback"]
            }
        },
        {
            "name": "record_brain_learning_event",
            "description": "Record a second-brain reinforcement learning signal for a shown/clicked/completed/ignored item. Use this when the user explicitly reacts to a brain result or when a tool action proves a shown item was useful.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "template": { "type": "STRING" },
                    "item_id": { "type": "STRING" },
                    "item_kind": { "type": "STRING" },
                    "event_type": {
                        "type": "STRING",
                        "enum": ["shown", "clicked", "opened", "useful", "wrong", "ignored", "completed_after_seen", "accepted_inference", "rejected_inference", "manual_link_created", "dismissed", "snoozed"]
                    },
                    "reward": { "type": "NUMBER" },
                    "context": { "type": "OBJECT" }
                },
                "required": ["item_id", "event_type"]
            }
        },
        {
            "name": "get_brain_learning_snapshot",
            "description": "Inspect the local second-brain contextual-bandit policy, recent learning events, and top learned item scores.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "template": { "type": "STRING" },
                    "limit": { "type": "INTEGER" }
                }
            }
        },
        {
            "name": "retrieve_memory",
            "description": "Retrieve durable global work memory relevant to the current question. Use early for almost every non-trivial answer because memory is first-class context: what the user does, how they work, why the work exists, and past decisions/preferences.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "The current user request or focused retrieval query" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "save_memory",
            "description": "Persist a work-related memory that should affect future answers. Use for explicit remember requests, durable preferences, recurring work patterns, important decisions, and project facts. Work-related memory does not require confirmation.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "kind": {
                        "type": "STRING",
                        "description": "Memory taxonomy",
                        "enum": ["episodic", "semantic", "procedural"]
                    },
                    "title": { "type": "STRING", "description": "Short memory title" },
                    "body": { "type": "STRING", "description": "Precise memory body, source-grounded and future-useful" },
                    "tags": {
                        "type": "ARRAY",
                        "items": { "type": "STRING" },
                        "description": "Short lowercase retrieval tags"
                    }
                },
                "required": ["kind", "title", "body"]
            }
        },
        {
            "name": "search_email_threads",
            "description": "Full-text search synced Gmail threads. Supports optional category filter (work, personal, newsletter, notification, other). Use get_email_category_summary first for counting/stats questions.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Search keywords (can be empty string when filtering only by category)" },
                    "category": { "type": "STRING", "description": "Filter by AI category: work, personal, newsletter, notification, other" },
                    "limit": { "type": "INTEGER", "description": "Max results (default 12, max 50)" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "get_email_category_summary",
            "description": "Returns thread counts per AI category and the user's account email. Category definitions use the work domains configured by the user; other categories include personal, newsletter, receipt, meeting, spam, and other. Use this first for email counting or statistics questions.",
            "parameters": {
                "type": "OBJECT",
                "properties": {}
            }
        },
        {
            "name": "get_email_thread",
            "description": "Get the full locally synced Gmail thread, including messages, participants, attachments, shared links, linked deliverables, and AI summary/sentiment if present.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "thread_id": { "type": "STRING", "description": "Gmail thread id" }
                },
                "required": ["thread_id"]
            }
        },
        {
            "name": "create_deliverable_from_email",
            "description": "Create a backlog deliverable from a Gmail thread and link the thread to it. Use only when the user asks you to turn an email/thread into a deliverable or clearly approves doing so.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "thread_id": { "type": "STRING", "description": "Gmail thread id" },
                    "title": { "type": "STRING", "description": "Deliverable title" },
                    "claim": { "type": "STRING", "description": "Concrete claim/description for the deliverable" },
                    "type": {
                        "type": "STRING",
                        "description": "Deliverable type",
                        "enum": ["deck", "design_doc", "prototype", "analysis", "framework", "pitch", "research", "code", "email", "meeting_prep", "other"]
                    },
                    "initiative_id": { "type": "STRING", "description": "Optional initiative id to link" }
                },
                "required": ["thread_id", "title", "claim"]
            }
        },
        {
            "name": "link_email_thread_to_deliverable",
            "description": "Link an existing Gmail thread to an existing deliverable. Always find both ids first.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "thread_id": { "type": "STRING", "description": "Gmail thread id" },
                    "deliverable_id": { "type": "STRING", "description": "Deliverable id" }
                },
                "required": ["thread_id", "deliverable_id"]
            }
        },
        {
            "name": "link_email_thread_to_initiative",
            "description": "Link an existing Gmail thread to an existing initiative. Always find both ids first.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "thread_id": { "type": "STRING", "description": "Gmail thread id" },
                    "initiative_id": { "type": "STRING", "description": "Initiative id" }
                },
                "required": ["thread_id", "initiative_id"]
            }
        },
        {
            "name": "capture_email_thread",
            "description": "Save a Gmail thread into the capture inbox with its subject, summary/snippet, and thread route. Use when the user asks to capture or remember an email.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "thread_id": { "type": "STRING", "description": "Gmail thread id" }
                },
                "required": ["thread_id"]
            }
        },
        {
            "name": "get_current_week",
            "description": "Get the current week plan: which deliverable is focused each day (Mon–Fri), tasks due this week, and the next scheduled meeting date.",
            "parameters": { "type": "OBJECT", "properties": {} }
        },
        {
            "name": "get_recent_activity",
            "description": "Get the most recently updated deliverables, latest meetings, and newest captures. Use for 'what's been happening?', 'catch me up', or when the question is time-sensitive without a specific topic.",
            "parameters": { "type": "OBJECT", "properties": {} }
        },

        // ── Write tools ──────────────────────────────────────────────────────
        {
            "name": "add_deliverable_note",
            "description": "Append a freeform note to a deliverable. Use when the user says 'add a note to X', 'log this against X', or gives you information to record about a specific work item. Always search for the deliverable first to get its id.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "deliverable_id": { "type": "STRING", "description": "The deliverable's id" },
                    "body": { "type": "STRING", "description": "Note body text" }
                },
                "required": ["deliverable_id", "body"]
            }
        },
        {
            "name": "add_initiative_note",
            "description": "Append a freeform note to a strategic initiative. Use when the user says 'add a note to initiative X' or wants to record something against an initiative.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "initiative_id": { "type": "STRING", "description": "The initiative's id" },
                    "body": { "type": "STRING", "description": "Note body text" }
                },
                "required": ["initiative_id", "body"]
            }
        },
        {
            "name": "create_capture",
            "description": "Save a new thought, idea, or reminder to the capture inbox. Use when the user says 'capture this', 'remember this', 'make a note', or wants to save something that isn't yet tied to a deliverable.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "body": { "type": "STRING", "description": "The thought or idea to capture" }
                },
                "required": ["body"]
            }
        },
        {
            "name": "update_deliverable_state",
            "description": "Change the state of a deliverable. Use when the user says 'mark X as shipped', 'move X to in review', 'put X back to drafting', or 'kill X'. Always search for the deliverable first to confirm the correct id.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "id": { "type": "STRING", "description": "Deliverable id" },
                    "state": {
                        "type": "STRING",
                        "description": "New state",
                        "enum": ["drafting", "in_review", "shipped", "killed"]
                    }
                },
                "required": ["id", "state"]
            }
        },
        {
            "name": "set_deliverable_focus",
            "description": "Set or clear the focus flag on a deliverable. Only one deliverable can be focused at a time — setting focus on one clears all others. Use when the user says 'focus on X', 'this is my priority', or 'unfocus X'.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "id": { "type": "STRING", "description": "Deliverable id" },
                    "focused": { "type": "BOOLEAN", "description": "true to focus, false to unfocus" }
                },
                "required": ["id", "focused"]
            }
        },
        {
            "name": "list_pending_tasks",
            "description": "List all open (todo or doing) tasks across all active deliverables. Use when the user asks 'what are my tasks', 'what do I need to do', 'what's on my plate', 'show me pending tasks', or similar. Returns tasks sorted by status (doing first) and due date.",
            "parameters": {
                "type": "OBJECT",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "add_deliverable_task",
            "description": "Add a new task (action item) to a deliverable. Use when the user says 'add task X to deliverable Y', 'I need to do Z for this deliverable', or asks you to break down work. Always find the deliverable id first. Optionally include a notes field for context and a url field for a reference link.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "deliverable_id": { "type": "STRING", "description": "Deliverable id" },
                    "title": { "type": "STRING", "description": "Task title in imperative form, e.g. 'Draft the executive summary'" },
                    "due_date": { "type": "STRING", "description": "Optional due date in YYYY-MM-DD format" },
                    "notes": { "type": "STRING", "description": "Optional context or details about the task" },
                    "url": { "type": "STRING", "description": "Optional reference URL for this task" }
                },
                "required": ["deliverable_id", "title"]
            }
        },
        {
            "name": "update_task_status",
            "description": "Change the status of a specific task. Use when the user says 'mark task X as done', 'start working on Y', 'I finished Z'. Call list_pending_tasks or get_deliverable_detail first to get the task's id.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "task_id": { "type": "STRING", "description": "Task id (from list_pending_tasks or get_deliverable_detail)" },
                    "status": {
                        "type": "STRING",
                        "description": "New status",
                        "enum": ["todo", "doing", "done"]
                    }
                },
                "required": ["task_id", "status"]
            }
        },
        {
            "name": "update_deliverable_metadata",
            "description": "Update deadline, effort score, impact score, or blocker reason for a deliverable. Only include fields you want to change — omitted fields keep their current values. Pass an empty string for deadline or blocker_reason to clear them.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "id": { "type": "STRING", "description": "Deliverable id" },
                    "deadline": { "type": "STRING", "description": "Due date in YYYY-MM-DD format, or empty string to clear" },
                    "effort": { "type": "INTEGER", "description": "Effort score 1–5 (1=trivial, 5=huge lift)" },
                    "impact": { "type": "INTEGER", "description": "Impact score 1–5 (1=low, 5=high)" },
                    "blocker_reason": { "type": "STRING", "description": "Why this is blocked, or empty string to clear the blocker" }
                },
                "required": ["id"]
            }
        }
        ,
        {
            "name": "ask_user_question",
            "description": "Ask the user a multiple-choice clarification question. Use only when an action or write is ambiguous, risky, or has multiple plausible targets. After calling it, stop and return the same question/options in the final JSON questions array.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "header": { "type": "STRING", "description": "Short chip label, max 12 characters" },
                    "question": { "type": "STRING", "description": "Clear question to ask the user" },
                    "options": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "label": { "type": "STRING", "description": "Concise option label, 1-5 words" },
                                "description": { "type": "STRING", "description": "What selecting this option means" }
                            },
                            "required": ["label", "description"]
                        }
                    }
                },
                "required": ["header", "question", "options"]
            }
        },
        {
            "name": "search_files",
            "description": "Full-text search for files (local and Google Drive) tracked in Trace by name, description, or folder path. Returns file id, name, kind (local/drive), mime type, path/link, and entity links. Use this to find documents, briefs, decks, or any file related to a topic.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Keywords to search for in file names, descriptions, or folder paths" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "list_files_for_entity",
            "description": "List all files linked to a specific entity (initiative, deliverable, task, stakeholder, meeting, etc). Use this when the user asks 'what files are attached to X?' or 'show me documents for Y'.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "entity_kind": {
                        "type": "STRING",
                        "description": "Type of the entity",
                        "enum": ["initiative", "deliverable", "deliverable_task", "stakeholder", "capture", "meeting", "conversation"]
                    },
                    "entity_id": { "type": "STRING", "description": "ID of the entity" }
                },
                "required": ["entity_kind", "entity_id"]
            }
        },
        {
            "name": "get_file_detail",
            "description": "Get full metadata for a specific file: name, kind, path or Drive link, mime type, size, description, all entity links, and timestamps. Use after finding a file via search_files or list_files_for_entity.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "file_id": { "type": "STRING", "description": "File ID (from search_files or list_files_for_entity)" }
                },
                "required": ["file_id"]
            }
        },

        // ── Calendar read tools ──────────────────────────────────────────────
        {
            "name": "get_calendar_events",
            "description": "Get all Google Calendar events for a specific date. Returns events with title, time, location, description, and attendees. Use for 'what do I have on Tuesday?', 'show my schedule for Jan 15'.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "date": { "type": "STRING", "description": "Date in YYYY-MM-DD format" }
                },
                "required": ["date"]
            }
        },
        {
            "name": "get_calendar_week",
            "description": "Get all Google Calendar events for a full week starting on the given date. Returns events grouped by day. Use for 'what's on my calendar this week?', 'show my week'.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "week_start": { "type": "STRING", "description": "Monday of the target week in YYYY-MM-DD format" }
                },
                "required": ["week_start"]
            }
        },
        {
            "name": "get_upcoming_events",
            "description": "Get upcoming Google Calendar events from today forward. Use for 'what are my next meetings?', 'what's coming up?', 'do I have anything scheduled soon?'",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "days": { "type": "INTEGER", "description": "Number of days to look ahead (default 7, max 30)" }
                }
            }
        },
        {
            "name": "search_calendar_events",
            "description": "Search Google Calendar events by keyword across title, description, and location. Use for 'find my meeting about X', 'when is the Y review?', 'do I have anything about Z scheduled?'",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING", "description": "Search keywords to match against event title, description, or location" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "find_free_slots",
            "description": "Find free time slots in the calendar for a given date (within 09:00–18:00 work hours). Use for 'when am I free on Friday?', 'find a 1-hour slot tomorrow', 'what time is available this afternoon?'",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "date": { "type": "STRING", "description": "Date to check in YYYY-MM-DD format" }
                },
                "required": ["date"]
            }
        },

        // ── Calendar write tools ─────────────────────────────────────────────
        {
            "name": "create_calendar_event",
            "description": "Create a new event on Google Calendar. Use when the user asks to 'schedule a meeting', 'block time for X', or 'add Y to my calendar'. Always confirm the date/time and title before creating. IMPORTANT: always pass time_zone as 'Asia/Kolkata' for timed events (user is in IST). For attendees, look up their email addresses from stakeholders before calling.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "title": { "type": "STRING", "description": "Event title" },
                    "date": { "type": "STRING", "description": "Event date in YYYY-MM-DD format" },
                    "start_time": { "type": "STRING", "description": "Start time in HH:MM (24h) format, omit for all-day events" },
                    "end_time": { "type": "STRING", "description": "End time in HH:MM (24h) format, omit for all-day events" },
                    "time_zone": { "type": "STRING", "description": "IANA timezone name. Always pass 'Asia/Kolkata' for IST. Required for timed events." },
                    "description": { "type": "STRING", "description": "Optional event description or agenda" },
                    "location": { "type": "STRING", "description": "Optional location or meeting link" },
                    "attendees": { "type": "ARRAY", "items": { "type": "STRING" }, "description": "List of attendee email addresses. Each person will receive a Google Calendar invite email." }
                },
                "required": ["title", "date"]
            }
        },
        {
            "name": "update_calendar_event",
            "description": "Update an existing Google Calendar event. Use when the user asks to 'reschedule', 'change the time of', 'update the description of', or 'rename' a calendar event. Find the event first with search_calendar_events.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "event_id": { "type": "STRING", "description": "Google Calendar event id (gcal_event_id from search results)" },
                    "title": { "type": "STRING", "description": "New event title (omit to keep current)" },
                    "date": { "type": "STRING", "description": "New date in YYYY-MM-DD format (omit to keep current)" },
                    "start_time": { "type": "STRING", "description": "New start time in HH:MM (24h) format (omit to keep current)" },
                    "end_time": { "type": "STRING", "description": "New end time in HH:MM (24h) format (omit to keep current)" },
                    "description": { "type": "STRING", "description": "New description (omit to keep current)" },
                    "location": { "type": "STRING", "description": "New location (omit to keep current)" }
                },
                "required": ["event_id"]
            }
        },
        {
            "name": "delete_calendar_event",
            "description": "Delete a Google Calendar event. Use only when the user explicitly asks to 'delete', 'cancel', or 'remove' a calendar event. Always confirm the event title before deleting. Find the event first with search_calendar_events.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "event_id": { "type": "STRING", "description": "Google Calendar event id (gcal_event_id from search results)" },
                    "title": { "type": "STRING", "description": "Event title (for confirmation in the response)" }
                },
                "required": ["event_id", "title"]
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gemini::{fallback_stakeholder_briefing, parse_extraction_text},
        models::Stakeholder,
    };

    #[test]
    fn public_url_validation_blocks_local_and_credentialed_targets() {
        for url in [
            "http://127.0.0.1/admin",
            "http://169.254.169.254/latest/meta-data",
            "http://10.0.0.1/",
            "http://[::1]/",
            "https://localhost/",
            "https://user:password@example.com/",
            "file:///etc/passwd",
        ] {
            assert!(validate_public_url(url).is_err(), "should reject {url}");
        }
        assert!(validate_public_url("https://example.com/docs").is_ok());
    }

    #[test]
    fn public_ip_validation_blocks_reserved_ranges() {
        for ip in [
            "0.0.0.0",
            "10.1.2.3",
            "100.64.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "198.18.0.1",
            "203.0.113.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(
                !is_public_ip(ip.parse().expect("IP address")),
                "should reject {ip}"
            );
        }
        assert!(is_public_ip("93.184.216.34".parse().expect("IP address")));
        assert!(is_public_ip(
            "2606:2800:220:1:248:1893:25c8:1946"
                .parse()
                .expect("IP address")
        ));
    }

    #[test]
    fn fallback_briefing_has_sections() {
        let briefing = fallback_stakeholder_briefing(
            Stakeholder {
                id: "stakeholder".to_string(),
                name: "CEO".to_string(),
                display_order: 1,
                email: String::new(),
                role: String::new(),
                notes: String::new(),
                avatar_url: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
            },
            Vec::new(),
        );

        assert_eq!(briefing.generated_with, "fallback");
        assert!(!briefing.sections.tldr.is_empty());
    }

    #[test]
    fn extraction_json_is_validated() {
        let result = parse_extraction_text(
            r#"
            {
              "conversation": {
                "title": "Trace ingest",
                "summary": "A conversation about the ingest queue."
              },
              "candidates": [
                {
                  "title": "Review queue",
                  "type": "design_doc",
                  "claim": "This proposes a queue before writes.",
                  "initiative_titles": ["Trace Quality"]
                }
              ]
            }
            "#,
            Some("https://claude.ai/chat/abc123".to_string()),
            "pasted_text",
        )
        .expect("valid extraction");

        assert_eq!(result.conversation.title, "Trace ingest");
        assert_eq!(result.candidates[0].deliverable_type.as_str(), "design_doc");
        assert_eq!(
            result.source_chat_url.as_deref(),
            Some("https://claude.ai/chat/abc123")
        );
    }

    #[test]
    fn malformed_extraction_json_is_rejected() {
        assert!(parse_extraction_text(
            r#"{ "conversation": { "title": "", "summary": "x" }, "candidates": [] }"#,
            None,
            "pasted_text",
        )
        .is_err());
    }
}
pub(super) fn build_minutes_tool_declarations() -> serde_json::Value {
    filter_tool_declarations(&[
        "get_workspace_summary",
        "search_deliverables",
        "get_deliverable_detail",
        "list_initiatives",
        "get_initiative_detail",
        "get_stakeholders",
        "get_stakeholder_deliverables",
        "search_meetings",
        "get_meeting_detail",
        "search_captures",
        "get_recent_activity",
    ])
}

// ── Digest tool declarations (read-only subset) ───────────────────────────────

pub(super) fn build_digest_tool_declarations() -> serde_json::Value {
    filter_tool_declarations(&[
        "get_workspace_summary",
        "get_recent_activity",
        "get_blocked_deliverables",
        "get_high_priority_deliverables",
        "get_current_week",
        "get_deliverables_by_state",
        "list_initiatives",
        "get_initiative_detail",
        "search_deliverables",
        "get_stakeholders",
        "get_deliverable_detail",
    ])
}

fn filter_tool_declarations(names: &[&str]) -> serde_json::Value {
    let filtered: Vec<serde_json::Value> = build_tool_declarations("ask")
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|t| {
            let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("");
            names.contains(&name)
        })
        .collect();
    serde_json::Value::Array(filtered)
}
