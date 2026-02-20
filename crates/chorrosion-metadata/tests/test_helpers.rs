//! Test helper to wait for the mock server to be ready on a given port.

use std::time::Duration;

/// Waits for the mock server to be ready by polling the given URL.
/// Fails the test if the server does not respond in time.
pub async fn wait_for_mock_server_ready(url: &str, timeout_secs: u64) {
    let client = reqwest::Client::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    while std::time::Instant::now() < deadline {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => return,
            _ => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }
    panic!("Mock server at {url} did not become ready in {timeout_secs} seconds");
}
