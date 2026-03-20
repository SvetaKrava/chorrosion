//! Integration tests for `http_retry::send_with_retry` with a live mock server.
//!
//! These tests verify that `Retry-After` headers on 429 responses are honoured and
//! that retry behaviour degrades gracefully when the header is absent.

use chorrosion_metadata::http_retry;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Helper: start a mock server and return it so that expectations are checked on drop.
async fn mock_server() -> MockServer {
    MockServer::start().await
}

/// Verify that `send_with_retry` performs up to three attempts (initial request
/// plus two retries) when the server returns 429 with a `Retry-After: 1` header
/// for the first two requests and finally returns 200 on the third.
///
/// The test uses two Mocks: the first serves 429 twice (with `Retry-After: 1`)
/// and the second serves a final 200; wiremock evaluates them in registration
/// order.
#[tokio::test(start_paused = true)]
async fn send_with_retry_honours_retry_after_header() {
    let server = mock_server().await;

    // First two requests → 429 with Retry-After: 1
    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .expect(2)
        .up_to_n_times(2)
        .mount(&server)
        .await;

    // Third request → 200
    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let url = format!("{}/test", server.uri());

    let result = http_retry::send_with_retry(|| client.get(&url), "test-client").await;

    assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
    assert_eq!(result.unwrap().status(), 200);
}

/// When the server returns 429 with a zero `Retry-After` (treated as absent),
/// `send_with_retry` should still retry using exponential backoff and succeed.
#[tokio::test(start_paused = true)]
async fn send_with_retry_falls_back_to_backoff_when_retry_after_is_zero() {
    let server = mock_server().await;

    Mock::given(method("GET"))
        .and(path("/zero"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .expect(2)
        .up_to_n_times(2)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/zero"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let url = format!("{}/zero", server.uri());

    let result = http_retry::send_with_retry(|| client.get(&url), "test-client").await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().status(), 200);
}

/// When the maximum retry attempts are exhausted (three 429 responses) the
/// function returns the last 429 response without panicking.
#[tokio::test(start_paused = true)]
async fn send_with_retry_returns_last_response_after_max_attempts() {
    let server = mock_server().await;

    Mock::given(method("GET"))
        .and(path("/limited"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .expect(3)
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let url = format!("{}/limited", server.uri());

    let result = http_retry::send_with_retry(|| client.get(&url), "test-client").await;

    assert!(result.is_ok(), "expect Ok(response), not Err");
    assert_eq!(
        result.unwrap().status(),
        429,
        "expect final 429 to be returned after exhausting retries"
    );
}
