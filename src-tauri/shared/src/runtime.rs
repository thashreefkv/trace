//! Centralized async spawn for the shared crate.
//!
//! Use `crate::runtime::spawn` / `crate::runtime::spawn_blocking` everywhere in
//! the shared crate instead of `tokio::spawn` / `tokio::task::spawn_blocking`.
//!
//! The Tauri app runs on tokio, so this is functionally identical to
//! `tauri::async_runtime::spawn` (which itself wraps `tokio::spawn`). The point
//! is to centralize: a single grep-able chokepoint enforces the project rule
//! that bare `tokio::spawn` never appears outside this module. New code that
//! adds a spawn cannot regress the contract by accident.
//!
//! See `CLAUDE.md` (Tauri async rule) for context.

use std::future::Future;
use std::sync::{Mutex, OnceLock};
use tokio::task::JoinHandle;

/// App-wide cache of the Gemini API key. Read-mostly; updated by command
/// handlers when the user saves or clears their key.
///
/// Hot paths like `retrieve_brain_context` use this to decide whether to
/// compute a query embedding for hybrid retrieval. Returning `None` here
/// causes graceful fallback to BM25-only scoring.
static GEMINI_API_KEY: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub fn set_gemini_api_key(key: Option<String>) {
    let cell = GEMINI_API_KEY.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = cell.lock() {
        *guard = key;
    }
}

pub fn gemini_api_key() -> Option<String> {
    GEMINI_API_KEY
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

/// App-wide HTTP client shared across every Gemini call site.
///
/// Reusing a single client lets reqwest pool TLS connections to
/// `generativelanguage.googleapis.com` so back-to-back Ask turns don't pay
/// a full handshake every time. No retries — we want network errors visible
/// to the rate-limiter's circuit breaker instead of papered over.
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            // End-to-end request cap. Audio transcription is the longest
            // legitimate call (~60s); 120s leaves headroom. A hung Gemini
            // response now fails fast so the circuit breaker can recover
            // instead of waiting indefinitely.
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("build reqwest client")
    })
}

/// Spawn an async task onto the ambient tokio runtime.
///
/// In the Tauri app this is the tauri-managed runtime. In standalone binaries
/// (`bin/mcp-server`, tests) it's the binary's own `#[tokio::main]` runtime.
///
/// Like `tokio::spawn`, this panics if called outside a running runtime — i.e.
/// never call from a `tauri::Builder::setup` callback before `.run()` has
/// started the runtime. Use it inside `#[tauri::command]` handlers or in tasks
/// spawned by them.
#[inline]
pub fn spawn<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::task::spawn(fut)
}

/// Spawn a blocking task onto the runtime's blocking pool.
#[inline]
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_returns_value() {
        let handle = spawn(async { 7 + 35 });
        assert_eq!(handle.await.unwrap(), 42);
    }

    #[tokio::test]
    async fn spawn_blocking_returns_value() {
        let handle = spawn_blocking(|| 21 * 2);
        assert_eq!(handle.await.unwrap(), 42);
    }
}
