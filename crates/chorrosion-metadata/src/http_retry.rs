use reqwest::{RequestBuilder, Response, StatusCode};
use tokio::time::{sleep, Duration};
use tracing::warn;

pub(crate) const MAX_ATTEMPTS: usize = 3;
const INITIAL_BACKOFF_MS: u64 = 200;
const MAX_BACKOFF_MS: u64 = 2_000;

pub async fn send_with_retry<F>(
    mut build_request: F,
    target: &'static str,
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
                    warn!(
                        target: "metadata",
                        client = target,
                        attempt,
                        max_attempts = MAX_ATTEMPTS,
                        status = %status,
                        "transient HTTP status received, retrying request"
                    );
                    let _ = response.bytes().await;
                    sleep(backoff_for_attempt(attempt)).await;
                    attempt += 1;
                    continue;
                }

                return Ok(response);
            }
            Err(error) => {
                if should_retry_error(&error) && attempt < MAX_ATTEMPTS {
                    warn!(
                        target: "metadata",
                        client = target,
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
