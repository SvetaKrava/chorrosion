// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration, Instant};

/// Rate limiter for MusicBrainz API calls.
///
/// MusicBrainz rate limit: 1 request per second for non-commercial use.
/// This implementation uses a semaphore and enforces a minimum delay between requests.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    min_interval: Duration,
    last_request: Arc<tokio::sync::Mutex<Option<Instant>>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the specified minimum interval between requests.
    ///
    /// # Arguments
    /// * `min_interval` - Minimum duration between requests (default: 1 second for MusicBrainz).
    pub fn new(min_interval: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
            min_interval,
            last_request: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Create a rate limiter with MusicBrainz defaults (1 request per second).
    pub fn musicbrainz_default() -> Self {
        Self::new(Duration::from_secs(1))
    }

    /// Wait until a request can be made according to the rate limit.
    pub async fn acquire(&self) {
        let _permit = self.semaphore.acquire().await.expect("semaphore closed");

        let mut last = self.last_request.lock().await;

        if let Some(last_instant) = *last {
            let elapsed = last_instant.elapsed();
            if elapsed < self.min_interval {
                let wait_time = self.min_interval - elapsed;
                tracing::trace!(
                    target: "musicbrainz",
                    "rate limiting: waiting {:?}",
                    wait_time
                );
                sleep(wait_time).await;
            }
        }

        *last = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;

    #[tokio::test]
    async fn test_rate_limiter_enforces_delay() {
        let limiter = RateLimiter::new(Duration::from_millis(100));

        let start = Instant::now();

        // First request should be immediate
        limiter.acquire().await;
        let first_elapsed = start.elapsed();
        assert!(first_elapsed < Duration::from_millis(50));

        // Second request should wait ~100ms
        limiter.acquire().await;
        let second_elapsed = start.elapsed();
        assert!(
            second_elapsed >= Duration::from_millis(100),
            "expected >= 100ms, got {:?}",
            second_elapsed
        );
        assert!(second_elapsed < Duration::from_millis(150));
    }

    #[tokio::test]
    async fn test_rate_limiter_multiple_requests() {
        let limiter = RateLimiter::new(Duration::from_millis(50));
        let start = Instant::now();

        for _ in 0..3 {
            limiter.acquire().await;
        }

        let elapsed = start.elapsed();
        // Should take at least 100ms (2 intervals between 3 requests)
        assert!(
            elapsed >= Duration::from_millis(100),
            "expected >= 100ms, got {:?}",
            elapsed
        );
    }
}
