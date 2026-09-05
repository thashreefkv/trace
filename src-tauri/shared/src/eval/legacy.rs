//! Eval harness for retrieval, Ask answers, classification, and promotion.
//!
//! Each *fixture* is a labelled scenario: an input + the expected outcome.
//! A *run* executes the current code against the fixture and scores the
//! result. Scores roll up into a baseline so we can detect regressions when
//! changing prompts, retrieval, or models.
//!
//! - Retrieval evals compare ranked node IDs from `brain::retrieve_brain_context`
//!   against an expected top-K list (precision@3, hit@3).
//! - Ask evals use an LLM judge (more capable than the model under test) to
//!   score answer quality against a rubric. Wired but disabled by default to
//!   avoid surprise Gemini cost.
//! - Classification + promotion evals are scaffolded; implementations land
//!   alongside Sections 3 and 4.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::fixtures::list_fixtures;
use super::runners::run_fixture;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureKind {
    Retrieval,
    Ask,
    Classification,
    Promotion,
}

impl FixtureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FixtureKind::Retrieval => "retrieval",
            FixtureKind::Ask => "ask",
            FixtureKind::Classification => "classification",
            FixtureKind::Promotion => "promotion",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "retrieval" => Some(Self::Retrieval),
            "ask" => Some(Self::Ask),
            "classification" => Some(Self::Classification),
            "promotion" => Some(Self::Promotion),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalFixture {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub input_json: String,
    pub expectation_json: String,
    pub notes: Option<String>,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRun {
    pub id: String,
    pub fixture_id: String,
    pub ts: i64,
    pub passed: bool,
    pub score: f64,
    pub metric: String,
    pub details_json: Option<String>,
    pub latency_ms: i64,
    pub baseline_score: Option<f64>,
    pub delta: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSummary {
    pub total: i64,
    pub passed: i64,
    pub failed: i64,
    pub avg_score: f64,
    pub by_kind: Vec<EvalKindBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalKindBreakdown {
    pub kind: String,
    pub count: i64,
    pub passed: i64,
    pub avg_score: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetrievalFixtureInput {
    pub query: String,
    #[serde(default)]
    pub focus_entity_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetrievalFixtureExpectation {
    /// Expected entity_id values that must appear in the top-K result list.
    pub expected_entity_ids: Vec<String>,
    /// K for precision@K (defaults to 3).
    #[serde(default)]
    pub top_k: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AskFixtureInput {
    pub question: String,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AskFixtureExpectation {
    /// Plain-English facts the rubric expects to find in the answer.
    /// Each becomes part of the judge prompt.
    pub expected_facts: Vec<String>,
    /// Entity kinds the answer should cite (e.g. ["deliverable", "stakeholder"]).
    #[serde(default)]
    pub expected_citation_kinds: Vec<String>,
    /// Specific entity_ids that must be cited. Stricter than kinds.
    #[serde(default)]
    pub expected_citation_ids: Vec<String>,
    /// Aggregate threshold for `passed = true`. Defaults to 0.7.
    #[serde(default)]
    pub min_aggregate_score: Option<f64>,
    /// Override judge model: "pro" (default) or "flash".
    #[serde(default)]
    pub judge_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromotionFixtureInput {
    pub capture_text: String,
    /// Optional: pre-existing capture id. When absent, the runner will
    /// create an ephemeral capture once Section 4's suggester ships.
    #[serde(default)]
    pub capture_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromotionFixtureExpectation {
    /// One of "task" | "deliverable" | "initiative".
    pub expected_kind: String,
    /// If set, the suggested target_id must match (stronger signal).
    #[serde(default)]
    pub expected_target_id: Option<String>,
}

pub async fn run_all(
    pool: &SqlitePool,
    brain_path: &Path,
) -> Result<Vec<EvalRun>, String> {
    let fixtures = list_fixtures(pool).await?;
    let mut runs = Vec::with_capacity(fixtures.len());
    for fixture in fixtures.iter().filter(|f| f.enabled) {
        match run_fixture(pool, brain_path, fixture).await {
            Ok(run) => runs.push(run),
            Err(error) => {
                eprintln!("[eval] fixture {} failed: {error}", fixture.name);
            }
        }
    }
    Ok(runs)
}

pub async fn list_runs_for_fixture(
    pool: &SqlitePool,
    fixture_id: &str,
    limit: i64,
) -> Result<Vec<EvalRun>, String> {
    sqlx::query_as::<_, EvalRunRow>(
        "SELECT id, fixture_id, ts, passed, score, metric, details_json,
                latency_ms, baseline_score, delta
           FROM eval_runs
          WHERE fixture_id = ?
          ORDER BY ts DESC
          LIMIT ?",
    )
    .bind(fixture_id)
    .bind(limit.clamp(1, 200))
    .fetch_all(pool)
    .await
    .map(|rows| rows.into_iter().map(EvalRun::from).collect())
    .map_err(|e| format!("list runs: {e}"))
}

pub async fn latest_runs(pool: &SqlitePool, limit: i64) -> Result<Vec<EvalRun>, String> {
    sqlx::query_as::<_, EvalRunRow>(
        "SELECT id, fixture_id, ts, passed, score, metric, details_json,
                latency_ms, baseline_score, delta
           FROM eval_runs
          ORDER BY ts DESC
          LIMIT ?",
    )
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await
    .map(|rows| rows.into_iter().map(EvalRun::from).collect())
    .map_err(|e| format!("latest runs: {e}"))
}

pub async fn set_baseline(pool: &SqlitePool, fixture_id: &str, run_id: &str) -> Result<(), String> {
    let score = sqlx::query_scalar::<_, f64>("SELECT score FROM eval_runs WHERE id = ?")
        .bind(run_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("fetch run score: {e}"))?;
    let ts = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO eval_baselines (fixture_id, score, set_at, run_id)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(fixture_id) DO UPDATE SET
           score = excluded.score,
           set_at = excluded.set_at,
           run_id = excluded.run_id",
    )
    .bind(fixture_id)
    .bind(score)
    .bind(ts)
    .bind(run_id)
    .execute(pool)
    .await
    .map_err(|e| format!("set baseline: {e}"))?;
    Ok(())
}

pub async fn summary(pool: &SqlitePool) -> Result<EvalSummary, String> {
    let fixtures = list_fixtures(pool).await?;
    if fixtures.is_empty() {
        return Ok(EvalSummary {
            total: 0,
            passed: 0,
            failed: 0,
            avg_score: 0.0,
            by_kind: Vec::new(),
        });
    }
    let latest_per_fixture = latest_per_fixture(pool).await?;
    let mut total = 0_i64;
    let mut passed = 0_i64;
    let mut score_sum = 0.0_f64;
    let mut by_kind: std::collections::BTreeMap<String, (i64, i64, f64)> =
        std::collections::BTreeMap::new();

    for fixture in &fixtures {
        let Some(run) = latest_per_fixture.get(&fixture.id) else {
            continue;
        };
        total += 1;
        if run.passed {
            passed += 1;
        }
        score_sum += run.score;
        let entry = by_kind.entry(fixture.kind.clone()).or_insert((0, 0, 0.0));
        entry.0 += 1;
        if run.passed {
            entry.1 += 1;
        }
        entry.2 += run.score;
    }

    let avg_score = if total > 0 { score_sum / total as f64 } else { 0.0 };
    let kind_rows = by_kind
        .into_iter()
        .map(|(kind, (count, passed, score_sum))| EvalKindBreakdown {
            kind,
            count,
            passed,
            avg_score: if count > 0 { score_sum / count as f64 } else { 0.0 },
        })
        .collect();

    Ok(EvalSummary {
        total,
        passed,
        failed: total - passed,
        avg_score,
        by_kind: kind_rows,
    })
}

async fn latest_per_fixture(
    pool: &SqlitePool,
) -> Result<std::collections::HashMap<String, EvalRun>, String> {
    let rows: Vec<EvalRunRow> = sqlx::query_as(
        "SELECT r.id, r.fixture_id, r.ts, r.passed, r.score, r.metric,
                r.details_json, r.latency_ms, r.baseline_score, r.delta
           FROM eval_runs r
           JOIN (
             SELECT fixture_id, MAX(ts) AS ts
               FROM eval_runs
              GROUP BY fixture_id
           ) latest ON latest.fixture_id = r.fixture_id AND latest.ts = r.ts",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("latest per fixture: {e}"))?;

    let mut map = std::collections::HashMap::new();
    for row in rows {
        let run = EvalRun::from(row);
        map.insert(run.fixture_id.clone(), run);
    }
    Ok(map)
}

#[derive(sqlx::FromRow)]
struct EvalRunRow {
    id: String,
    fixture_id: String,
    ts: i64,
    passed: i64,
    score: f64,
    metric: String,
    details_json: Option<String>,
    latency_ms: i64,
    baseline_score: Option<f64>,
    delta: Option<f64>,
}

impl From<EvalRunRow> for EvalRun {
    fn from(row: EvalRunRow) -> Self {
        EvalRun {
            id: row.id,
            fixture_id: row.fixture_id,
            ts: row.ts,
            passed: row.passed != 0,
            score: row.score,
            metric: row.metric,
            details_json: row.details_json,
            latency_ms: row.latency_ms,
            baseline_score: row.baseline_score,
            delta: row.delta,
        }
    }
}
