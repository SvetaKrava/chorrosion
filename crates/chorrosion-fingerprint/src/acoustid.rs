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
#[derive(Debug, Clone)]
pub struct AcoustidClient {
    client: Client,
    base_url: String,
    api_key: String,
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

        trace!(target: "fingerprint", "AcoustID lookup: {}", url);

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
        trace!(target: "fingerprint", "AcoustID response: {}", body);

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
    /// - `NoMatches` if the API returns no matches at all.
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

        // If no matches at all, return NoMatches error
        if all_matches.is_empty() {
            return Err(crate::FingerprintError::NoMatches);
        }

        // Find the best match
        let best_match = all_matches
            .into_iter()
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap())
            .expect("all_matches is not empty");

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
#[derive(Debug)]
pub struct AcoustidClientBuilder {
    api_key: String,
    base_url: String,
    timeout: Duration,
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

        // Should return NoMatches error when API returns no results
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), crate::FingerprintError::NoMatches));
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
}
