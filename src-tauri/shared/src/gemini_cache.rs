//! Explicit prompt caching via Gemini's `cachedContents` API.
//!
//! Caching the (system_prompt + tools) ensemble lets us reuse the same
//! 1–5k preamble across every Ask/minutes/briefing turn at the discounted
//! cached-token price.
//!
//! Lifecycle:
//! 1. Caller invokes [`ensure_cache`] with the (model, system_prompt, tools)
//!    tuple. We compute a stable content hash and look it up in an in-memory
//!    map.
//! 2. On miss (or expiry), we POST to `cachedContents` and store the returned
//!    `name` (e.g. `cachedContents/abc123`) with an expiry timestamp.
//! 3. The caller passes `cachedContent: "<name>"` in the generateContent body
//!    instead of inline `systemInstruction` and `tools`.
//!
//! If the API rejects the cache (commonly because the content is under the
//! per-model minimum token count), we return `Ok(None)` and the caller falls
//! back to inline. We log the rejection once per content hash so we don't
//! spam the console.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Window during which a previously-rejected (hash, model) tuple is treated
/// as uncacheable before we let a fresh `cachedContents` POST retry. Without
/// this, one transient 400 (rate-limit burst, model glitch) would disable
/// caching for that prompt until the app restarted.
const REJECTION_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone)]
struct CachedEntry {
    name: String,
    expires_at: Instant,
}

#[derive(Debug, Default)]
struct CacheState {
    entries: HashMap<u64, CachedEntry>,
    /// Map of content-hash → when the rejection was recorded. Re-creation
    /// is attempted again after `REJECTION_TTL`.
    rejected_hashes: HashMap<u64, Instant>,
}

static CACHE_STATE: OnceLock<Mutex<CacheState>> = OnceLock::new();

fn state() -> &'static Mutex<CacheState> {
    CACHE_STATE.get_or_init(|| Mutex::new(CacheState::default()))
}

fn fnv1a(input: &str) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn content_hash(model: &str, system_prompt: &str, tools: &serde_json::Value) -> u64 {
    let serialized = format!(
        "{model}\u{1f}{system_prompt}\u{1f}{}",
        serde_json::to_string(tools).unwrap_or_default()
    );
    fnv1a(&serialized)
}

/// Ensure a Gemini `cachedContents` entry exists for the given ensemble.
///
/// Returns `Some(name)` if a cache is available, `None` if caching was
/// rejected (e.g. minimum token count not met) — the caller should fall
/// back to inline systemInstruction + tools when this returns `None`.
pub async fn ensure_cache(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    tools: &serde_json::Value,
    ttl_seconds: u64,
) -> Result<Option<String>, String> {
    if api_key.is_empty() {
        return Ok(None);
    }
    let key = content_hash(model, system_prompt, tools);
    let now = Instant::now();

    {
        let mut guard = state().lock().expect("cache state poisoned");
        if let Some(&rejected_at) = guard.rejected_hashes.get(&key) {
            if now.duration_since(rejected_at) < REJECTION_TTL {
                return Ok(None);
            }
            // TTL expired — drop the stale rejection and let the call below
            // attempt a fresh cache create.
            guard.rejected_hashes.remove(&key);
        }
        if let Some(entry) = guard.entries.get(&key) {
            if entry.expires_at > now {
                return Ok(Some(entry.name.clone()));
            }
        }
    }

    let mut body = serde_json::json!({
        "model": format!("models/{model}"),
        "systemInstruction": { "parts": [{ "text": system_prompt }] },
        "ttl": format!("{ttl_seconds}s"),
    });
    if !tools.is_null() {
        body["tools"] = tools.clone();
    }

    let response = match crate::runtime::http_client()
        .post("https://generativelanguage.googleapis.com/v1beta/cachedContents")
        .query(&[("key", api_key)])
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(error) => return Err(format!("cachedContents request failed: {error}")),
    };

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        // Below-minimum-tokens is the most common failure; treat all 4xx as
        // "fall back to inline" rather than propagating an error.
        if status.is_client_error() {
            let mut guard = state().lock().expect("cache state poisoned");
            let was_new = guard.rejected_hashes.insert(key, Instant::now()).is_none();
            if was_new {
                eprintln!(
                    "[gemini_cache] cache creation rejected for model {model} ({status}): {}",
                    text.chars().take(160).collect::<String>()
                );
            }
            return Ok(None);
        }
        return Err(format!("cachedContents failed with {status}: {text}"));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("cachedContents response was not valid JSON: {e}"))?;
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "cachedContents response missing 'name'".to_string())?
        .to_string();

    let safety_margin = Duration::from_secs(60);
    let lifetime = Duration::from_secs(ttl_seconds).saturating_sub(safety_margin);
    let expires_at = now + lifetime.max(Duration::from_secs(60));
    {
        let mut guard = state().lock().expect("cache state poisoned");
        guard.entries.insert(
            key,
            CachedEntry {
                name: name.clone(),
                expires_at,
            },
        );
    }
    Ok(Some(name))
}

#[allow(dead_code)]
pub fn invalidate_all() {
    if let Some(lock) = CACHE_STATE.get() {
        if let Ok(mut guard) = lock.lock() {
            guard.entries.clear();
            guard.rejected_hashes.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_stable_for_same_input() {
        let tools = serde_json::json!([]);
        let a = content_hash("m", "prompt", &tools);
        let b = content_hash("m", "prompt", &tools);
        assert_eq!(a, b);
    }

    #[test]
    fn content_hash_differs_by_model() {
        let tools = serde_json::json!([]);
        let a = content_hash("m1", "prompt", &tools);
        let b = content_hash("m2", "prompt", &tools);
        assert_ne!(a, b);
    }

    /// Rejections recorded `REJECTION_TTL` ago must be dropped on the next
    /// lookup so transient 400s don't permanently disable caching.
    #[test]
    fn rejected_hashes_expire_after_ttl() {
        let key = 0xDEADBEEFu64;
        let stale = Instant::now()
            .checked_sub(REJECTION_TTL + Duration::from_secs(60))
            .expect("clock arithmetic");
        {
            let mut guard = state().lock().expect("cache state poisoned");
            guard.rejected_hashes.insert(key, stale);
        }
        // Simulate the same predicate `ensure_cache` runs.
        let still_rejected = {
            let mut guard = state().lock().expect("cache state poisoned");
            if let Some(&rejected_at) = guard.rejected_hashes.get(&key) {
                if Instant::now().duration_since(rejected_at) < REJECTION_TTL {
                    true
                } else {
                    guard.rejected_hashes.remove(&key);
                    false
                }
            } else {
                false
            }
        };
        assert!(!still_rejected, "stale rejection should have been dropped");
        let guard = state().lock().expect("cache state poisoned");
        assert!(
            !guard.rejected_hashes.contains_key(&key),
            "stale entry must be removed on lookup"
        );
    }

    #[test]
    fn fresh_rejection_keeps_blocking() {
        let key = 0xC0FFEEu64;
        {
            let mut guard = state().lock().expect("cache state poisoned");
            guard.rejected_hashes.insert(key, Instant::now());
        }
        let blocked = {
            let guard = state().lock().expect("cache state poisoned");
            guard
                .rejected_hashes
                .get(&key)
                .map(|&t| Instant::now().duration_since(t) < REJECTION_TTL)
                .unwrap_or(false)
        };
        assert!(blocked, "fresh rejection must still block caching");
    }
}
