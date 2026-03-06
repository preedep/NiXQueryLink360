use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

/// Retry policy with exponential backoff
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, base_delay_ms: u64) -> Self {
        RetryPolicy { max_attempts, base_delay_ms }
    }

    /// Check if an HTTP status code is retryable
    pub fn is_retryable_status(status: u16) -> bool {
        matches!(status, 429 | 500 | 502 | 503 | 504)
    }

    /// Calculate delay for a given attempt (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let multiplier = 2u64.pow(attempt);
        Duration::from_millis(self.base_delay_ms * multiplier)
    }

    /// Execute a future with retry logic
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
                    warn!(attempt = attempt, error = ?e, "Request failed, will retry");
                    last_err = Some(e);
                    if attempt < self.max_attempts - 1 {
                        sleep(self.delay_for_attempt(attempt)).await;
                    }
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
