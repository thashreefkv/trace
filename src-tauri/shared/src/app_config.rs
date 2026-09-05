//! App-wide configuration: AI budget limits, alert thresholds, enforcement.
//!
//! State lives in the `app_config_settings` single-row table (migration 0046).
//! The chokepoint in `gemini::post_gemini` calls
//! [`block_message_if_budget_exceeded`] before each request — when the user
//! has opted into hard-blocking and the daily/monthly spend exceeds the
//! configured limit, that call returns a user-facing error message and the
//! request is skipped.

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AppConfig {
    pub budget_daily_usd: f64,
    pub budget_monthly_usd: f64,
    pub budget_alert_threshold_pct: f64,
    pub budget_block_when_exceeded: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            budget_daily_usd: 0.0,
            budget_monthly_usd: 0.0,
            budget_alert_threshold_pct: 80.0,
            budget_block_when_exceeded: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AlertState {
    #[default]
    Ok,
    Warning,
    Exceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BudgetStatus {
    pub daily_spent_usd: f64,
    pub monthly_spent_usd: f64,
    pub daily_limit_usd: f64,
    pub monthly_limit_usd: f64,
    pub daily_pct: f64,
    pub monthly_pct: f64,
    pub alert_threshold_pct: f64,
    pub alert_state: AlertState,
    pub block_active: bool,
}

pub async fn get_app_config(pool: &SqlitePool) -> Result<AppConfig, String> {
    let row: Option<(f64, f64, f64, i64)> = sqlx::query_as(
        "SELECT budget_daily_usd, budget_monthly_usd,
                budget_alert_threshold_pct, budget_block_when_exceeded
         FROM app_config_settings WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("app_config select: {e}"))?;

    Ok(match row {
        Some((daily, monthly, pct, block)) => AppConfig {
            budget_daily_usd: daily,
            budget_monthly_usd: monthly,
            budget_alert_threshold_pct: pct,
            budget_block_when_exceeded: block != 0,
        },
        None => AppConfig::default(),
    })
}

pub async fn set_app_config(pool: &SqlitePool, config: &AppConfig) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO app_config_settings
           (id, budget_daily_usd, budget_monthly_usd,
            budget_alert_threshold_pct, budget_block_when_exceeded, updated_at)
         VALUES (1, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET
           budget_daily_usd = excluded.budget_daily_usd,
           budget_monthly_usd = excluded.budget_monthly_usd,
           budget_alert_threshold_pct = excluded.budget_alert_threshold_pct,
           budget_block_when_exceeded = excluded.budget_block_when_exceeded,
           updated_at = datetime('now')",
    )
    .bind(config.budget_daily_usd)
    .bind(config.budget_monthly_usd)
    .bind(config.budget_alert_threshold_pct)
    .bind(if config.budget_block_when_exceeded { 1 } else { 0 })
    .execute(pool)
    .await
    .map_err(|e| format!("app_config upsert: {e}"))?;
    Ok(())
}

fn start_of_today_ms() -> i64 {
    let now = Utc::now();
    let date = NaiveDate::from_ymd_opt(now.year(), now.month(), now.day()).unwrap();
    Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .timestamp_millis()
}

fn start_of_month_ms() -> i64 {
    let now = Utc::now();
    let date = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();
    Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .timestamp_millis()
}

async fn sum_cost_since(pool: &SqlitePool, since_ms: i64) -> Result<f64, String> {
    let row: (f64,) = sqlx::query_as(
        "SELECT CAST(COALESCE(SUM(est_cost_usd), 0.0) AS REAL)
         FROM gemini_usage_log WHERE ts >= ?",
    )
    .bind(since_ms)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("sum cost: {e}"))?;
    Ok(row.0)
}

pub async fn check_budget(pool: &SqlitePool) -> Result<BudgetStatus, String> {
    let config = get_app_config(pool).await?;
    let daily_spent = sum_cost_since(pool, start_of_today_ms()).await?;
    let monthly_spent = sum_cost_since(pool, start_of_month_ms()).await?;

    let daily_pct = if config.budget_daily_usd > 0.0 {
        100.0 * daily_spent / config.budget_daily_usd
    } else {
        0.0
    };
    let monthly_pct = if config.budget_monthly_usd > 0.0 {
        100.0 * monthly_spent / config.budget_monthly_usd
    } else {
        0.0
    };

    let exceeded = (config.budget_daily_usd > 0.0 && daily_spent >= config.budget_daily_usd)
        || (config.budget_monthly_usd > 0.0 && monthly_spent >= config.budget_monthly_usd);
    let warning = !exceeded
        && ((config.budget_daily_usd > 0.0
            && daily_pct >= config.budget_alert_threshold_pct)
            || (config.budget_monthly_usd > 0.0
                && monthly_pct >= config.budget_alert_threshold_pct));

    let alert_state = if exceeded {
        AlertState::Exceeded
    } else if warning {
        AlertState::Warning
    } else {
        AlertState::Ok
    };

    Ok(BudgetStatus {
        daily_spent_usd: daily_spent,
        monthly_spent_usd: monthly_spent,
        daily_limit_usd: config.budget_daily_usd,
        monthly_limit_usd: config.budget_monthly_usd,
        daily_pct,
        monthly_pct,
        alert_threshold_pct: config.budget_alert_threshold_pct,
        alert_state,
        block_active: exceeded && config.budget_block_when_exceeded,
    })
}

/// Called by `gemini::post_gemini` before sending a request. Returns the
/// user-facing block message if the budget is exceeded AND the user has
/// opted into hard-blocking. Returns `None` otherwise (the common case).
///
/// Errors querying the DB are swallowed and treated as "not blocked" — we
/// never want a config-table query failure to silently halt AI features.
pub async fn block_message_if_budget_exceeded(pool: &SqlitePool) -> Option<String> {
    let status = match check_budget(pool).await {
        Ok(s) => s,
        Err(_) => return None,
    };
    if !status.block_active {
        return None;
    }

    let scope = if status.daily_limit_usd > 0.0 && status.daily_spent_usd >= status.daily_limit_usd
    {
        format!(
            "daily limit ${:.2} reached (spent ${:.2})",
            status.daily_limit_usd, status.daily_spent_usd
        )
    } else {
        format!(
            "monthly limit ${:.2} reached (spent ${:.2})",
            status.monthly_limit_usd, status.monthly_spent_usd
        )
    };
    Some(format!(
        "AI calls blocked: {scope}. Raise the limit in Settings to resume."
    ))
}
