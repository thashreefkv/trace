//! Structured events emitted on the `report:event` Tauri channel as a report
//! generation pipeline runs. The frontend subscribes via
//! `listen<ReportEvent>("report:event")` and updates the StepTimeline live.
//!
//! Mirrors the AskRunEvent pattern in `gemini/streaming.rs`.

use serde::{Deserialize, Serialize};

use crate::models::ReportClarification;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReportEvent {
    RunStarted {
        report_run_id: String,
    },
    StepCreated {
        report_run_id: String,
        step_id: String,
        step_name: String,
        section_index: Option<i64>,
        section_id: Option<String>,
    },
    StepStarted {
        step_id: String,
        report_run_id: String,
        step_name: String,
        ticker_label: Option<String>,
    },
    StepProgress {
        step_id: String,
        ticker_label: String,
        detail: Option<String>,
    },
    SectionDelta {
        step_id: String,
        section_id: String,
        text: String,
    },
    StepNeedsClarification {
        step_id: String,
        report_run_id: String,
        prompt: ReportClarification,
    },
    StepCompleted {
        step_id: String,
        report_run_id: String,
        step_name: String,
        output_summary: Option<String>,
        latency_ms: Option<i64>,
        model: Option<String>,
        cache_hit: bool,
    },
    StepFailed {
        step_id: String,
        report_run_id: String,
        step_name: String,
        error: String,
    },
    RunFinished {
        report_run_id: String,
        status: String,
    },
}

impl ReportEvent {
    pub const CHANNEL: &'static str = "report:event";
}
