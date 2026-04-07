// SPDX-License-Identifier: GPL-3.0-or-later
use reqwest::Client;
use std::time::Duration;

const DEFAULT_HTTP_TIMEOUT_SECONDS: u64 = 30;

fn default_user_agent() -> String {
    format!(
        "chorrosion/{} (+https://github.com/SvetaKrava/chorrosion)",
        env!("CARGO_PKG_VERSION")
    )
}

fn build_http_client_with_timeout(timeout: Duration) -> Client {
    Client::builder()
        .user_agent(default_user_agent())
        .timeout(timeout)
        .build()
        .unwrap_or_else(|error| {
            tracing::debug!(
                target: "application",
                ?error,
                "Failed to build HTTP client with custom settings, falling back to default"
            );
            Client::new()
        })
}

/// Builds a `reqwest::Client` configured with the chorrosion user-agent and a 30-second timeout.
/// Falls back to a default `Client` if the builder fails.
pub(crate) fn build_http_client() -> Client {
    build_http_client_with_timeout(Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECONDS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn build_http_client_sets_expected_user_agent() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener
            .local_addr()
            .expect("listener should expose local address");
        let (tx, rx) = oneshot::channel::<String>();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept should succeed");
            let mut buffer = [0_u8; 4096];
            let read = socket
                .read(&mut buffer)
                .await
                .expect("socket read should succeed");
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();

            let _ = tx.send(request);

            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await
                .expect("socket write should succeed");
        });

        let client = build_http_client();
        let response = client
            .get(format!("http://{addr}/ua"))
            .send()
            .await
            .expect("request should complete");
        assert!(response.status().is_success());

        let request = rx.await.expect("captured request should be sent");
        let request_lower = request.to_ascii_lowercase();
        let expected_header = format!("user-agent: {}", default_user_agent().to_ascii_lowercase());

        assert!(
            request_lower.contains(&expected_header),
            "expected request to include header '{expected_header}', got: {request}"
        );
    }

    #[tokio::test]
    async fn build_http_client_applies_timeout() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener
            .local_addr()
            .expect("listener should expose local address");

        // Accept one connection and keep it open without writing a response so
        // the client request can only complete via timeout.
        tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.expect("accept should succeed");
            tokio::time::sleep(Duration::from_secs(5)).await;
        });

        let client = build_http_client_with_timeout(Duration::from_millis(50));
        let error = client
            .get(format!("http://{addr}/timeout"))
            .send()
            .await
            .expect_err("request should time out");

        assert!(error.is_timeout(), "expected timeout error, got: {error}");
    }
}
