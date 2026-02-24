use moka::sync::Cache;
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{debug, instrument};

use std::sync::Arc;

pub struct LyricsClient {
    client: Client,
    rate_limiter: Arc<Semaphore>,
    cache: Cache<String, LyricsMetadata>,
    base_url: String,
}

impl LyricsClient {
    pub fn new(base_url: Option<String>) -> Self {
        Self::new_with_limits_and_base_url(1, base_url)
    }

    pub fn new_with_limits(max_concurrent_requests: usize) -> Self {
        Self::new_with_limits_and_base_url(max_concurrent_requests, None)
    }

    pub fn new_with_limits_and_base_url(
        max_concurrent_requests: usize,
        base_url: Option<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            rate_limiter: Arc::new(Semaphore::new(max_concurrent_requests.max(1))),
            cache: Cache::new(10_000),
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
            .map_err(|source| LyricsError::InvalidBaseUrl(source.to_string()))?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| LyricsError::InvalidBaseUrl(self.base_url.clone()))?;
            segments.extend(["v1", artist_name, track_title]);
        }

        debug!(target: "lyrics", url = %url, "Fetching lyrics metadata");

        let response = self.client.get(url).send().await?;
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
