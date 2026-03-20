use reqwest::{RequestBuilder, Response, StatusCode};
use tokio::time::{sleep, Duration};
use tracing::warn;

pub(crate) const MAX_ATTEMPTS: usize = 3;
const INITIAL_BACKOFF_MS: u64 = 200;
const MAX_BACKOFF_MS: u64 = 2_000;
/// Maximum number of seconds we will honour from a `Retry-After` header.
/// Requests asking us to wait longer are capped at this value to avoid
/// indefinite hangs during a scheduler run.
const MAX_RETRY_AFTER_SECS: u64 = 60;

pub async fn send_with_retry<F>(
    mut build_request: F,
    client_name: &'static str,
) -> Result<Response, reqwest::Error>
where
    F: FnMut() -> RequestBuilder,
{
    let mut attempt = 1usize;

    loop {
        match build_request().send().await {
            Ok(response) => {
                let status = response.status();
                if should_retry_status(status) && attempt < MAX_ATTEMPTS {
                    let wait = if status == StatusCode::TOO_MANY_REQUESTS {
                        retry_after_delay(&response).unwrap_or_else(|| backoff_for_attempt(attempt))
                    } else {
                        backoff_for_attempt(attempt)
                    };
                    warn!(
                        target: "metadata",
                        client = client_name,
                        attempt,
                        max_attempts = MAX_ATTEMPTS,
                        status = %status,
                        wait_ms = wait.as_millis(),
                        "transient HTTP status received, retrying request"
                    );
                    let _ = response.bytes().await;
                    sleep(wait).await;
                    attempt += 1;
                    continue;
                }

                return Ok(response);
            }
            Err(error) => {
                if should_retry_error(&error) && attempt < MAX_ATTEMPTS {
                    warn!(
                        target: "metadata",
                        client = client_name,
                        attempt,
                        max_attempts = MAX_ATTEMPTS,
                        error = %error,
                        "transient request error received, retrying request"
                    );
                    sleep(backoff_for_attempt(attempt)).await;
                    attempt += 1;
                    continue;
                }

                return Err(error);
            }
        }
    }
}

/// Parse a `Retry-After` header value as an integer number of seconds.
///
/// Returns `None` when the header is absent, non-UTF-8, non-numeric, or zero.
/// The returned duration is capped at [`MAX_RETRY_AFTER_SECS`].
pub fn retry_after_delay(response: &Response) -> Option<Duration> {
    let value = response.headers().get(reqwest::header::RETRY_AFTER)?;
    let text = value.to_str().ok()?.trim();
    let secs: u64 = text.parse().ok()?;
    if secs == 0 {
        return None;
    }
    Some(Duration::from_secs(secs.min(MAX_RETRY_AFTER_SECS)))
}

pub(crate) fn should_retry_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

pub(crate) fn should_retry_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

pub(crate) fn backoff_for_attempt(attempt: usize) -> Duration {
    let factor = 1u64 << attempt.saturating_sub(1);
    let millis = (INITIAL_BACKOFF_MS * factor).min(MAX_BACKOFF_MS);
    Duration::from_millis(millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_parses_integer_seconds() {
        // We test the pure parsing logic without a live Response using a
        // helper that constructs a minimal response.
        fn parse(header_val: &str) -> Option<Duration> {
            // Build a minimal reqwest::Response via a raw http::Response so we
            // can inspect the header parsing logic end-to-end.
            let raw = http::Response::builder()
                .header(reqwest::header::RETRY_AFTER, header_val)
                .body(bytes::Bytes::new())
                .unwrap();
            let resp = reqwest::Response::from(raw);
            retry_after_delay(&resp)
        }

        assert_eq!(parse("30"), Some(Duration::from_secs(30)));
        assert_eq!(parse("1"), Some(Duration::from_secs(1)));
        // Values above the cap should be capped.
        assert_eq!(
            parse("999"),
            Some(Duration::from_secs(MAX_RETRY_AFTER_SECS))
        );
        // Zero is treated as absent.
        assert_eq!(parse("0"), None);
        // Non-numeric values fall back to None.
        assert_eq!(parse("Wed, 21 Oct 2015 07:28:00 GMT"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn retry_after_absent_returns_none() {
        let raw = http::Response::builder().body(bytes::Bytes::new()).unwrap();
        let resp = reqwest::Response::from(raw);
        assert_eq!(retry_after_delay(&resp), None);
    }
}
