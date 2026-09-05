//! Gemini API usage + cost tracking.
//!
//! Every Gemini call passes through `record()` so the user can see a
//! per-feature, per-model breakdown of tokens, cache hit rate, and estimated
//! cost. Pricing constants are approximations based on public rates for
//! current Gemini Flash / Pro / embedding tiers; users should treat the cost
//! column as a rough guide, not a billing source of truth.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Default)]
pub struct UsageMetadata {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub cached_tokens: i64,
    pub total_tokens: i64,
}

/// Extract `usageMetadata` from a Gemini API response.
pub fn parse_from_response(response: &serde_json::Value) -> UsageMetadata {
    let meta = response.get("usageMetadata");
    let prompt = meta
        .and_then(|m| m.get("promptTokenCount"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let completion = meta
        .and_then(|m| m.get("candidatesTokenCount"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cached = meta
        .and_then(|m| m.get("cachedContentTokenCount"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let total = meta
        .and_then(|m| m.get("totalTokenCount"))
        .and_then(|v| v.as_i64())
        .unwrap_or(prompt + completion);
    UsageMetadata {
        prompt_tokens: prompt,
        completion_tokens: completion,
        cached_tokens: cached,
        total_tokens: total,
    }
}

/// Approximate $/1M-token pricing for (uncached prompt, cached prompt, completion).
fn pricing_for_model(model: &str) -> (f64, f64, f64) {
    let m = model.to_ascii_lowercase();
    if m.contains("pro") {
        (1.25, 0.3125, 5.0)
    } else if m.contains("flash") {
        (0.075, 0.01875, 0.30)
    } else if m.contains("embedding") {
        (0.025, 0.025, 0.0)
    } else {
        (0.10, 0.025, 0.40)
    }
}

pub fn estimate_cost_usd(model: &str, usage: &UsageMetadata) -> f64 {
    let uncached_prompt = (usage.prompt_tokens - usage.cached_tokens).max(0) as f64;
    let cached = usage.cached_tokens as f64;
    let completion = usage.completion_tokens as f64;
    let (in_rate, cached_rate, out_rate) = pricing_for_model(model);
    (uncached_prompt * in_rate + cached * cached_rate + completion * out_rate) / 1_000_000.0
}

pub async fn record(
    pool: &SqlitePool,
    feature: &str,
    model: &str,
    usage: &UsageMetadata,
    latency_ms: i64,
    error: Option<&str>,
) {
    let id = ulid::Ulid::new().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    let cost = estimate_cost_usd(model, usage);
    let result = sqlx::query(
        "INSERT INTO gemini_usage_log
           (id, ts, feature, model, prompt_tokens, completion_tokens,
            cached_tokens, total_tokens, est_cost_usd, latency_ms, error)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(ts)
    .bind(feature)
    .bind(model)
    .bind(usage.prompt_tokens)
    .bind(usage.completion_tokens)
    .bind(usage.cached_tokens)
    .bind(usage.total_tokens)
    .bind(cost)
    .bind(latency_ms)
    .bind(error)
    .execute(pool)
    .await;
    if let Err(error) = result {
        eprintln!("[gemini_usage] failed to record: {error}");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UsageByFeature {
    pub feature: String,
    pub calls: i64,
    pub total_tokens: i64,
    pub cached_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UsageByModel {
    pub model: String,
    pub calls: i64,
    pub total_tokens: i64,
    pub cached_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UsageSummary {
    pub period_hours: i64,
    pub total_calls: i64,
    pub error_calls: i64,
    pub total_tokens: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_cached_tokens: i64,
    pub total_cost_usd: f64,
    pub by_feature: Vec<UsageByFeature>,
    pub by_model: Vec<UsageByModel>,
}

pub async fn summary_for_hours(pool: &SqlitePool, hours: i64) -> Result<UsageSummary, String> {
    let cutoff = chrono::Utc::now().timestamp_millis() - hours * 60 * 60 * 1000;

    // Note: every numeric fallback must use 0.0 (not 0) so SQLite reports
    // the column as REAL. Integer fallbacks make sqlx reject the f64 decode
    // with "Rust type f64 ... is not compatible with SQL type INTEGER".
    let totals: (i64, i64, i64, i64, i64, i64, f64) = sqlx::query_as(
        "SELECT
           COUNT(*),
           SUM(CASE WHEN error IS NOT NULL THEN 1 ELSE 0 END),
           CAST(COALESCE(SUM(prompt_tokens), 0) AS INTEGER),
           CAST(COALESCE(SUM(completion_tokens), 0) AS INTEGER),
           CAST(COALESCE(SUM(cached_tokens), 0) AS INTEGER),
           CAST(COALESCE(SUM(total_tokens), 0) AS INTEGER),
           CAST(COALESCE(SUM(est_cost_usd), 0.0) AS REAL)
         FROM gemini_usage_log
         WHERE ts >= ?",
    )
    .bind(cutoff)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("usage totals: {e}"))?;

    let feature_rows: Vec<(String, i64, i64, i64, f64)> = sqlx::query_as(
        "SELECT feature, COUNT(*),
                CAST(COALESCE(SUM(total_tokens), 0) AS INTEGER),
                CAST(COALESCE(SUM(cached_tokens), 0) AS INTEGER),
                CAST(COALESCE(SUM(est_cost_usd), 0.0) AS REAL)
         FROM gemini_usage_log
         WHERE ts >= ?
         GROUP BY feature
         ORDER BY COALESCE(SUM(est_cost_usd), 0.0) DESC",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("usage by feature: {e}"))?;

    let model_rows: Vec<(String, i64, i64, i64, f64)> = sqlx::query_as(
        "SELECT model, COUNT(*),
                CAST(COALESCE(SUM(total_tokens), 0) AS INTEGER),
                CAST(COALESCE(SUM(cached_tokens), 0) AS INTEGER),
                CAST(COALESCE(SUM(est_cost_usd), 0.0) AS REAL)
         FROM gemini_usage_log
         WHERE ts >= ?
         GROUP BY model
         ORDER BY COALESCE(SUM(est_cost_usd), 0.0) DESC",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("usage by model: {e}"))?;

    Ok(UsageSummary {
        period_hours: hours,
        total_calls: totals.0,
        error_calls: totals.1,
        total_prompt_tokens: totals.2,
        total_completion_tokens: totals.3,
        total_cached_tokens: totals.4,
        total_tokens: totals.5,
        total_cost_usd: totals.6,
        by_feature: feature_rows
            .into_iter()
            .map(|(feature, calls, total_tokens, cached_tokens, cost_usd)| UsageByFeature {
                feature,
                calls,
                total_tokens,
                cached_tokens,
                cost_usd,
            })
            .collect(),
        by_model: model_rows
            .into_iter()
            .map(|(model, calls, total_tokens, cached_tokens, cost_usd)| UsageByModel {
                model,
                calls,
                total_tokens,
                cached_tokens,
                cost_usd,
            })
            .collect(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DailyBucket {
    pub date: String,
    pub calls: i64,
    pub tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DailyTrend {
    pub period_days: i64,
    pub total_cost_usd: f64,
    pub buckets: Vec<DailyBucket>,
}

/// Per-day Gemini cost over the last `days` days. The result contains one
/// bucket per UTC day with at least one logged call within the window.
/// Days with zero calls are omitted; the frontend chart fills gaps with
/// zero values to keep the x-axis continuous.
pub async fn summary_for_days(pool: &SqlitePool, days: i64) -> Result<DailyTrend, String> {
    let days = days.max(1);
    let cutoff = chrono::Utc::now().timestamp_millis() - days * 24 * 60 * 60 * 1000;

    let rows: Vec<(String, i64, i64, f64)> = sqlx::query_as(
        "SELECT
            date(datetime(ts / 1000, 'unixepoch')) AS bucket,
            COUNT(*),
            CAST(COALESCE(SUM(total_tokens), 0) AS INTEGER),
            CAST(COALESCE(SUM(est_cost_usd), 0.0) AS REAL)
         FROM gemini_usage_log
         WHERE ts >= ?
         GROUP BY bucket
         ORDER BY bucket ASC",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("usage daily trend: {e}"))?;

    let total_cost_usd = rows.iter().map(|r| r.3).sum::<f64>();
    let buckets = rows
        .into_iter()
        .map(|(date, calls, tokens, cost_usd)| DailyBucket {
            date,
            calls,
            tokens,
            cost_usd,
        })
        .collect();

    Ok(DailyTrend {
        period_days: days,
        total_cost_usd,
        buckets,
    })
}
