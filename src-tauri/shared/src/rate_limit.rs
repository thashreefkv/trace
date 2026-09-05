//! Token-bucket rate limiter + circuit breaker for AI tool calls.
//!
//! Two independent layers share this module:
//!
//! * **Token bucket** (`try_acquire`) — keeps a runaway Gemini loop or
//!   adversarially-prompted agent from hammering external APIs. Defaults are
//!   generous so normal workflows are never blocked.
//! * **Circuit breaker** (`check_circuit` + `note_success`/`note_failure`) —
//!   when a (feature, model) pair fails N times in a row, halt new requests
//!   for a cooldown window so a misconfigured key or upstream outage doesn't
//!   burn through cost / quota.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const BREAKER_TRIP_THRESHOLD: u32 = 5;
const BREAKER_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy)]
pub struct Quota {
    pub capacity: u32,
    pub refill_per_minute: u32,
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last_refill: Instant,
    capacity: f64,
    refill_per_sec: f64,
}

#[derive(Debug, Default, Clone, Copy)]
struct Breaker {
    consecutive_errors: u32,
    opened_at: Option<Instant>,
}

#[derive(Debug)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
    breakers: Mutex<HashMap<String, Breaker>>,
    quotas: HashMap<String, Quota>,
    default_quota: Quota,
}

impl RateLimiter {
    pub fn new(default: Quota) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            breakers: Mutex::new(HashMap::new()),
            quotas: HashMap::new(),
            default_quota: default,
        }
    }

    pub fn with_quota(mut self, key: impl Into<String>, quota: Quota) -> Self {
        self.quotas.insert(key.into(), quota);
        self
    }

    pub fn try_acquire(&self, key: &str, cost: f64) -> Result<(), RateLimitError> {
        let mut buckets = self.buckets.lock().expect("rate limiter mutex poisoned");
        let quota = self.quotas.get(key).copied().unwrap_or(self.default_quota);
        let bucket = buckets.entry(key.to_string()).or_insert_with(|| Bucket {
            tokens: quota.capacity as f64,
            last_refill: Instant::now(),
            capacity: quota.capacity as f64,
            refill_per_sec: quota.refill_per_minute as f64 / 60.0,
        });

        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * bucket.refill_per_sec).min(bucket.capacity);
        bucket.last_refill = now;

        if bucket.tokens + 1e-9 >= cost {
            bucket.tokens -= cost;
            Ok(())
        } else {
            let deficit = cost - bucket.tokens;
            let retry_after_secs = if bucket.refill_per_sec > 0.0 {
                (deficit / bucket.refill_per_sec).ceil() as u64
            } else {
                60
            };
            Err(RateLimitError {
                key: key.to_string(),
                retry_after_secs: retry_after_secs.max(1),
                circuit_open: false,
            })
        }
    }

    /// Returns `Err` if the breaker for `key` is currently open (within
    /// cooldown after N consecutive failures). Use this before issuing an
    /// expensive external call.
    pub fn check_circuit(&self, key: &str) -> Result<(), RateLimitError> {
        let mut breakers = self.breakers.lock().expect("breaker mutex poisoned");
        let entry = breakers.entry(key.to_string()).or_default();
        if let Some(opened_at) = entry.opened_at {
            let elapsed = opened_at.elapsed();
            if elapsed < BREAKER_COOLDOWN {
                let remaining = BREAKER_COOLDOWN - elapsed;
                return Err(RateLimitError {
                    key: key.to_string(),
                    retry_after_secs: remaining.as_secs().max(1),
                    circuit_open: true,
                });
            }
            // Cooldown expired — half-open the breaker by clearing it. The
            // next note_failure can re-trip immediately, the next note_success
            // confirms recovery.
            entry.opened_at = None;
            entry.consecutive_errors = 0;
        }
        Ok(())
    }

    /// Record that a call against `key` succeeded. Resets the consecutive
    /// error count and closes any tripped breaker.
    pub fn note_success(&self, key: &str) {
        let mut breakers = self.breakers.lock().expect("breaker mutex poisoned");
        let entry = breakers.entry(key.to_string()).or_default();
        entry.consecutive_errors = 0;
        entry.opened_at = None;
    }

    /// Record that a call against `key` failed. After
    /// `BREAKER_TRIP_THRESHOLD` consecutive failures, the breaker opens for
    /// `BREAKER_COOLDOWN`.
    pub fn note_failure(&self, key: &str) {
        let mut breakers = self.breakers.lock().expect("breaker mutex poisoned");
        let entry = breakers.entry(key.to_string()).or_default();
        entry.consecutive_errors = entry.consecutive_errors.saturating_add(1);
        if entry.consecutive_errors >= BREAKER_TRIP_THRESHOLD && entry.opened_at.is_none() {
            entry.opened_at = Some(Instant::now());
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitError {
    pub key: String,
    pub retry_after_secs: u64,
    pub circuit_open: bool,
}

impl std::fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.circuit_open {
            write!(
                f,
                "circuit open on '{}': retry in {}s",
                self.key, self.retry_after_secs
            )
        } else {
            write!(
                f,
                "rate limited on '{}': retry in {}s",
                self.key, self.retry_after_secs
            )
        }
    }
}

/// Build the default app-wide limiter. Quotas are intentionally generous so
/// healthy agentic loops are never throttled.
fn build_default_limiter() -> RateLimiter {
    RateLimiter::new(Quota {
        capacity: 120,
        refill_per_minute: 120,
    })
    .with_quota(
        "ask",
        Quota {
            capacity: 90,
            refill_per_minute: 90,
        },
    )
    .with_quota(
        "mcp",
        Quota {
            capacity: 60,
            refill_per_minute: 60,
        },
    )
    .with_quota(
        "destructive",
        Quota {
            capacity: 10,
            refill_per_minute: 10,
        },
    )
}

static APP_LIMITER: OnceLock<RateLimiter> = OnceLock::new();

pub fn app_limiter() -> &'static RateLimiter {
    APP_LIMITER.get_or_init(build_default_limiter)
}

/// Classify a tool name by its risk. Used as a secondary key alongside mode
/// so destructive operations get a much tighter budget than reads.
pub fn classify_tool(tool: &str) -> &'static str {
    let t = tool.to_ascii_lowercase();
    if t.starts_with("delete_")
        || t.starts_with("unlink_")
        || t.starts_with("clear_")
        || t.starts_with("remove_")
    {
        "destructive"
    } else if t.starts_with("create_")
        || t.starts_with("update_")
        || t.starts_with("save_")
        || t.starts_with("add_")
        || t.starts_with("set_")
        || t.starts_with("promote_")
        || t.starts_with("apply_")
        || t.starts_with("link_")
        || t.starts_with("mark_")
        || t.starts_with("record_")
    {
        "write"
    } else {
        "read"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_grants_then_throttles() {
        let limiter = RateLimiter::new(Quota {
            capacity: 2,
            refill_per_minute: 60,
        });
        assert!(limiter.try_acquire("k", 1.0).is_ok());
        assert!(limiter.try_acquire("k", 1.0).is_ok());
        let err = limiter.try_acquire("k", 1.0).unwrap_err();
        assert!(err.retry_after_secs >= 1);
        assert!(!err.circuit_open);
    }

    #[test]
    fn classify_picks_category() {
        assert_eq!(classify_tool("delete_initiative"), "destructive");
        assert_eq!(classify_tool("create_deliverable"), "write");
        assert_eq!(classify_tool("search_email_threads"), "read");
    }

    #[test]
    fn breaker_trips_after_five_failures() {
        let limiter = RateLimiter::new(Quota {
            capacity: 100,
            refill_per_minute: 100,
        });
        assert!(limiter.check_circuit("ask:flash").is_ok());
        for _ in 0..BREAKER_TRIP_THRESHOLD {
            limiter.note_failure("ask:flash");
        }
        let err = limiter
            .check_circuit("ask:flash")
            .expect_err("breaker should be open");
        assert!(err.circuit_open);
        assert!(err.retry_after_secs > 0);
    }

    #[test]
    fn breaker_clears_on_success() {
        let limiter = RateLimiter::new(Quota {
            capacity: 100,
            refill_per_minute: 100,
        });
        for _ in 0..(BREAKER_TRIP_THRESHOLD - 1) {
            limiter.note_failure("ask:flash");
        }
        limiter.note_success("ask:flash");
        for _ in 0..(BREAKER_TRIP_THRESHOLD - 1) {
            limiter.note_failure("ask:flash");
        }
        assert!(
            limiter.check_circuit("ask:flash").is_ok(),
            "consecutive count should reset on success"
        );
    }
}
