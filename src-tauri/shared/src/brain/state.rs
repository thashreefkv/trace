//! Process-local shared state for the brain subsystem.
//!
//! Three concerns live here:
//!
//! 1. **Rebuild lock** — `rebuild_lock()` returns a process-wide `tokio::Mutex`
//!    that serializes every `rebuild_brain` call. Two layers protect the same
//!    `brain.kuzu`: this in-process mutex queues callers within one binary,
//!    and a sidecar file lock (taken inside `write_projection`) serializes the
//!    Tauri app against the MCP server.
//!
//! 2. **Dirty bits** — `BRAIN_REBUILDING` and `BRAIN_DIRTY` form a two-flag
//!    coalescer used by `request_rebuild`. The flow is: callers set DIRTY +
//!    notify; the worker swaps DIRTY → false, runs the rebuild, then loops if
//!    DIRTY became true again. REBUILDING tracks whether a worker is mid-run
//!    so we don't spawn duplicates.
//!
//! 3. **Inference-threshold cache** — `InferenceThresholdCache` is an
//!    in-memory snapshot of `inference_thresholds` loaded once at the top of
//!    a `refresh_brain_inferences` pass so the inner per-row inserts don't
//!    each round-trip to SQLite.

use sqlx::SqlitePool;

pub(super) static BRAIN_REBUILDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub(super) static BRAIN_DIRTY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(super) fn rebuild_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// In-memory cache of confidence thresholds for the duration of a single
/// `refresh_brain_inferences` call. Loaded once up front so the inner
/// per-row inserts don't each hit the DB.
#[derive(Debug, Default)]
pub(super) struct InferenceThresholdCache {
    values: std::collections::HashMap<String, f64>,
}

impl InferenceThresholdCache {
    pub(super) async fn load(pool: &SqlitePool) -> Self {
        let rows: Vec<(String, f64)> =
            sqlx::query_as("SELECT template, threshold FROM inference_thresholds")
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        let mut values = std::collections::HashMap::new();
        for (template, threshold) in rows {
            values.insert(template, threshold);
        }
        Self { values }
    }

    pub(super) fn get(&self, template: &str, fallback: f64) -> f64 {
        self.values.get(template).copied().unwrap_or(fallback)
    }
}
