//! Capture-promotion prompt construction + Gemini response parsing.
//! Extracted from legacy.rs (Section 13).

use serde::Deserialize;
use ulid::Ulid;

use crate::models::{Capture, CaptureStatus};
use super::legacy::*;

// ---------- Prompt + structured output ----------

#[derive(Debug, Clone, Deserialize)]
struct RawSuggestion {
    kind: String,
    #[serde(default)]
    target_id: Option<String>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    rationale: Option<String>,
    #[serde(default)]
    alternatives: Vec<RawAlternative>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawAlternative {
    kind: String,
    #[serde(default)]
    target_id: Option<String>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    rationale: Option<String>,
}

pub(crate) struct ParsedSuggestion {
    pub(crate) kind: String,
    pub(crate) target_id: Option<String>,
    pub(crate) target_kind: Option<String>,
    pub(crate) target_title: Option<String>,
    pub(crate) confidence: f64,
    pub(crate) rationale: String,
    pub(crate) alternatives: Vec<PromotionAlternative>,
}

const SUGGESTER_INSTRUCTION: &str = r#"You promote captured thoughts into the right project structure for a personal project manager called Trace.

A capture is a quick note the user dropped — could be a task, a decision, a new idea, or a meeting takeaway. Your job: decide whether it becomes
  - a "task" on an existing deliverable (must include target_id),
  - a new "deliverable" (optionally linked to an existing initiative via target_id),
  - or a brand-new "initiative" (target_id is null).

Rules:
- If the capture clearly fits one of the listed candidate deliverables, prefer kind="task" with that target_id.
- If it's a new piece of work but belongs to an existing initiative, prefer kind="deliverable" with target_id = the initiative id.
- Only suggest kind="initiative" when the capture introduces a genuinely new area of work.
- Confidence is your honest probability that the user will accept this suggestion (0.0–1.0). Be calibrated — if the capture is ambiguous, say so with a 0.4–0.6 confidence.
- Provide up to 3 alternatives (different kind or different target). Don't repeat the primary suggestion.
- Rationale: one sentence, plain English, no fluff.
- Only reference target_ids that appear in CANDIDATES. Don't invent IDs.
"#;

pub(crate) fn build_suggester_body(
    capture: &Capture,
    candidates: &[Candidate],
    sanitized: &crate::prompt_safety::Sanitized,
) -> serde_json::Value {
    let mut deliverables = Vec::new();
    let mut initiatives = Vec::new();
    for c in candidates {
        let entry = serde_json::json!({
            "id": c.id,
            "title": c.title,
            "summary": truncate(&c.summary, 240),
        });
        if c.kind == "deliverable" {
            deliverables.push(entry);
        } else {
            initiatives.push(entry);
        }
    }

    let wrapped_body = crate::prompt_safety::wrap_capture("user", &capture.created_at, sanitized);
    let user_payload = serde_json::json!({
        "capture_kind": capture.kind,
        "capture_body": wrapped_body,
        "candidates": {
            "deliverables": deliverables,
            "initiatives": initiatives,
        },
    });

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "kind": { "type": "string", "enum": ["task", "deliverable", "initiative"] },
            "target_id": { "type": ["string", "null"] },
            "confidence": { "type": "number" },
            "rationale": { "type": "string" },
            "alternatives": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": { "type": "string", "enum": ["task", "deliverable", "initiative"] },
                        "target_id": { "type": ["string", "null"] },
                        "confidence": { "type": "number" },
                        "rationale": { "type": "string" }
                    },
                    "required": ["kind", "confidence", "rationale"]
                }
            }
        },
        "required": ["kind", "confidence", "rationale"]
    });

    serde_json::json!({
        "systemInstruction": { "parts": [{ "text": SUGGESTER_INSTRUCTION }] },
        "contents": [{
            "role": "user",
            "parts": [{
                "text": format!(
                    "Decide the best promotion for this capture. Use only listed candidate IDs.\n\nINPUTS:\n{}",
                    serde_json::to_string_pretty(&user_payload).unwrap_or_else(|_| user_payload.to_string())
                )
            }]
        }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseJsonSchema": schema,
            "temperature": 0.1
        }
    })
}

pub(crate) fn parse_suggester_response(
    raw: &serde_json::Value,
    candidates: &[Candidate],
) -> Result<ParsedSuggestion, String> {
    let text = raw
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|parts| parts.as_array())
        .and_then(|parts| parts.iter().find_map(|p| p.get("text").and_then(|t| t.as_str())))
        .ok_or_else(|| "suggester response did not include text".to_string())?;
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let raw_parsed: RawSuggestion = serde_json::from_str(cleaned)
        .map_err(|e| format!("suggester JSON validation failed: {e}"))?;

    let mut kind = raw_parsed.kind.trim().to_ascii_lowercase();
    let mut target_id = raw_parsed
        .target_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Resolve + validate the target.
    let mut target_kind = None;
    let mut target_title = None;
    if let Some(id) = &target_id {
        if let Some(c) = candidates.iter().find(|c| c.id == *id) {
            target_kind = Some(c.kind.clone());
            target_title = Some(c.title.clone());
        } else {
            // Model hallucinated an id; clear it and fall back.
            target_id = None;
        }
    }

    // task without a deliverable target is invalid — fall back to a
    // deliverable so the suggestion is still actionable instead of erroring.
    if kind == "task" {
        match target_kind.as_deref() {
            Some("deliverable") => {}
            _ => {
                kind = "deliverable".to_string();
                if target_kind.as_deref() == Some("initiative") {
                    // Keep the initiative as the parent.
                } else {
                    target_id = None;
                    target_kind = None;
                    target_title = None;
                }
            }
        }
    }

    if !matches!(kind.as_str(), "task" | "deliverable" | "initiative") {
        return Err(format!("invalid kind: {kind}"));
    }

    let confidence = raw_parsed.confidence.unwrap_or(0.5).clamp(0.0, 1.0);
    let rationale = raw_parsed.rationale.unwrap_or_default().trim().to_string();

    let mut alternatives = Vec::new();
    for alt in raw_parsed.alternatives.into_iter().take(MAX_ALTERNATIVES) {
        let kind = alt.kind.trim().to_ascii_lowercase();
        if !matches!(kind.as_str(), "task" | "deliverable" | "initiative") {
            continue;
        }
        let alt_target_id = alt.target_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let (alt_target_kind, alt_target_title) = match &alt_target_id {
            Some(id) => candidates
                .iter()
                .find(|c| c.id == *id)
                .map(|c| (Some(c.kind.clone()), Some(c.title.clone())))
                .unwrap_or((None, None)),
            None => (None, None),
        };
        // Skip exact duplicates of the primary.
        if kind == kind_lower(&raw_parsed.kind) && alt_target_id == target_id {
            continue;
        }
        alternatives.push(PromotionAlternative {
            kind,
            target_id: alt_target_id,
            target_kind: alt_target_kind,
            target_title: alt_target_title,
            confidence: alt.confidence.unwrap_or(0.4).clamp(0.0, 1.0),
            rationale: alt.rationale.unwrap_or_default().trim().to_string(),
        });
    }

    Ok(ParsedSuggestion {
        kind,
        target_id,
        target_kind,
        target_title,
        confidence,
        rationale,
        alternatives,
    })
}

fn kind_lower(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push('…');
    out
}

pub(crate) fn ephemeral_capture(text: &str) -> Capture {
    Capture {
        id: format!("eval_{}", Ulid::new()),
        kind: "thought".to_string(),
        body: text.to_string(),
        status: CaptureStatus::Inbox.as_str().to_string(),
        promoted_deliverable_id: None,
        promoted_deliverable_title: None,
        promoted_initiative_id: None,
        promoted_initiative_title: None,
        promoted_conversation_id: None,
        promoted_conversation_title: None,
        promoted_task_id: None,
        promoted_task_title: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        promoted_at: None,
    }
}

