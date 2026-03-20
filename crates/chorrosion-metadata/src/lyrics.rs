use moka::sync::Cache;
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{debug, instrument};

use std::sync::Arc;

use crate::http_retry;

pub struct LyricsClient {
    client: Client,
    rate_limiter: Arc<Semaphore>,
    cache: Cache<String, LyricsMetadata>,
    base_url: String,
}

impl LyricsClient {
    /// Creates a `LyricsClient` using the default lyrics.ovh base URL and
    /// a default concurrency limit of 1 request at a time.
    pub fn new() -> Self {
        Self::new_with_limits_and_base_url(1, None)
    }

    /// Creates a `LyricsClient` with a custom base URL and a default
    /// concurrency limit of 1 request at a time.
    pub fn new_with_base_url(base_url: String) -> Self {
        Self::new_with_limits_and_base_url(1, Some(base_url))
    }

    /// Creates a `LyricsClient` with a custom concurrency limit and the default base URL.
    pub fn new_with_limits(max_concurrent_requests: usize) -> Self {
        Self::new_with_limits_and_base_url(max_concurrent_requests, None)
    }

    /// Creates a `LyricsClient` with a custom concurrency limit and optional base URL.
    pub fn new_with_limits_and_base_url(
        max_concurrent_requests: usize,
        base_url: Option<String>,
    ) -> Self {
        Self::new_with_limits_cache_timeout_and_base_url(
            max_concurrent_requests,
            5_000,
            15,
            base_url,
        )
    }

    /// Creates a `LyricsClient` with a custom concurrency limit, explicit cache capacity, and
    /// optional base URL.
    pub fn new_with_limits_cache_and_base_url(
        max_concurrent_requests: usize,
        cache_capacity: u64,
        base_url: Option<String>,
    ) -> Self {
        Self::new_with_limits_cache_timeout_and_base_url(
            max_concurrent_requests,
            cache_capacity,
            15,
            base_url,
        )
    }

    /// Creates a `LyricsClient` with explicit cache capacity, request timeout, and optional
    /// base URL.
    pub fn new_with_limits_cache_timeout_and_base_url(
        max_concurrent_requests: usize,
        cache_capacity: u64,
        request_timeout_seconds: u64,
        base_url: Option<String>,
    ) -> Self {
        let timeout = Duration::from_secs(request_timeout_seconds.max(1));
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|error| {
                debug!(
                    ?error,
                    "Failed to build Lyrics HTTP client with timeout, falling back to default client"
                );
                Client::new()
            });
        Self {
            client,
            rate_limiter: Arc::new(Semaphore::new(max_concurrent_requests.max(1))),
            cache: Cache::new(cache_capacity.max(1)),
            base_url: base_url
                .unwrap_or_else(|| "https://api.lyrics.ovh".to_string())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    #[instrument(skip(self), fields(artist = artist_name, title = track_title))]
    pub async fn fetch_lyrics(
        &self,
        artist_name: &str,
        track_title: &str,
    ) -> Result<LyricsMetadata, LyricsError> {
        let cache_key = format!("{}:{}", artist_name, track_title);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|_| LyricsError::RateLimiterClosed)?;

        let mut url = Url::parse(&self.base_url)
            .map_err(|_| LyricsError::InvalidBaseUrl(self.base_url.clone()))?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| LyricsError::InvalidBaseUrl(self.base_url.clone()))?;
            segments.extend(["v1", artist_name, track_title]);
        }

        debug!(target: "lyrics", url = %url, "Fetching lyrics metadata");

        let response =
            http_retry::send_with_retry(|| self.client.get(url.clone()), "lyrics").await?;
        let status = response.status();
        let response_body = response.text().await?;
        let payload = parse_lyrics_body(status, &response_body)?;

        let lyrics = payload
            .lyrics
            .filter(|value| !value.trim().is_empty())
            .ok_or(LyricsError::MissingField("lyrics"))?;

        let metadata = LyricsMetadata {
            artist: artist_name.to_string(),
            title: track_title.to_string(),
            lyrics,
        };
        self.cache.insert(cache_key, metadata.clone());
        Ok(metadata)
    }
}

impl Default for LyricsClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsMetadata {
    pub artist: String,
    pub title: String,
    pub lyrics: String,
}

#[derive(Debug, Error)]
pub enum LyricsError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP status {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },
    #[error("Lyrics API error: {message}")]
    Api { message: String },
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
    #[error("Invalid base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("Missing expected field: {0}")]
    MissingField(&'static str),
    #[error("Rate limiter closed")]
    RateLimiterClosed,
}

#[derive(Debug, Deserialize)]
struct LyricsApiResponse {
    lyrics: Option<String>,
    error: Option<String>,
    message: Option<String>,
}

fn parse_lyrics_body(
    status: StatusCode,
    response_body: &str,
) -> Result<LyricsApiResponse, LyricsError> {
    if !status.is_success() {
        return Err(LyricsError::HttpStatus {
            status,
            body: response_body.to_string(),
        });
    }

    let payload: LyricsApiResponse = serde_json::from_str(response_body)?;

    if let Some(message) = payload.error.as_deref().or(payload.message.as_deref()) {
        return Err(LyricsError::Api {
            message: message.to_string(),
        });
    }

    Ok(payload)
}
