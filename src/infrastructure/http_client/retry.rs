//! Generic exponential-backoff retry policy for async operations.
//!
//! [`RetryPolicy`] wraps any async closure and re-executes it up to
//! `max_attempts` times, sleeping an exponentially increasing delay between
//! each failure.  It is agnostic to the error type so it can be reused for
//! any fallible future (HTTP requests, background jobs, etc.).
//!
//! # Retry schedule (example: `base_delay_ms = 500`, `max_attempts = 3`)
//! | Attempt | Delay before next try |
//! |---------|-----------------------|
//! | 1 (initial) | — (no delay)      |
//! | 2           | 500 ms             |
//! | 3           | 1 000 ms           |
//!
//! After `max_attempts` failures the last error is returned to the caller.

use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

/// Exponential-backoff retry policy.
///
/// Cheaply [`Clone`]-able so it can be shared with the HTTP client and
/// passed into retry closures without reference counting overhead.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Total number of attempts (initial try + retries).  Must be ≥ 1.
    pub max_attempts: u32,

    /// Base delay in milliseconds.  The delay before attempt `n` (0-indexed)
    /// is `base_delay_ms * 2^n`.
    pub base_delay_ms: u64,
}

impl RetryPolicy {
    /// Create a new policy with the given limits.
    ///
    /// # Parameters
    /// - `max_attempts`  — total number of attempts (≥ 1; `1` means no retries)
    /// - `base_delay_ms` — seed delay in milliseconds for the backoff formula
    pub fn new(max_attempts: u32, base_delay_ms: u64) -> Self {
        RetryPolicy { max_attempts, base_delay_ms }
    }

    /// Returns `true` if the given HTTP status code should trigger a retry.
    ///
    /// Retryable codes: `429 Too Many Requests`, `500 Internal Server Error`,
    /// `502 Bad Gateway`, `503 Service Unavailable`, `504 Gateway Timeout`.
    pub fn is_retryable_status(status: u16) -> bool {
        matches!(status, 429 | 500 | 502 | 503 | 504)
    }

    /// Calculate the backoff delay for a given attempt index (0-indexed).
    ///
    /// Formula: `base_delay_ms * 2^attempt`
    ///
    /// # Example
    /// ```text
    /// attempt 0 → base_delay_ms * 1
    /// attempt 1 → base_delay_ms * 2
    /// attempt 2 → base_delay_ms * 4
    /// ```
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let multiplier = 2u64.pow(attempt);
        Duration::from_millis(self.base_delay_ms * multiplier)
    }

    /// Execute `f` with automatic retries on failure.
    ///
    /// `f` is called at most `max_attempts` times.  On each failure a
    /// structured warning is emitted that includes the attempt number, the
    /// total allowed attempts, the upcoming delay, and the error value.
    /// After the final attempt the last error is returned unchanged.
    ///
    /// # Type parameters
    /// - `F`  — closure that produces a new future on each call
    /// - `Fut`— the future returned by `F`
    /// - `T`  — success value
    /// - `E`  — error type (must implement `Debug` for logging)
    pub async fn execute<F, Fut, T, E>(&self, mut f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        let mut last_err = None;

        for attempt in 0..self.max_attempts {
            match f().await {
                Ok(val) => return Ok(val),
                Err(e) => {
                    let remaining = self.max_attempts - attempt - 1;
                    if remaining > 0 {
                        let delay_ms = self.delay_for_attempt(attempt).as_millis();
                        warn!(
                            attempt = attempt + 1,
                            max_attempts = self.max_attempts,
                            delay_ms,
                            error = ?e,
                            "Request failed — retrying after backoff"
                        );
                        sleep(self.delay_for_attempt(attempt)).await;
                    } else {
                        warn!(
                            attempt = attempt + 1,
                            max_attempts = self.max_attempts,
                            error = ?e,
                            "Request failed — no retries remaining"
                        );
                    }
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_status_codes() {
        assert!(RetryPolicy::is_retryable_status(429));
        assert!(RetryPolicy::is_retryable_status(500));
        assert!(RetryPolicy::is_retryable_status(502));
        assert!(RetryPolicy::is_retryable_status(503));
        assert!(RetryPolicy::is_retryable_status(504));
    }

    #[test]
    fn test_non_retryable_status_codes() {
        assert!(!RetryPolicy::is_retryable_status(200));
        assert!(!RetryPolicy::is_retryable_status(400));
        assert!(!RetryPolicy::is_retryable_status(401));
        assert!(!RetryPolicy::is_retryable_status(404));
    }

    #[test]
    fn test_exponential_backoff_delay() {
        let policy = RetryPolicy::new(3, 100);
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_first_try() {
        let policy = RetryPolicy::new(3, 1);
        let result: Result<i32, &str> = policy.execute(|| async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_fails_after_max_attempts() {
        let policy = RetryPolicy::new(2, 1);
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = counter.clone();
        let result: Result<i32, &str> = policy.execute(|| {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err("always fails")
            }
        }).await;
        assert!(result.is_err());
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        let policy = RetryPolicy::new(3, 1);
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = counter.clone();
        let result: Result<i32, &str> = policy.execute(|| {
            let c = counter_clone.clone();
            async move {
                let n = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n == 0 { Err("first attempt fails") } else { Ok(99) }
            }
        }).await;
        assert_eq!(result.unwrap(), 99);
    }
}
