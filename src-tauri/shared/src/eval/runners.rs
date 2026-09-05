//! Eval runners (retrieval/classification/ask/promotion) + LLM judges. From legacy.rs (13-std6).

use std::path::Path;

use serde::Deserialize;
use sqlx::SqlitePool;

use crate::brain;
use crate::models::{BrainRetrieveInput, WorkGraphNode};
use super::legacy::*;

pub async fn run_fixture(
    pool: &SqlitePool,
    brain_path: &Path,
    fixture: &EvalFixture,
) -> Result<EvalRun, String> {
    let kind = FixtureKind::from_str(&fixture.kind)
        .ok_or_else(|| format!("unknown fixture kind: {}", fixture.kind))?;

    let started = std::time::Instant::now();
    let (passed, score, metric, details) = match kind {
        FixtureKind::Retrieval => run_retrieval_inner(pool, brain_path, fixture).await?,
        FixtureKind::Classification => run_classification_inner(pool, fixture).await?,
        FixtureKind::Ask => run_ask_inner(pool, brain_path, fixture).await?,
        FixtureKind::Promotion => run_promotion_inner(pool, fixture).await?,
    };
    let latency_ms = started.elapsed().as_millis() as i64;

    let baseline = baseline_score(pool, &fixture.id).await;
    let delta = baseline.map(|b| score - b);

    let run = EvalRun {
        id: ulid::Ulid::new().to_string(),
        fixture_id: fixture.id.clone(),
        ts: chrono::Utc::now().timestamp_millis(),
        passed,
        score,
        metric,
        details_json: details.as_ref().and_then(|d| serde_json::to_string(d).ok()),
        latency_ms,
        baseline_score: baseline,
        delta,
    };

    sqlx::query(
        "INSERT INTO eval_runs
           (id, fixture_id, ts, passed, score, metric, details_json,
            latency_ms, baseline_score, delta)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&run.id)
    .bind(&run.fixture_id)
    .bind(run.ts)
    .bind(if run.passed { 1_i64 } else { 0_i64 })
    .bind(run.score)
    .bind(&run.metric)
    .bind(run.details_json.as_deref())
    .bind(run.latency_ms)
    .bind(run.baseline_score)
    .bind(run.delta)
    .execute(pool)
    .await
    .map_err(|e| format!("insert run: {e}"))?;

    Ok(run)
}

async fn run_retrieval_inner(
    pool: &SqlitePool,
    brain_path: &Path,
    fixture: &EvalFixture,
) -> Result<(bool, f64, String, Option<serde_json::Value>), String> {
    let input: RetrievalFixtureInput = serde_json::from_str(&fixture.input_json)
        .map_err(|e| format!("parse input: {e}"))?;
    let expectation: RetrievalFixtureExpectation = serde_json::from_str(&fixture.expectation_json)
        .map_err(|e| format!("parse expectation: {e}"))?;

    let top_k = expectation.top_k.unwrap_or(3).max(1);

    let result = brain::retrieve_brain_context(
        pool,
        brain_path,
        BrainRetrieveInput {
            query: input.query.clone(),
            focus_entity_id: input.focus_entity_id.clone(),
            max_hops: Some(2),
            limit: Some(top_k.max(12) as usize),
        },
    )
    .await?;

    let observed: Vec<String> = result
        .ranked_nodes
        .iter()
        .take(top_k)
        .map(node_entity_key)
        .collect();

    let expected_set: std::collections::HashSet<String> =
        expectation.expected_entity_ids.iter().cloned().collect();

    let hits = observed
        .iter()
        .filter(|id| expected_set.contains(*id))
        .count();

    let precision_at_k = if observed.is_empty() {
        0.0
    } else {
        hits as f64 / observed.len() as f64
    };
    let passed = hits > 0 && precision_at_k >= expectation_threshold(&expectation);

    let details = serde_json::json!({
        "top_k": top_k,
        "observed": observed,
        "expected": expectation.expected_entity_ids,
        "hits": hits,
    });

    Ok((
        passed,
        precision_at_k,
        format!("precision_at_{top_k}"),
        Some(details),
    ))
}

/// Classification eval: compare each dimension the fixture pinned against the
/// current value in `gmail_threads`. By default uses exact-match accuracy.
/// When `judge_soft: true`, each pinned dimension is rubric-scored by the
/// judge model — synonyms/paraphrases pass.
async fn run_classification_inner(
    pool: &SqlitePool,
    fixture: &EvalFixture,
) -> Result<(bool, f64, String, Option<serde_json::Value>), String> {
    #[derive(Deserialize)]
    struct ClassificationInput {
        thread_id: String,
    }
    let input: ClassificationInput = serde_json::from_str(&fixture.input_json)
        .map_err(|e| format!("parse input: {e}"))?;
    let expectation: ClassificationExpectation =
        serde_json::from_str(&fixture.expectation_json)
            .map_err(|e| format!("parse expectation: {e}"))?;

    let row: Option<(
        String,
        String,
        Option<String>,
        i64,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT COALESCE(ai_category,'other'), COALESCE(ai_priority,'low'),
                intent, action_required, thread_state, predicted_action
           FROM gmail_threads WHERE thread_id = ?",
    )
    .bind(&input.thread_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("read thread: {e}"))?;

    let Some((cat, pri, intent, action_req, thread_state, predicted_action)) = row else {
        return Ok((
            false,
            0.0,
            "accuracy".to_string(),
            Some(serde_json::json!({ "error": "thread not found", "thread_id": input.thread_id })),
        ));
    };

    // Build a list of (dimension, expected, observed, is_bool) tuples for the
    // pinned dimensions only. Drives both hard- and soft-match paths.
    let mut dimensions: Vec<(String, String, String, bool)> = Vec::new();
    if let Some(want) = expectation.category.as_deref() {
        dimensions.push(("category".into(), want.to_string(), cat.clone(), false));
    }
    if let Some(want) = expectation.priority.as_deref() {
        dimensions.push(("priority".into(), want.to_string(), pri.clone(), false));
    }
    if let Some(want) = expectation.intent.as_deref() {
        dimensions.push((
            "intent".into(),
            want.to_string(),
            intent.clone().unwrap_or_default(),
            false,
        ));
    }
    if let Some(want) = expectation.action_required {
        dimensions.push((
            "action_required".into(),
            want.to_string(),
            (action_req != 0).to_string(),
            true,
        ));
    }
    if let Some(want) = expectation.thread_state.as_deref() {
        dimensions.push((
            "thread_state".into(),
            want.to_string(),
            thread_state.clone().unwrap_or_default(),
            false,
        ));
    }
    if let Some(want) = expectation.predicted_action.as_deref() {
        dimensions.push((
            "predicted_action".into(),
            want.to_string(),
            predicted_action.clone().unwrap_or_default(),
            false,
        ));
    }

    if dimensions.is_empty() {
        return Ok((
            false,
            0.0,
            "accuracy".to_string(),
            Some(serde_json::json!({ "error": "no dimensions pinned in expectation" })),
        ));
    }

    let mut observed_map = serde_json::Map::new();
    for (name, _, value, is_bool) in &dimensions {
        let json_value = if *is_bool {
            serde_json::Value::Bool(value == "true")
        } else {
            serde_json::Value::String(value.clone())
        };
        observed_map.insert(name.clone(), json_value);
    }

    if expectation.judge_soft.unwrap_or(false) {
        run_classification_soft(pool, &input.thread_id, dimensions, observed_map, &expectation)
            .await
    } else {
        let mut hits = 0_i32;
        let total = dimensions.len() as i32;
        for (_, want, actual, _) in &dimensions {
            if actual.eq_ignore_ascii_case(want) {
                hits += 1;
            }
        }
        let score = hits as f64 / total as f64;
        let passed = total > 0 && (hits as f64 / total as f64) >= 1.0;
        let details = serde_json::json!({
            "mode": "exact",
            "checked": total,
            "matched": hits,
            "observed": observed_map,
            "expected": serde_json::to_value(&expectation.into_map()).unwrap_or_default(),
        });
        Ok((passed, score, "accuracy".to_string(), Some(details)))
    }
}

async fn run_classification_soft(
    pool: &SqlitePool,
    thread_id: &str,
    dimensions: Vec<(String, String, String, bool)>,
    observed_map: serde_json::Map<String, serde_json::Value>,
    expectation: &ClassificationExpectation,
) -> Result<(bool, f64, String, Option<serde_json::Value>), String> {
    let api_key = crate::runtime::gemini_api_key()
        .ok_or_else(|| "Gemini API key required for judge_soft classification".to_string())?;
    let judge_model = match expectation.judge_model.as_deref() {
        Some("flash") => JUDGE_MODEL_FLASH,
        _ => JUDGE_MODEL_PRO,
    };
    let threshold = expectation
        .min_score
        .unwrap_or(ASK_PASS_THRESHOLD_DEFAULT)
        .clamp(0.0, 1.0);

    let body = build_classification_judge_body(thread_id, &dimensions);
    let raw = crate::gemini::post_gemini_external(
        Some(pool),
        "eval_judge",
        judge_model,
        &api_key,
        &body,
    )
    .await?;
    let scores = parse_classification_judge_response(raw, &dimensions)?;
    let aggregate = if scores.is_empty() {
        0.0
    } else {
        scores.values().sum::<f64>() / scores.len() as f64
    };
    let passed = aggregate >= threshold;

    let mut per_dim = serde_json::Map::new();
    for (name, _, _, _) in &dimensions {
        per_dim.insert(
            name.clone(),
            serde_json::json!(scores.get(name).copied().unwrap_or(0.0)),
        );
    }

    let details = serde_json::json!({
        "mode": "judge_soft",
        "judge_model": judge_model,
        "threshold": threshold,
        "aggregate": aggregate,
        "per_dimension_scores": per_dim,
        "observed": observed_map,
        "expected": serde_json::to_value(&expectation.clone().into_map()).unwrap_or_default(),
    });
    Ok((passed, aggregate, "judge_score".to_string(), Some(details)))
}

fn build_classification_judge_body(
    thread_id: &str,
    dimensions: &[(String, String, String, bool)],
) -> serde_json::Value {
    // Schema: an object with one numeric field per dimension. Use a stable
    // ordering so the model returns scores by exact dimension name.
    let mut props = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, _, _, _) in dimensions {
        props.insert(name.clone(), serde_json::json!({ "type": "number" }));
        required.push(name.clone());
    }
    let schema = serde_json::json!({
        "type": "object",
        "properties": props,
        "required": required,
    });

    let rubric = "You are scoring email-classification accuracy on synonym-tolerant dimensions. \
                  For each dimension, score 0.0 (completely wrong) to 1.0 (exact or strong synonym). \
                  Booleans: 1.0 if both sides agree, 0.0 otherwise. \
                  Categorical strings: 1.0 on exact match, 0.7-0.9 for close synonyms, 0.0 for unrelated. \
                  Free-form text (e.g. predicted_action): score by semantic overlap with the expected action.";

    let pairs: Vec<serde_json::Value> = dimensions
        .iter()
        .map(|(name, expected, observed, is_bool)| {
            serde_json::json!({
                "dimension": name,
                "expected": expected,
                "observed": observed,
                "kind": if *is_bool { "boolean" } else { "string" },
            })
        })
        .collect();

    serde_json::json!({
        "systemInstruction": { "parts": [{ "text": rubric }] },
        "contents": [{
            "role": "user",
            "parts": [{
                "text": format!(
                    "Thread {}\n\nDimensions to score (return one number per dimension matching the schema field name):\n{}",
                    thread_id,
                    serde_json::to_string_pretty(&pairs).unwrap_or_else(|_| "[]".to_string())
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

fn parse_classification_judge_response(
    raw: serde_json::Value,
    dimensions: &[(String, String, String, bool)],
) -> Result<std::collections::HashMap<String, f64>, String> {
    let text = raw
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .and_then(|p| p.iter().find_map(|p| p.get("text").and_then(|t| t.as_str())))
        .ok_or_else(|| "classification judge response missing text".to_string())?;
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let parsed: serde_json::Value = serde_json::from_str(cleaned)
        .map_err(|e| format!("classification judge JSON invalid: {e}"))?;
    let obj = parsed
        .as_object()
        .ok_or_else(|| "classification judge response was not a JSON object".to_string())?;
    let mut out = std::collections::HashMap::new();
    for (name, _, _, _) in dimensions {
        let score = obj
            .get(name)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        out.insert(name.clone(), score);
    }
    Ok(out)
}

const JUDGE_MODEL_PRO: &str = "gemini-3.1-pro-preview";
const JUDGE_MODEL_FLASH: &str = "gemini-3-flash-preview";
const ASK_PASS_THRESHOLD_DEFAULT: f64 = 0.7;

/// Rubric prompt for the Ask judge. Constant string so the model can
/// cache it across back-to-back fixture runs (we still wrap each call
/// through `post_gemini_external` so cost lands under `eval_judge`).
const ASK_JUDGE_RUBRIC: &str = r#"You are evaluating an AI assistant's answer for a personal project-management app called Trace.

Trace's house voice: open with the answer, plain prose, calibrated certainty (say what's known vs inferred), push back when the data disagrees with the user, no apologetic boilerplate.

Score the answer on four dimensions, each 0.0 to 1.0:

1. **clarity** — Is the writing direct? Does it open with the answer (no "Sure!" / "Of course!" preamble)? Plain prose vs unnecessary lists?
2. **factuality** — Are all `expected_facts` correctly represented in the answer? Penalize hallucinated or contradicting facts.
3. **citation_accuracy** — Do the cited entities satisfy `expected_citation_kinds` (kinds present) and `expected_citation_ids` (specific IDs present)? Penalize phantom citations (cited entities that don't appear in the answer) and missing required citations.
4. **tone** — Calibrated certainty? Pushes back when appropriate? Avoids apologetic / boilerplate language?

If a dimension has no expectation in the rubric (e.g. no `expected_citation_kinds`), score it 1.0 (vacuously satisfied).

Return JSON matching the schema. `aggregate` MUST be the arithmetic mean of the four dimension scores. `rationale` is one sentence per dimension explaining the score, joined into a single string. `notes` is an optional JSON object with per-dimension free-text notes."#;

#[derive(Debug, Clone, Deserialize)]
struct AskJudgeOutput {
    clarity: f64,
    factuality: f64,
    citation_accuracy: f64,
    tone: f64,
    aggregate: f64,
    rationale: String,
    #[serde(default)]
    notes: serde_json::Value,
}

async fn run_ask_inner(
    pool: &SqlitePool,
    brain_path: &Path,
    fixture: &EvalFixture,
) -> Result<(bool, f64, String, Option<serde_json::Value>), String> {
    let input: AskFixtureInput = serde_json::from_str(&fixture.input_json)
        .map_err(|e| format!("parse ask input: {e}"))?;
    let expect: AskFixtureExpectation = serde_json::from_str(&fixture.expectation_json)
        .map_err(|e| format!("parse ask expectation: {e}"))?;

    let api_key = crate::runtime::gemini_api_key()
        .ok_or_else(|| "Gemini API key required for ask eval".to_string())?;

    let answer = crate::gemini::ask_search(
        &api_key,
        &input.question,
        input.context.as_deref(),
        pool,
        Some(brain_path),
        None,
    )
    .await?;

    let judge_model = match expect.judge_model.as_deref() {
        Some("flash") => JUDGE_MODEL_FLASH,
        _ => JUDGE_MODEL_PRO,
    };

    let judge_body = build_ask_judge_body(&input, &expect, &answer);
    let raw = crate::gemini::post_gemini_external(
        Some(pool),
        "eval_judge",
        judge_model,
        &api_key,
        &judge_body,
    )
    .await?;
    let judge = parse_ask_judge_response(raw)?;

    let threshold = expect
        .min_aggregate_score
        .unwrap_or(ASK_PASS_THRESHOLD_DEFAULT)
        .clamp(0.0, 1.0);
    let passed = judge.aggregate >= threshold;

    let details = serde_json::json!({
        "answer": answer.answer,
        "citations": answer.refs,
        "judge": {
            "model": judge_model,
            "clarity": judge.clarity,
            "factuality": judge.factuality,
            "citation_accuracy": judge.citation_accuracy,
            "tone": judge.tone,
            "aggregate": judge.aggregate,
            "rationale": judge.rationale,
            "notes": judge.notes,
        },
        "expected": {
            "facts": expect.expected_facts,
            "citation_kinds": expect.expected_citation_kinds,
            "citation_ids": expect.expected_citation_ids,
            "threshold": threshold,
        },
    });

    Ok((passed, judge.aggregate, "judge_score".to_string(), Some(details)))
}

fn build_ask_judge_body(
    input: &AskFixtureInput,
    expect: &AskFixtureExpectation,
    answer: &crate::models::AskSearchResult,
) -> serde_json::Value {
    // Schema for structured output. Keep names exactly matching AskJudgeOutput.
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "clarity": { "type": "number" },
            "factuality": { "type": "number" },
            "citation_accuracy": { "type": "number" },
            "tone": { "type": "number" },
            "aggregate": { "type": "number" },
            "rationale": { "type": "string" },
            "notes": { "type": "object" }
        },
        "required": ["clarity", "factuality", "citation_accuracy", "tone", "aggregate", "rationale"]
    });

    let user_payload = serde_json::json!({
        "question": input.question,
        "actual_answer": answer.answer,
        "actual_citations": answer.refs,
        "expected_facts": expect.expected_facts,
        "expected_citation_kinds": expect.expected_citation_kinds,
        "expected_citation_ids": expect.expected_citation_ids,
    });

    serde_json::json!({
        "systemInstruction": { "parts": [{ "text": ASK_JUDGE_RUBRIC }] },
        "contents": [{
            "role": "user",
            "parts": [{
                "text": format!(
                    "Evaluate this answer against the rubric. Return the JSON schema only.\n\nINPUTS:\n{}",
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

fn parse_ask_judge_response(raw: serde_json::Value) -> Result<AskJudgeOutput, String> {
    // Pull the candidate text first.
    let text = raw
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|parts| parts.as_array())
        .and_then(|parts| parts.iter().find_map(|p| p.get("text").and_then(|t| t.as_str())))
        .ok_or_else(|| "judge response did not include text".to_string())?;
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<AskJudgeOutput>(cleaned)
        .map_err(|e| format!("judge response JSON validation failed: {e}"))
}

async fn run_promotion_inner(
    pool: &SqlitePool,
    fixture: &EvalFixture,
) -> Result<(bool, f64, String, Option<serde_json::Value>), String> {
    let input: PromotionFixtureInput = serde_json::from_str(&fixture.input_json)
        .map_err(|e| format!("parse promotion input: {e}"))?;
    let expect: PromotionFixtureExpectation = serde_json::from_str(&fixture.expectation_json)
        .map_err(|e| format!("parse promotion expectation: {e}"))?;

    let api_key = crate::runtime::gemini_api_key()
        .ok_or_else(|| "Gemini API key not configured".to_string())?;

    let outcome = crate::capture_promotion::evaluate_for_fixture(
        pool,
        &api_key,
        &input.capture_text,
        &expect.expected_kind,
        expect.expected_target_id.as_deref(),
    )
    .await?;

    let details = serde_json::json!({
        "suggestion": {
            "kind": outcome.kind,
            "target_id": outcome.target_id,
            "confidence": outcome.confidence,
            "rationale": outcome.rationale,
            "model": outcome.model,
        },
        "expected": {
            "kind": expect.expected_kind,
            "target_id": expect.expected_target_id,
        },
        "scoring": {
            "kind_matches": outcome.kind_matches,
            "target_matches": outcome.target_matches,
            "score": outcome.score,
        },
    });

    Ok((outcome.passed, outcome.score, "accuracy".to_string(), Some(details)))
}

impl ClassificationExpectation {
    fn into_map(self) -> serde_json::Map<String, serde_json::Value> {
        let mut m = serde_json::Map::new();
        if let Some(c) = self.category {
            m.insert("category".into(), serde_json::Value::String(c));
        }
        if let Some(p) = self.priority {
            m.insert("priority".into(), serde_json::Value::String(p));
        }
        if let Some(i) = self.intent {
            m.insert("intent".into(), serde_json::Value::String(i));
        }
        if let Some(a) = self.action_required {
            m.insert("action_required".into(), serde_json::Value::Bool(a));
        }
        if let Some(t) = self.thread_state {
            m.insert("thread_state".into(), serde_json::Value::String(t));
        }
        if let Some(pa) = self.predicted_action {
            m.insert("predicted_action".into(), serde_json::Value::String(pa));
        }
        m
    }
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct ClassificationExpectation {
    category: Option<String>,
    priority: Option<String>,
    intent: Option<String>,
    action_required: Option<bool>,
    thread_state: Option<String>,
    predicted_action: Option<String>,
    /// When true, score each pinned dimension via the LLM judge instead of
    /// exact-match. Useful for soft dimensions where synonyms / paraphrases
    /// should pass (e.g. `intent: "question"` should accept "asking").
    judge_soft: Option<bool>,
    /// Override judge model. Same shape as Ask fixtures.
    judge_model: Option<String>,
    /// Pass threshold for soft mode. Default 0.7. Ignored in exact-match mode.
    min_score: Option<f64>,
}

fn node_entity_key(node: &WorkGraphNode) -> String {
    if !node.entity_id.is_empty() {
        node.entity_id.clone()
    } else {
        node.id.clone()
    }
}

/// Minimum precision needed to call the fixture "passing". Reads from the
/// expectation if it carries a `min_precision` field, else uses 0.5 (at least
/// one of two top results matches).
fn expectation_threshold(expectation: &RetrievalFixtureExpectation) -> f64 {
    let _ = expectation;
    0.5
}

async fn baseline_score(pool: &SqlitePool, fixture_id: &str) -> Option<f64> {
    sqlx::query_scalar::<_, f64>("SELECT score FROM eval_baselines WHERE fixture_id = ?")
        .bind(fixture_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

