//! App-wide configuration commands: AI budget + alert thresholds.
//!
//! Also owns the periodic budget checker that watches `gemini_usage_log`
//! totals and emits `budget:event` toasts when daily/monthly spend crosses
//! the warning or exceeded threshold.

use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex as AsyncMutex;

use project_manager_shared::app_config::{self, AlertState, AppConfig, BudgetStatus};

use crate::db::AppState;

#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    app_config::get_app_config(&state.pool).await
}

#[tauri::command]
pub async fn set_app_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    app_config::set_app_config(&state.pool, &config).await
}

#[tauri::command]
pub async fn get_budget_status(state: State<'_, AppState>) -> Result<BudgetStatus, String> {
    app_config::check_budget(&state.pool).await
}

#[tauri::command]
pub async fn get_gemini_daily_trend(
    state: State<'_, AppState>,
    days: i64,
) -> Result<project_manager_shared::gemini_usage::DailyTrend, String> {
    project_manager_shared::gemini_usage::summary_for_days(&state.pool, days).await
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
struct BudgetEvent {
    kind: &'static str,  // "warning" | "exceeded" | "recovery"
    scope: &'static str, // "daily" | "monthly"
    spent_usd: f64,
    limit_usd: f64,
    threshold_pct: f64,
    block_active: bool,
}

#[derive(Debug, Default)]
struct AlertTracker {
    daily: AlertState,
    monthly: AlertState,
}

/// Spawn a 60s loop that checks the budget and emits `budget:event` on
/// state transitions. Recoverable from any DB error — we never want this
/// task to die silently.
pub fn spawn_budget_monitor(app: AppHandle, pool: SqlitePool) {
    let tracker = Arc::new(AsyncMutex::new(AlertTracker::default()));
    tauri::async_runtime::spawn(async move {
        // Brief delay so startup migrations and other setup work complete
        // before we issue the first SELECT.
        tokio::time::sleep(Duration::from_secs(5)).await;
        loop {
            if let Ok(status) = app_config::check_budget(&pool).await {
                tick(&app, &tracker, &status).await;
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

async fn tick(app: &AppHandle, tracker: &AsyncMutex<AlertTracker>, status: &BudgetStatus) {
    let mut guard = tracker.lock().await;
    maybe_emit(
        app,
        "daily",
        status.daily_limit_usd,
        status.daily_spent_usd,
        &mut guard.daily,
        compute_state(
            status.daily_limit_usd,
            status.daily_spent_usd,
            status.alert_threshold_pct,
        ),
        status.alert_threshold_pct,
        status.block_active,
    );
    maybe_emit(
        app,
        "monthly",
        status.monthly_limit_usd,
        status.monthly_spent_usd,
        &mut guard.monthly,
        compute_state(
            status.monthly_limit_usd,
            status.monthly_spent_usd,
            status.alert_threshold_pct,
        ),
        status.alert_threshold_pct,
        status.block_active,
    );
}

fn compute_state(limit: f64, spent: f64, threshold_pct: f64) -> AlertState {
    if limit <= 0.0 {
        return AlertState::Ok;
    }
    if spent >= limit {
        AlertState::Exceeded
    } else if spent >= limit * threshold_pct / 100.0 {
        AlertState::Warning
    } else {
        AlertState::Ok
    }
}

#[allow(clippy::too_many_arguments)]
fn maybe_emit(
    app: &AppHandle,
    scope: &'static str,
    limit: f64,
    spent: f64,
    last: &mut AlertState,
    next: AlertState,
    threshold_pct: f64,
    block_active: bool,
) {
    if *last == next {
        return;
    }
    let kind = match (&*last, &next) {
        (_, AlertState::Warning) => "warning",
        (_, AlertState::Exceeded) => "exceeded",
        (AlertState::Warning | AlertState::Exceeded, AlertState::Ok) => "recovery",
        _ => {
            *last = next;
            return;
        }
    };
    let _ = app.emit(
        "budget:event",
        BudgetEvent {
            kind,
            scope,
            spent_usd: spent,
            limit_usd: limit,
            threshold_pct,
            block_active,
        },
    );
    *last = next;
}
