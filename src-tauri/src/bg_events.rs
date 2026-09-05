//! Unified background-task event channel.
//!
//! All long-running background jobs (Gmail sync, Drive sync, Calendar sync,
//! Brain rebuild, etc.) emit lifecycle events on the `bg:event` channel so the
//! UI can show a single "background tasks" indicator regardless of source.
//!
//! Source-specific events (e.g. `gmail:new-mail`) continue to be emitted for
//! native notifications and route-specific reactions.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BgEvent {
    pub source: &'static str,
    pub kind: BgEventKind,
    pub at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum BgEventKind {
    Started,
    Finished,
    Error,
}

pub const SOURCE_GMAIL: &str = "gmail";
pub const SOURCE_DRIVE: &str = "drive";
pub const SOURCE_CALENDAR: &str = "calendar";
pub const SOURCE_BRAIN: &str = "brain";

pub fn emit_started(app: &AppHandle, source: &'static str) {
    let _ = app.emit(
        "bg:event",
        BgEvent {
            source,
            kind: BgEventKind::Started,
            at: now_ms(),
            summary: None,
            error: None,
        },
    );
}

pub fn emit_finished(app: &AppHandle, source: &'static str, summary: Option<String>) {
    let _ = app.emit(
        "bg:event",
        BgEvent {
            source,
            kind: BgEventKind::Finished,
            at: now_ms(),
            summary,
            error: None,
        },
    );
}

pub fn emit_error(app: &AppHandle, source: &'static str, error: impl ToString) {
    let _ = app.emit(
        "bg:event",
        BgEvent {
            source,
            kind: BgEventKind::Error,
            at: now_ms(),
            summary: None,
            error: Some(error.to_string()),
        },
    );
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
