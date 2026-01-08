// SPDX-License-Identifier: GPL-3.0-or-later

use crate::fingerprint::Fingerprint;
use crate::error::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, trace};
use url::Url;
use uuid::Uuid;

const ACOUSTID_API_BASE: &str = "https://api.acoustid.org/v2";
const USER_AGENT: &str = concat!(
    "Chorrosion/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/SvetaKrava/chorrosion )"
);

/// AcoustID lookup result for a recording match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordingMatch {
    /// MusicBrainz recording ID.
    pub id: Uuid,
    /// Title of the recording.
    pub title: Option<String>,
    /// Artist name(s).
    #[serde(default)]
    pub artists: Vec<RecordingArtist>,
    /// Release information associated with this recording.
    #[serde(default)]
    pub releases: Vec<ReleaseInfo>,
    /// Match score (0-1), higher is more confident.
    pub score: f32,
}

/// Artist associated with a recording.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecordingArtist {
    /// MusicBrainz artist ID.
    pub id: Uuid,
    /// Artist name.
    pub name: String,
}

/// Release information for a recording.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseInfo {
    /// MusicBrainz release ID.
    pub id: Uuid,
    /// Release title.
    pub title: String,
    /// Release date (YYYY, YYYY-MM, or YYYY-MM-DD).
    #[serde(default)]
    pub date: Option<String>,
}

/// AcoustID API client for fingerprint lookup.
#[derive(Clone)]
pub struct AcoustidClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl std::fmt::Debug for AcoustidClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcoustidClient")
            .field("client", &self.client)
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl AcoustidClient {
    /// Create a new AcoustID client.
    ///
    /// # Arguments
    /// * `api_key` - AcoustID API key for requests.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::builder(api_key).build()
    }

    /// Create a client builder for custom configuration.
    pub fn builder(api_key: impl Into<String>) -> AcoustidClientBuilder {
        AcoustidClientBuilder::new(api_key)
    }

    /// Lookup a fingerprint on AcoustID and return all matching recordings (internal, unfiltered).
    async fn lookup_raw(&self, fingerprint: &Fingerprint) -> Result<Vec<RecordingMatch>> {
        fingerprint.validate()?;

        let mut url = Url::parse(&format!("{}/lookup", self.base_url))
            .map_err(|e| crate::FingerprintError::InvalidResponse(e.to_string()))?;

        url.query_pairs_mut()
            .append_pair("client", &self.api_key)
            .append_pair("fingerprint", &fingerprint.hash)
            .append_pair("duration", &fingerprint.duration.to_string())
            .append_pair("meta", "recordings releases artistids");

        let mut redacted_url = url.clone();
        redacted_url.set_query(None);
        trace!(target: "fingerprint", "AcoustID lookup: {}", redacted_url);

        let response = self
            .client
            .get(url.as_str())
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        let status = response.status();
        debug!(target: "fingerprint", "AcoustID response status: {}", status);

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(crate::FingerprintError::AcoustidError(
                format!("HTTP {}: {}", status, message)
            ));
        }

        let body = response.text().await?;
        let max_log_len: usize = 2048;
        let body_for_log = if body.len() > max_log_len {
            format!(
                "{}...[truncated {} of {} bytes]",
                &body[..max_log_len],
                body.len() - max_log_len,
                body.len()
            )
        } else {
            body.clone()
        };
        trace!(
            target: "fingerprint",
            "AcoustID response (up to {} bytes shown): {}",
            max_log_len,
            body_for_log
        );

        let api_response: AcoustidResponse = serde_json::from_str(&body)?;

        if !api_response.status.eq_ignore_ascii_case("ok") {
            return Err(crate::FingerprintError::AcoustidError(
                api_response.error.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }

        Ok(api_response.results)
    }

    /// Lookup a fingerprint on AcoustID and return matching recordings.
    ///
    /// # Arguments
    /// * `fingerprint` - The Chromaprint fingerprint.
    /// * `min_score` - Minimum confidence score (0-1) for returned matches.
    ///
    /// # Example
    /// ```no_run
    /// # use chorrosion_fingerprint::{AcoustidClient, Fingerprint};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = AcoustidClient::new("your-api-key")?;
    /// let fp = Fingerprint::new("AQADvEWZ==", 120);
    /// let matches = client.lookup(&fp, 0.7).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn lookup(&self, fingerprint: &Fingerprint, min_score: f32) -> Result<Vec<RecordingMatch>> {
        if !(0.0..=1.0).contains(&min_score) {
            return Err(crate::FingerprintError::AcoustidError(
                "Invalid parameter: min_score must be between 0.0 and 1.0".to_string(),
            ));
        }

        let all_matches = self.lookup_raw(fingerprint).await?;

        let matches = all_matches
            .into_iter()
            .filter(|m| m.score >= min_score)
            .collect();

        Ok(matches)
    }

    /// Lookup and return the best match (highest score).
    ///
    /// # Arguments
    /// * `fingerprint` - The Chromaprint fingerprint.
    /// * `min_score` - Minimum confidence score (0-1) for the match.
    ///
    /// # Errors
    /// Returns:
    /// - `AcoustidError` if the API returns no matches at all.
    /// - `LowConfidence` if matches exist but the best score is below `min_score`.
    pub async fn lookup_best(
        &self,
        fingerprint: &Fingerprint,
        min_score: f32,
    ) -> Result<RecordingMatch> {
        if !(0.0..=1.0).contains(&min_score) {
            return Err(crate::FingerprintError::AcoustidError(
                "Invalid parameter: min_score must be between 0.0 and 1.0".to_string(),
            ));
        }

        // Get all matches (unfiltered) to distinguish between no matches and low confidence
        let all_matches = self.lookup_raw(fingerprint).await?;

        // If no matches at all, return error
        if all_matches.is_empty() {
            return Err(crate::FingerprintError::AcoustidError(
                "No matches found for fingerprint".to_string()
            ));
        }

        // Find the best match
        let best_match = all_matches
            .into_iter()
            .max_by(|a, b| {
                // Use total_cmp for f32 to handle NaN cases properly
                a.score.total_cmp(&b.score)
            })
            .expect("Internal error: all_matches should not be empty after checking is_empty()");

        // Check if the best match meets the minimum score threshold
        if best_match.score >= min_score {
            Ok(best_match)
        } else {
            Err(crate::FingerprintError::LowConfidence {
                score: best_match.score,
            })
        }
    }
}

/// AcoustID API response structure.
#[derive(Debug, Deserialize)]
struct AcoustidResponse {
    status: String,
    #[serde(default)]
    results: Vec<RecordingMatch>,
    error: Option<String>,
}

/// Builder for AcoustID client.
pub struct AcoustidClientBuilder {
    api_key: String,
    base_url: String,
    timeout: Duration,
}

impl std::fmt::Debug for AcoustidClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcoustidClientBuilder")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl AcoustidClientBuilder {
    /// Create a new builder.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: ACOUSTID_API_BASE.to_string(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Set a custom base URL (useful for testing).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the AcoustID client.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The base URL is not a valid URL format
    /// - The HTTP client cannot be created
    pub fn build(self) -> Result<AcoustidClient> {
        // Validate base URL format early
        Url::parse(&self.base_url)
            .map_err(|e| crate::FingerprintError::AcoustidError(
                format!("Invalid base URL: {}", e)
            ))?;

        let client = Client::builder()
            .timeout(self.timeout)
            .user_agent(USER_AGENT)
            .build()?;

        Ok(AcoustidClient {
            client,
            base_url: self.base_url,
            api_key: self.api_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_response() -> serde_json::Value {
        serde_json::json!({
            "status": "ok",
            "results": [{
                "id": "0dd2d1a0-88f2-41a4-b6da-0f3ba8caf50a",
                "title": "Fake Plastic Trees",
                "score": 0.95,
                "artists": [{
                    "id": "a74b1b7f-71a5-4011-9441-d0b5e4122711",
                    "name": "Radiohead"
                }],
                "releases": [{
                    "id": "9c40fc9f-3e6f-4e81-b5e5-76d05ce7b5f0",
                    "title": "The Bends",
                    "date": "1995-03-16"
                }]
            }]
        })
    }

    #[tokio::test]
    async fn test_acoustid_lookup() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lookup"))
            .and(query_param("fingerprint", "AQADvEWZ=="))
            .and(query_param("duration", "120"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response()))
            .mount(&mock_server)
            .await;

        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        let matches = client.lookup(&fp, 0.5).await.unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].title, Some("Fake Plastic Trees".to_string()));
        assert!(matches[0].score > 0.9);
    }

    #[tokio::test]
    async fn test_acoustid_lookup_best() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response()))
            .mount(&mock_server)
            .await;

        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        let best = client.lookup_best(&fp, 0.8).await.unwrap();

        assert_eq!(best.title, Some("Fake Plastic Trees".to_string()));
    }

    #[tokio::test]
    async fn test_acoustid_low_confidence_filter() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response()))
            .mount(&mock_server)
            .await;

        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        // Set min_score higher than the response score (0.95)
        let matches = client.lookup(&fp, 0.99).await.unwrap();

        // Should have no matches due to score filtering
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn test_acoustid_invalid_min_score() {
        let client = AcoustidClient::new("test-key").unwrap();
        let fp = Fingerprint::new("AQADvEWZ==", 120);

        let result = client.lookup(&fp, 1.5).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_acoustid_invalid_base_url() {
        // Invalid URL should be caught during build()
        let result = AcoustidClient::builder("test-key")
            .base_url("not-a-valid-url")
            .build();
        
        assert!(result.is_err());
        
        // Also test with a malformed URL
        let result = AcoustidClient::builder("test-key")
            .base_url("ht!tp://invalid")
            .build();
        
        assert!(result.is_err());
    }

    #[test]
    fn test_acoustid_valid_base_url() {
        // Valid URLs should work
        let result = AcoustidClient::builder("test-key")
            .base_url("https://api.example.com/v2")
            .build();
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_acoustid_lookup_best_no_matches() {
        let mock_server = MockServer::start().await;

        // Return empty results
        let empty_response = serde_json::json!({
            "status": "ok",
            "results": []
        });

        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_response))
            .mount(&mock_server)
            .await;

        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        let result = client.lookup_best(&fp, 0.5).await;

        // Should return AcoustidError when API returns no results
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::FingerprintError::AcoustidError(msg) => {
                assert!(msg.contains("No matches"), "Error should indicate no matches found: {}", msg);
            }
            other => panic!("Expected AcoustidError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_acoustid_lookup_best_low_confidence() {
        let mock_server = MockServer::start().await;

        // Return a result with score 0.6
        let low_score_response = serde_json::json!({
            "status": "ok",
            "results": [{
                "id": "0dd2d1a0-88f2-41a4-b6da-0f3ba8caf50a",
                "title": "Test Song",
                "score": 0.6,
                "artists": [],
                "releases": []
            }]
        });

        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(low_score_response))
            .mount(&mock_server)
            .await;

        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        // Request min_score of 0.8, but result has 0.6
        let result = client.lookup_best(&fp, 0.8).await;

        // Should return LowConfidence with the actual score (0.6)
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::FingerprintError::LowConfidence { score } => {
                assert!((score - 0.6).abs() < 0.001, "Expected score 0.6, got {}", score);
            }
            other => panic!("Expected LowConfidence error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_acoustid_lookup_best_multiple_matches() {
        let mock_server = MockServer::start().await;

        // Return multiple results with different scores
        let multiple_response = serde_json::json!({
            "status": "ok",
            "results": [
                {
                    "id": "0dd2d1a0-88f2-41a4-b6da-0f3ba8caf50a",
                    "title": "Song A",
                    "score": 0.7,
                    "artists": [],
                    "releases": []
                },
                {
                    "id": "1ee3e2b1-99f3-52b5-c7db-1f4cb9dcf61b",
                    "title": "Song B",
                    "score": 0.85,
                    "artists": [],
                    "releases": []
                },
                {
                    "id": "2ff4f3c2-aaf4-63c6-d8ec-2f5dc0edea2c",
                    "title": "Song C",
                    "score": 0.6,
                    "artists": [],
                    "releases": []
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(multiple_response))
            .mount(&mock_server)
            .await;

        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        
        // Test 1: min_score below best match - should succeed with best match
        let result = client.lookup_best(&fp, 0.5).await;
        assert!(result.is_ok());
        let best = result.unwrap();
        assert_eq!(best.title, Some("Song B".to_string()));
        assert!((best.score - 0.85).abs() < 0.001);
        
        // Test 2: min_score above best match - should return LowConfidence with best score
        let result = client.lookup_best(&fp, 0.9).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::FingerprintError::LowConfidence { score } => {
                assert!((score - 0.85).abs() < 0.001, "Expected score 0.85, got {}", score);
            }
            other => panic!("Expected LowConfidence error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_acoustid_lookup_best_invalid_min_score() {
        let client = AcoustidClient::new("test-key").unwrap();
        let fp = Fingerprint::new("AQADvEWZ==", 120);

        // Test with min_score > 1.0
        let result = client.lookup_best(&fp, 1.5).await;
        assert!(result.is_err());
        
        // Test with min_score < 0.0
        let result = client.lookup_best(&fp, -0.1).await;
        assert!(result.is_err());
    }

    /// Helper function to test HTTP error responses from AcoustID API.
    ///
    /// Creates a mock server that returns the specified HTTP error status code and body,
    /// then verifies that the AcoustID client properly handles the error by:
    /// - Returning an AcoustidError
    /// - Including the status code in the error message
    /// - Including the response body in the error message
    ///
    /// # Arguments
    /// * `status_code` - The HTTP status code to return (e.g., 404, 500)
    /// * `body` - The response body text to return with the error
    async fn test_http_error_response(status_code: u16, body: &str) {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(ResponseTemplate::new(status_code).set_body_string(body))
            .mount(&mock_server)
            .await;

        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        let result = client.lookup(&fp, 0.5).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            crate::FingerprintError::AcoustidError(msg) => {
                assert!(
                    msg.contains(&status_code.to_string()),
                    "Error message should contain status code {}: {}",
                    status_code,
                    msg
                );
                assert!(
                    msg.contains(body),
                    "Error message should contain response body '{}': {}",
                    body,
                    msg
                );
            }
            other => panic!("Expected AcoustidError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_acoustid_http_404_error() {
        test_http_error_response(404, "Not Found").await;
    }

    #[tokio::test]
    async fn test_acoustid_http_500_error() {
        test_http_error_response(500, "Internal Server Error").await;
    }

    #[tokio::test]
    async fn test_acoustid_http_403_error() {
        test_http_error_response(403, "Forbidden").await;
    }

    #[tokio::test]
    async fn test_acoustid_http_error_with_json_body() {
        let mock_server = MockServer::start().await;

        let error_response = serde_json::json!({
            "status": "error",
            "error": "Invalid API key"
        });

        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(ResponseTemplate::new(401).set_body_json(error_response))
            .mount(&mock_server)
            .await;

        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        let result = client.lookup(&fp, 0.5).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            crate::FingerprintError::AcoustidError(msg) => {
                assert!(msg.contains("401"), "Error message should contain status code 401: {}", msg);
                // The body is consumed as text, so we should see the JSON string representation
                assert!(msg.contains("Invalid API key"), 
                    "Error message should contain API key error from JSON body: {}", msg);
            }
            other => panic!("Expected AcoustidError, got {:?}", other),
        }
    }

    #[test]
    fn test_acoustid_client_debug_redacts_api_key() {
        let client = AcoustidClient::new("super-secret-api-key").unwrap();
        let debug_output = format!("{:?}", client);
        
        // API key should be redacted
        assert!(!debug_output.contains("super-secret-api-key"),
            "Debug output should not contain the actual API key");
        assert!(debug_output.contains("[REDACTED]"),
            "Debug output should show [REDACTED] instead of the API key");
    }

    #[test]
    fn test_acoustid_client_builder_debug_redacts_api_key() {
        let builder = AcoustidClient::builder("super-secret-api-key");
        let debug_output = format!("{:?}", builder);
        
        // API key should be redacted
        assert!(!debug_output.contains("super-secret-api-key"),
            "Debug output should not contain the actual API key");
        assert!(debug_output.contains("[REDACTED]"),
            "Debug output should show [REDACTED] instead of the API key");
    }

    #[test]
    fn test_builder_applies_custom_base_url() {
        let custom_url = "https://custom.api.example.com/v2";
        let client = AcoustidClient::builder("test-key")
            .base_url(custom_url)
            .build()
            .unwrap();
        
        // Verify the custom base URL was applied
        assert_eq!(client.base_url, custom_url);
    }

    #[test]
    fn test_builder_applies_custom_timeout() {
        let custom_timeout = Duration::from_secs(60);
        let _client = AcoustidClient::builder("test-key")
            .timeout(custom_timeout)
            .build()
            .unwrap();
        
        // The client's timeout is embedded in the reqwest::Client.
        // We can't directly inspect it, but we can verify the build succeeds
        // and use the debug output to check it was set (though reqwest::Client
        // doesn't expose timeout in Debug either).
        // 
        // Instead, we'll test this more thoroughly via an integration test
        // that verifies timeout behavior. For now, just verify build succeeds.
        assert!(true, "Client built successfully with custom timeout");
    }

    #[tokio::test]
    async fn test_builder_custom_base_url_used_in_requests() {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(sample_response()))
            .mount(&mock_server)
            .await;

        // Build client with custom base URL pointing to mock server
        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        let result = client.lookup(&fp, 0.5).await;

        // If the custom base URL wasn't used, this would fail to connect
        assert!(result.is_ok(), "Request should succeed using custom base URL");
    }

    #[tokio::test]
    async fn test_builder_custom_timeout_behavior() {
        let mock_server = MockServer::start().await;

        // Create a mock that delays response longer than our custom timeout
        Mock::given(method("GET"))
            .and(path("/lookup"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(sample_response())
                    .set_delay(Duration::from_millis(200))
            )
            .mount(&mock_server)
            .await;

        // Build client with very short timeout
        let client = AcoustidClient::builder("test-key")
            .base_url(mock_server.uri())
            .timeout(Duration::from_millis(50))
            .build()
            .unwrap();

        let fp = Fingerprint::new("AQADvEWZ==", 120);
        let result = client.lookup(&fp, 0.5).await;

        // Request should timeout
        assert!(result.is_err(), "Request should timeout with short timeout");
    }

    #[test]
    fn test_builder_chaining() {
        let custom_url = "https://custom.api.example.com/v2";
        let custom_timeout = Duration::from_secs(45);
        
        // Test that builder methods can be chained
        let client = AcoustidClient::builder("test-key")
            .base_url(custom_url)
            .timeout(custom_timeout)
            .build()
            .unwrap();
        
        assert_eq!(client.base_url, custom_url);
    }

    #[test]
    fn test_builder_default_values() {
        // Build with only required parameter to verify defaults
        let client = AcoustidClient::builder("test-key")
            .build()
            .unwrap();
        
        // Verify default base URL
        assert_eq!(client.base_url, ACOUSTID_API_BASE);
        // Default timeout is set but we can't easily inspect it in the client
    }
}
