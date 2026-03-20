use moka::sync::Cache;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{self, Value};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{debug, instrument};

use crate::http_retry;

pub struct FanartTvClient {
    api_key: String,
    client_key: Option<String>,
    client: Client,
    rate_limiter: Arc<Semaphore>,
    cache_artist: Cache<String, ArtistArtwork>,
    cache_album: Cache<String, AlbumArtwork>,
    base_url: String,
}

impl FanartTvClient {
    pub fn new(api_key: String, client_key: Option<String>, base_url: Option<String>) -> Self {
        Self::new_with_limits_and_base_url(api_key, client_key, 1, base_url)
    }

    pub fn new_with_limits(
        api_key: String,
        client_key: Option<String>,
        max_concurrent_requests: usize,
    ) -> Self {
        Self::new_with_limits_and_base_url(api_key, client_key, max_concurrent_requests, None)
    }

    pub fn new_with_limits_and_base_url(
        api_key: String,
        client_key: Option<String>,
        max_concurrent_requests: usize,
        base_url: Option<String>,
    ) -> Self {
        Self::new_with_limits_cache_timeout_and_base_url(
            api_key,
            client_key,
            max_concurrent_requests,
            5_000,
            5_000,
            crate::DEFAULT_REQUEST_TIMEOUT_SECS,
            base_url,
        )
    }

    /// Creates a `FanartTvClient` with concurrency limits, explicit cache capacities, and optional
    /// base URL.
    ///
    /// A default request timeout of [`crate::DEFAULT_REQUEST_TIMEOUT_SECS`] seconds is applied.
    /// Use [`FanartTvClient::new_with_limits_cache_timeout_and_base_url`] to supply an explicit
    /// timeout instead.
    pub fn new_with_limits_cache_and_base_url(
        api_key: String,
        client_key: Option<String>,
        max_concurrent_requests: usize,
        artist_cache_capacity: u64,
        album_cache_capacity: u64,
        base_url: Option<String>,
    ) -> Self {
        Self::new_with_limits_cache_timeout_and_base_url(
            api_key,
            client_key,
            max_concurrent_requests,
            artist_cache_capacity,
            album_cache_capacity,
            crate::DEFAULT_REQUEST_TIMEOUT_SECS,
            base_url,
        )
    }

    /// Creates a `FanartTvClient` with concurrency limits, explicit cache capacities,
    /// explicit request timeout, and optional base URL.
    pub fn new_with_limits_cache_timeout_and_base_url(
        api_key: String,
        client_key: Option<String>,
        max_concurrent_requests: usize,
        artist_cache_capacity: u64,
        album_cache_capacity: u64,
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
                    "Failed to build Fanart.tv HTTP client with timeout, falling back to default client"
                );
                Client::new()
            });
        Self {
            api_key,
            client_key,
            client,
            rate_limiter: Arc::new(Semaphore::new(max_concurrent_requests.max(1))),
            cache_artist: Cache::new(artist_cache_capacity.max(1)),
            cache_album: Cache::new(album_cache_capacity.max(1)),
            base_url: base_url
                .unwrap_or_else(|| "https://webservice.fanart.tv/v3".to_string())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    #[instrument(skip(self), fields(artist_mbid = artist_mbid))]
    pub async fn fetch_artist_artwork(
        &self,
        artist_mbid: &str,
    ) -> Result<ArtistArtwork, FanartTvError> {
        if let Some(cached) = self.cache_artist.get(artist_mbid) {
            return Ok(cached);
        }

        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|_| FanartTvError::RateLimiterClosed)?;

        let url = format!("{}/music/{}", self.base_url, artist_mbid);
        debug!(target: "fanarttv", url = %url, "fetching artist artwork");

        let response =
            http_retry::send_with_retry(|| self.request(self.client.get(&url)), "fanarttv").await?;

        let status = response.status();
        let body = response.text().await?;
        let value = parse_fanarttv_body(status, &body)?;
        let artwork = parse_artist_artwork(value)?;
        self.cache_artist
            .insert(artist_mbid.to_string(), artwork.clone());
        Ok(artwork)
    }

    #[instrument(skip(self), fields(release_group_mbid = release_group_mbid))]
    pub async fn fetch_album_artwork(
        &self,
        release_group_mbid: &str,
    ) -> Result<AlbumArtwork, FanartTvError> {
        if let Some(cached) = self.cache_album.get(release_group_mbid) {
            return Ok(cached);
        }

        let _permit = self
            .rate_limiter
            .acquire()
            .await
            .map_err(|_| FanartTvError::RateLimiterClosed)?;

        let url = format!("{}/music/albums/{}", self.base_url, release_group_mbid);
        debug!(target: "fanarttv", url = %url, "fetching album artwork");

        let response =
            http_retry::send_with_retry(|| self.request(self.client.get(&url)), "fanarttv").await?;

        let status = response.status();
        let body = response.text().await?;
        let value = parse_fanarttv_body(status, &body)?;
        let artwork = parse_album_artwork(value)?;
        self.cache_album
            .insert(release_group_mbid.to_string(), artwork.clone());
        Ok(artwork)
    }

    fn request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let request = request.header("api-key", self.api_key.trim());
        match self.client_key.as_deref() {
            Some(client_key) if !client_key.trim().is_empty() => {
                request.header("client-key", client_key.trim())
            }
            _ => request,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtworkImage {
    pub url: String,
    pub likes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistArtwork {
    pub backgrounds: Vec<ArtworkImage>,
    pub logos: Vec<ArtworkImage>,
    pub thumbs: Vec<ArtworkImage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumArtwork {
    pub covers: Vec<ArtworkImage>,
    pub cdarts: Vec<ArtworkImage>,
}

#[derive(Debug, Error)]
pub enum FanartTvError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP status {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },
    #[error("Fanart.tv API error: {message}")]
    Api { message: String },
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
    #[error("Rate limiter closed")]
    RateLimiterClosed,
}

#[derive(Debug, Deserialize)]
struct ArtworkItem {
    url: String,
    likes: Option<String>,
}

fn parse_fanarttv_body(status: StatusCode, response_body: &str) -> Result<Value, FanartTvError> {
    if !status.is_success() {
        return Err(FanartTvError::HttpStatus {
            status,
            body: response_body.to_string(),
        });
    }

    let value: Value = serde_json::from_str(response_body)?;
    if let Some(message) = value
        .get("error")
        .and_then(|error| error.as_str())
        .or_else(|| value.get("message").and_then(|message| message.as_str()))
    {
        return Err(FanartTvError::Api {
            message: message.to_string(),
        });
    }

    Ok(value)
}

fn parse_artist_artwork(value: Value) -> Result<ArtistArtwork, FanartTvError> {
    Ok(ArtistArtwork {
        backgrounds: parse_images(&value, "artistbackground")?,
        logos: parse_images(&value, "hdmusiclogo")?,
        thumbs: parse_images(&value, "artistthumb")?,
    })
}

fn parse_album_artwork(value: Value) -> Result<AlbumArtwork, FanartTvError> {
    Ok(AlbumArtwork {
        covers: parse_images(&value, "albumcover")?,
        cdarts: parse_images(&value, "cdart")?,
    })
}

fn parse_images(value: &Value, field: &str) -> Result<Vec<ArtworkImage>, FanartTvError> {
    let Some(raw_images) = value.get(field) else {
        return Ok(Vec::new());
    };

    let items: Vec<ArtworkItem> = serde_json::from_value(raw_images.clone())?;
    Ok(items
        .into_iter()
        .map(|item| ArtworkImage {
            url: item.url,
            likes: item.likes.and_then(|likes| likes.parse::<u32>().ok()),
        })
        .collect())
}
