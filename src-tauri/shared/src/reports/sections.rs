//! Per-section drafting helpers for the report pipeline.
//!
//! Three primitives, each one Gemini call:
//! - [`plan_sections`] — cheap Flash call producing a section list (heading + instruction)
//! - [`draft_section`] — Pro call producing one section's polished Markdown
//! - [`critique_draft`] — Flash call producing an issues list over the assembled draft
//!
//! All three honour the strict resolved scope: evidence is filtered before the
//! prompt is built, and the prompt repeats the scope constraint verbatim. The
//! scope leak that contaminated v1's outline (Articles Project → Notes Project
//! report) cannot recur because cross-initiative source units never reach the
//! evidence packet.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::models::{
    GraphQueryMode, ReasoningDepth, ReasoningQueryInput, ReasoningResult, ReasoningScope,
    ReportCritique, ReportCritiqueIssue, ReportSectionDraft, ReportSectionPlan, ReportType,
};
use crate::reasoning::{self, ResolvedScope};

use super::templates::{report_label, scope_label};

const PLAN_MODEL: &str = "gemini-3-flash-preview";
const CRITIQUE_MODEL: &str = "gemini-3-flash-preview";

#[derive(Debug, Deserialize)]
struct PlannedSections {
    #[serde(default)]
    sections: Vec<PlannedSection>,
}

#[derive(Debug, Deserialize)]
struct PlannedSection {
    heading: String,
    #[serde(default)]
    instructions: String,
}

/// Ask Gemini Flash to propose 4–7 sections for the report. Returns a fresh
/// `Vec<ReportSectionPlan>` with stable section ids the UI uses for drag /
/// rename / re-draft.
pub async fn plan_sections(
    pool: &SqlitePool,
    api_key: &str,
    report_type: ReportType,
    title: &str,
    scope: &ReasoningScope,
    resolved: &ResolvedScope,
) -> Result<Vec<ReportSectionPlan>, String> {
    let coverage = scope_label(scope, &resolved.initiative_titles);
    let scope_names = if resolved.initiative_titles.is_empty() {
        "the selected scope".to_string()
    } else {
        resolved
            .initiative_titles
            .iter()
            .map(|title| format!("\"{title}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let label = report_label(report_type);
    let prompt = format!(
        r#"You are planning a {label} titled "{title}" for {coverage}.

Strict rule: every section must be about {scope_names} only. Do NOT include sections about other initiatives.

Propose 4 to 7 sections covering the structure of a {label}. For each section, give:
- a short heading (no markdown)
- a one-sentence instruction explaining what the section should cover

Return JSON in the shape: {{ "sections": [{{ "heading": "...", "instructions": "..." }}] }}"#,
    );
    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "temperature": 0.2,
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "OBJECT",
                "properties": {
                    "sections": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "heading": { "type": "STRING" },
                                "instructions": { "type": "STRING" }
                            },
                            "required": ["heading", "instructions"]
                        }
                    }
                },
                "required": ["sections"]
            }
        }
    });
    let raw = crate::gemini::post_gemini_external(
        Some(pool),
        "reports_plan_sections",
        PLAN_MODEL,
        api_key,
        &body,
    )
    .await?;
    let planned = parse_planned_sections(&raw)?;
    let mut out = Vec::with_capacity(planned.sections.len());
    for (idx, section) in planned.sections.into_iter().enumerate() {
        out.push(ReportSectionPlan {
            id: format!("sec_{}", Ulid::new()),
            heading: section.heading.trim().to_string(),
            instructions: section.instructions.trim().to_string(),
            status: "queued".to_string(),
            position: idx as i64,
        });
    }
    Ok(out)
}

/// Draft a single section using deep reasoning. Returns the section's
/// Markdown body plus the citation ids it uses.
///
/// Reuses `query_reasoning_graph` so all evidence filtering, caching, and
/// prompt-injection guards stay consistent with Ask.
pub async fn draft_section(
    pool: &SqlitePool,
    brain_path: &Path,
    api_key: &str,
    report_run_id: &str,
    report_type: ReportType,
    report_title: &str,
    scope: &ReasoningScope,
    resolved: &ResolvedScope,
    section: &ReportSectionPlan,
) -> Result<(ReportSectionDraft, ReasoningResult), String> {
    let label = report_label(report_type);
    let scope_names = if resolved.initiative_titles.is_empty() {
        "the selected scope".to_string()
    } else {
        resolved
            .initiative_titles
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let extra = if section.instructions.is_empty() {
        String::new()
    } else {
        format!("\nWriter instructions for this section: {}\n", section.instructions)
    };
    let query = format!(
        r#"Draft the section "{heading}" of the {label} "{title}".

Strict scope: write about {scope_names} ONLY. If a source packet is about a different initiative, ignore it.{extra}

Produce 2–5 short paragraphs of polished Markdown. Cite factual sentences using [S#] markers. Do not include the section heading itself — only the body."#,
        heading = section.heading,
        label = label,
        title = report_title,
    );
    // Use a short, topic-focused retrieval query so BM25/embedding matching
    // returns relevant emails/meetings/deliverables rather than matching on
    // instruction words ("Produce polished Markdown", "Cite factual sentences").
    let retrieval_query = {
        let mut parts = resolved.initiative_titles.clone();
        parts.push(section.heading.clone());
        if !section.instructions.is_empty() {
            parts.push(section.instructions.clone());
        }
        parts.join(" ")
    };
    let result = reasoning::query_reasoning_graph(
        pool,
        brain_path,
        api_key,
        ReasoningQueryInput {
            query,
            retrieval_query: Some(retrieval_query),
            depth: ReasoningDepth::Deep,
            query_mode: GraphQueryMode::Drift,
            scope: scope.clone(),
            purpose: Some(format!("report:{report_run_id}:section:{}", section.id)),
        },
    )
    .await?;
    let draft = ReportSectionDraft {
        markdown: result.answer_markdown.clone(),
        citation_ids: result
            .citations
            .iter()
            .map(|c| c.source_unit_id.clone())
            .collect(),
        last_drafted_at: Some(crate::repo::now_utc()),
        cache_hit: result.cache_hit,
    };
    Ok((draft, result))
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelCritique {
    #[serde(default)]
    issues: Vec<ModelCritiqueIssue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelCritiqueIssue {
    section_id: String,
    kind: String,
    message: String,
    #[serde(default)]
    span_text: Option<String>,
    #[serde(default)]
    suggestion: Option<String>,
}

/// Run a Flash pass over the assembled draft and flag contradictions, weak
/// citations, scope leaks, and unsupported assertions. The frontend renders
/// the returned issues both inline (highlighted spans) and in the Issues bar.
pub async fn critique_draft(
    pool: &SqlitePool,
    api_key: &str,
    report_type: ReportType,
    report_title: &str,
    scope_names: &str,
    sections: &[ReportSectionPlan],
    drafts: &std::collections::HashMap<String, ReportSectionDraft>,
) -> Result<ReportCritique, String> {
    if sections.is_empty() {
        return Ok(ReportCritique::default());
    }
    let assembled = sections
        .iter()
        .map(|sec| {
            let body = drafts
                .get(&sec.id)
                .map(|draft| draft.markdown.as_str())
                .unwrap_or("(no draft)");
            format!(
                "## section_id={section_id} :: {heading}\n{body}",
                section_id = sec.id,
                heading = sec.heading,
                body = body
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");
    let prompt = format!(
        r#"Review this {label} titled "{title}" for {scope_names}.

Surface only material problems:
- contradiction: two sentences disagree about a fact
- weak_citation: a factual claim is missing a [S#] citation
- scope_leak: content that is clearly not about {scope_names}
- unsupported: a confident assertion the evidence doesn't justify

For each issue, name the section_id it lives in, pick a kind from the four
above, write a one-sentence message, optionally quote the span_text, and
optionally suggest a fix. If the report is sound, return an empty array.

Draft:
{assembled}

Return JSON in the shape: {{ "issues": [...] }}"#,
        label = report_label(report_type),
        title = report_title,
    );
    let body = json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "temperature": 0.1,
            "responseMimeType": "application/json",
            "responseSchema": {
                "type": "OBJECT",
                "properties": {
                    "issues": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "section_id": { "type": "STRING" },
                                "kind": { "type": "STRING" },
                                "message": { "type": "STRING" },
                                "span_text": { "type": "STRING" },
                                "suggestion": { "type": "STRING" }
                            },
                            "required": ["section_id", "kind", "message"]
                        }
                    }
                },
                "required": ["issues"]
            }
        }
    });
    let raw = crate::gemini::post_gemini_external(
        Some(pool),
        "reports_critique",
        CRITIQUE_MODEL,
        api_key,
        &body,
    )
    .await?;
    let critique = parse_critique(&raw)?;
    let issues = critique
        .issues
        .into_iter()
        .map(|issue| ReportCritiqueIssue {
            id: format!("crit_{}", Ulid::new()),
            section_id: issue.section_id,
            kind: issue.kind,
            message: issue.message,
            span_text: issue.span_text,
            suggestion: issue.suggestion,
        })
        .collect();
    Ok(ReportCritique {
        issues,
        generated_at: Some(crate::repo::now_utc()),
    })
}

fn parse_planned_sections(raw: &Value) -> Result<PlannedSections, String> {
    let text = raw
        .get("candidates")
        .and_then(|value| value.get(0))
        .and_then(|value| value.get("content"))
        .and_then(|value| value.get("parts"))
        .and_then(|value| value.get(0))
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Gemini plan-sections response had no text".to_string())?;
    serde_json::from_str(text)
        .map_err(|error| format!("Gemini plan-sections output was invalid: {error}"))
}

fn parse_critique(raw: &Value) -> Result<ModelCritique, String> {
    let text = raw
        .get("candidates")
        .and_then(|value| value.get(0))
        .and_then(|value| value.get("content"))
        .and_then(|value| value.get("parts"))
        .and_then(|value| value.get(0))
        .and_then(|value| value.get("text"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| "Gemini critique response had no text".to_string())?;
    serde_json::from_str(text)
        .map_err(|error| format!("Gemini critique output was invalid: {error}"))
}
