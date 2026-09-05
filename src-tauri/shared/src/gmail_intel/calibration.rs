//! Isotonic calibration report + PAV regression. Extracted from legacy.rs (13-std2).

use serde::Serialize;
use sqlx::SqlitePool;


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CalibrationReport {
    pub total_overrides: i64,
    pub by_dimension: Vec<CalibrationByDimension>,
    pub note: String,
    /// Per-dimension isotonic calibration curve: for each bucket of the
    /// classifier's stated confidence, the observed accept rate. Populated
    /// only when `total_overrides >= ISOTONIC_MIN_SAMPLES` so the PAV
    /// curve doesn't overfit small-N noise.
    pub isotonic: Vec<IsotonicDimensionCurve>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct IsotonicDimensionCurve {
    pub dimension: String,
    pub sample_count: i64,
    pub points: Vec<IsotonicPoint>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct IsotonicPoint {
    /// Mean stated confidence of samples in this bucket.
    pub confidence: f64,
    /// Observed accept rate after PAV monotonic regression.
    pub accuracy: f64,
    /// Raw observed accept rate (pre-PAV) for transparency.
    pub raw_accuracy: f64,
    /// How many user-classification samples backed this bucket.
    pub samples: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CalibrationByDimension {
    pub dimension: String,
    pub original: String,
    pub corrected: String,
    pub count: i64,
    /// Wilson 95% CI for the rate that "LLM said X → user corrected to Y" out
    /// of total times the LLM said X. Lets us report uncertainty honestly on
    /// small samples instead of pretending a 3/4 ratio is calibrated.
    pub rate_lo: f64,
    pub rate_hi: f64,
    pub rate: f64,
}

/// Aggregate the override stream into a simple precision view per dimension.
/// Each override is paired with what the LLM said at the time to compute
/// directional bias ("LLM said work, you said personal — N times").
pub async fn calibration_report(pool: &SqlitePool) -> Result<CalibrationReport, String> {
    let total: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM gmail_user_classifications")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    // For each dimension, fetch (original, corrected, count) and the total
    // number of times the LLM emitted that original value, then compute a
    // Wilson CI on the misclassification rate.
    let mut by_dimension: Vec<CalibrationByDimension> = Vec::new();
    by_dimension.extend(
        collect_dimension(pool, "ai_category", "category", "u.category").await,
    );
    by_dimension.extend(
        collect_dimension(pool, "ai_priority", "priority", "u.priority").await,
    );
    by_dimension.extend(
        collect_dimension(pool, "intent", "intent", "u.intent").await,
    );
    by_dimension.extend(
        collect_dimension(pool, "thread_state", "thread_state", "u.thread_state").await,
    );

    let isotonic = if total >= ISOTONIC_MIN_SAMPLES {
        build_isotonic_curves(pool).await
    } else {
        Vec::new()
    };

    let note = if total < 30 {
        "Need at least ~30 corrections before calibration is meaningful.".to_string()
    } else if total < ISOTONIC_MIN_SAMPLES {
        format!(
            "Wilson CI shown. Isotonic calibration curve unlocks at {ISOTONIC_MIN_SAMPLES}+ corrections \
             (currently {total}) — below that, PAV bins overfit and look more confident than they should."
        )
    } else {
        "Frequent corrections suggest where the classifier is systematically biased. \
         Isotonic curves below show observed accept rate per confidence bucket."
            .to_string()
    };

    Ok(CalibrationReport {
        total_overrides: total,
        by_dimension,
        note,
        isotonic,
    })
}

const ISOTONIC_MIN_SAMPLES: i64 = 100;
const ISOTONIC_BUCKETS: usize = 10;

/// Build a PAV-monotonized calibration curve per dimension. Each dimension's
/// curve is bucketed by stated confidence (from `gmail_threads.dimensions_confidence_json`)
/// and the bucket's observed accept rate is computed by comparing the LLM's
/// stored value against the user's override (override present + same value =
/// "accept", override present + different = "reject"; threads with no override
/// don't contribute since we lack a ground-truth signal there).
async fn build_isotonic_curves(pool: &SqlitePool) -> Vec<IsotonicDimensionCurve> {
    let mut out = Vec::new();
    for (key, label, thread_col, user_expr, is_bool) in &[
        ("category", "category", "ai_category", "u.category", false),
        ("priority", "priority", "ai_priority", "u.priority", false),
        ("intent", "intent", "intent", "u.intent", false),
        (
            "action_required",
            "action_required",
            "action_required",
            "u.action_required",
            true,
        ),
        (
            "thread_state",
            "thread_state",
            "thread_state",
            "u.thread_state",
            false,
        ),
    ] {
        if let Some(curve) =
            isotonic_curve_for_dimension(pool, key, label, thread_col, user_expr, *is_bool).await
        {
            out.push(curve);
        }
    }
    out
}

async fn isotonic_curve_for_dimension(
    pool: &SqlitePool,
    confidence_key: &str,
    dim_label: &str,
    thread_col: &str,
    user_expr: &str,
    is_bool: bool,
) -> Option<IsotonicDimensionCurve> {
    // Pull every user-override paired with the LLM's value + the per-dim
    // confidence (from dimensions_confidence_json). Threads with no
    // confidence entry are skipped — we can't bucket them.
    let sql = format!(
        "SELECT
            COALESCE(CAST(t.{col} AS TEXT), ''),
            COALESCE(CAST({user} AS TEXT), ''),
            t.dimensions_confidence_json
        FROM gmail_user_classifications u
        JOIN gmail_threads t ON t.thread_id = u.thread_id
        WHERE {user} IS NOT NULL",
        col = thread_col,
        user = user_expr
    );
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as(&sql).fetch_all(pool).await.unwrap_or_default();

    let mut buckets: Vec<(Vec<f64>, Vec<bool>)> =
        (0..ISOTONIC_BUCKETS).map(|_| (Vec::new(), Vec::new())).collect();

    let mut total_samples = 0_i64;
    for (llm_value, user_value, conf_json) in rows {
        let Some(raw) = conf_json else { continue };
        let parsed: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let confidence = parsed
            .get(confidence_key)
            .and_then(|v| v.as_f64())
            .unwrap_or(-1.0);
        if !(0.0..=1.0).contains(&confidence) {
            continue;
        }
        let accepted = if is_bool {
            // For booleans the user value comes through as "0"/"1" or
            // "true"/"false"; normalize on both sides.
            normalize_bool_str(&llm_value) == normalize_bool_str(&user_value)
        } else {
            !llm_value.is_empty() && llm_value.eq_ignore_ascii_case(&user_value)
        };
        let idx = bucket_index_for(confidence);
        buckets[idx].0.push(confidence);
        buckets[idx].1.push(accepted);
        total_samples += 1;
    }

    if total_samples == 0 {
        return None;
    }

    // Raw per-bucket accept rate.
    let mut raw_points: Vec<(f64, f64, i64)> = Vec::new();
    for (confs, accepts) in &buckets {
        if confs.is_empty() {
            continue;
        }
        let mean_conf = confs.iter().sum::<f64>() / confs.len() as f64;
        let positives = accepts.iter().filter(|a| **a).count() as i64;
        let total = accepts.len() as i64;
        let raw = positives as f64 / total as f64;
        raw_points.push((mean_conf, raw, total));
    }
    raw_points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Pool Adjacent Violators (PAV) — enforce monotonic non-decreasing
    // accept rate as confidence rises. Weights = bucket sample counts.
    let isotonic = pool_adjacent_violators(&raw_points);

    let points = raw_points
        .into_iter()
        .zip(isotonic)
        .map(|((confidence, raw_accuracy, samples), accuracy)| IsotonicPoint {
            confidence,
            accuracy,
            raw_accuracy,
            samples,
        })
        .collect();

    Some(IsotonicDimensionCurve {
        dimension: dim_label.to_string(),
        sample_count: total_samples,
        points,
    })
}

fn bucket_index_for(confidence: f64) -> usize {
    let clamped = confidence.clamp(0.0, 0.9999);
    (clamped * ISOTONIC_BUCKETS as f64).floor() as usize
}

fn normalize_bool_str(value: &str) -> &str {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "t" | "yes" => "true",
        "0" | "false" | "f" | "no" | "" => "false",
        _ => value,
    }
}

/// Pool Adjacent Violators (PAV) algorithm. Input: sorted (x, y, weight)
/// triples; output: monotonically non-decreasing y' values aligned with
/// the input order. Standard implementation — merges adjacent buckets
/// whose means would decrease, weighting by sample count.
fn pool_adjacent_violators(points: &[(f64, f64, i64)]) -> Vec<f64> {
    if points.is_empty() {
        return Vec::new();
    }
    // Stack of pools: (sum_weighted_y, total_weight, span_len, start_index).
    let mut stack: Vec<(f64, f64, usize, usize)> = Vec::new();
    for (idx, (_, y, w)) in points.iter().enumerate() {
        let mut current = (y * (*w as f64), *w as f64, 1, idx);
        while let Some(prev) = stack.last() {
            if prev.0 / prev.1 > current.0 / current.1 {
                let merged = (
                    prev.0 + current.0,
                    prev.1 + current.1,
                    prev.2 + current.2,
                    prev.3,
                );
                stack.pop();
                current = merged;
            } else {
                break;
            }
        }
        stack.push(current);
    }
    let mut out = vec![0.0; points.len()];
    for (sum, weight, span, start) in stack {
        let mean = if weight > 0.0 { sum / weight } else { 0.0 };
        for i in start..start + span {
            out[i] = mean;
        }
    }
    out
}

async fn collect_dimension(
    pool: &SqlitePool,
    thread_column: &str,
    dim_label: &str,
    user_expr: &str,
) -> Vec<CalibrationByDimension> {
    // Number of times the LLM emitted each original value (denominator).
    let totals: Vec<(String, i64)> = sqlx::query_as(&format!(
        "SELECT COALESCE(CAST(t.{col} AS TEXT), ''), COUNT(*)
           FROM gmail_threads t
          WHERE t.{col} IS NOT NULL
          GROUP BY t.{col}",
        col = thread_column
    ))
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let total_by_original: std::collections::HashMap<String, i64> =
        totals.into_iter().collect();

    let pairs: Vec<(String, String, i64)> = sqlx::query_as(&format!(
        "SELECT COALESCE(CAST(t.{col} AS TEXT), ''),
                COALESCE(CAST({user} AS TEXT), CAST(t.{col} AS TEXT)),
                COUNT(*)
           FROM gmail_user_classifications u
           JOIN gmail_threads t ON t.thread_id = u.thread_id
          WHERE {user} IS NOT NULL AND CAST({user} AS TEXT) != CAST(t.{col} AS TEXT)
          GROUP BY t.{col}, {user}
          ORDER BY COUNT(*) DESC
          LIMIT 20",
        col = thread_column,
        user = user_expr
    ))
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    pairs
        .into_iter()
        .map(|(orig, corrected, count)| {
            let denom = total_by_original
                .get(&orig)
                .copied()
                .unwrap_or(count)
                .max(count);
            let (lo, p, hi) = wilson_interval(count, denom);
            CalibrationByDimension {
                dimension: dim_label.to_string(),
                original: orig,
                corrected,
                count,
                rate: p,
                rate_lo: lo,
                rate_hi: hi,
            }
        })
        .collect()
}

/// Wilson score interval (95% CI) for a binomial proportion. Stable on small
/// N where normal-approx breaks down.
fn wilson_interval(successes: i64, trials: i64) -> (f64, f64, f64) {
    if trials <= 0 {
        return (0.0, 0.0, 0.0);
    }
    let n = trials as f64;
    let p = (successes as f64) / n;
    let z = 1.96_f64; // 95% CI
    let denom = 1.0 + z * z / n;
    let centre = p + z * z / (2.0 * n);
    let margin = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt());
    let lo = ((centre - margin) / denom).clamp(0.0, 1.0);
    let hi = ((centre + margin) / denom).clamp(0.0, 1.0);
    (lo, p, hi)
}
