//! Discogs API client implementation

use moka::sync::Cache;
use reqwest::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{self, Value};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, instrument};

/// Internal time-based rate limiter for Discogs API calls.
///
/// Discogs rate limit: 60 authenticated requests per minute (~1/sec).
/// This uses a semaphore combined with a minimum-interval delay to avoid
/// hitting the API quota even when many permits are available.
struct DiscogsRateLimiter {
    semaphore: Arc<Semaphore>,
    min_interval: Duration,
    last_request: Arc<tokio::sync::Mutex<Option<Instant>>>,
}

impl DiscogsRateLimiter {
    fn new(max_concurrent: usize, min_interval: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
            min_interval,
            last_request: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    async fn acquire(&self) -> Result<tokio::sync::SemaphorePermit<'_>, DiscogsError> {
        let permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| DiscogsError::RateLimiterClosed)?;

        let mut last = self.last_request.lock().await;
        if let Some(last_instant) = *last {
            let elapsed = last_instant.elapsed();
            if elapsed < self.min_interval {
                let wait = self.min_interval - elapsed;
                debug!(target: "discogs", "rate limiting: waiting {:?}", wait);
                sleep(wait).await;
            }
        }
        *last = Some(Instant::now());

        Ok(permit)
    }
}

/// Struct representing the Discogs API client.
pub struct DiscogsClient {
    token: Option<String>,
    client: Client,
    rate_limiter: DiscogsRateLimiter,
    cache_artist: Cache<String, ArtistMetadata>,
    cache_album: Cache<String, AlbumMetadata>,
    /// Base URL stored without a trailing slash.
    base_url: String,
}

impl DiscogsClient {
    /// Creates a new Discogs API client with default concurrency and base URL.
    pub fn new(token: Option<String>, base_url: Option<String>) -> Self {
        let client = Self::new_with_limits_and_base_url(token, 1, base_url);
        debug!(target: "discogs", base_url = %client.base_url, "Initialized Discogs client");
        client
    }

    /// Creates a new Discogs API client with concurrency limiting and default base URL.
    pub fn new_with_limits(token: Option<String>, max_concurrent_requests: usize) -> Self {
        Self::new_with_limits_and_base_url(token, max_concurrent_requests, None)
    }

    /// Creates a new Discogs API client with concurrency limiting, caching, and optional base URL.
    pub fn new_with_limits_and_base_url(
        token: Option<String>,
        max_concurrent_requests: usize,
        base_url: Option<String>,
    ) -> Self {
        let client = Client::builder()
            .user_agent(concat!(
                "chorrosion/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/SvetaKrava/chorrosion)"
            ))
            .build()
            .unwrap_or_else(|error| {
                debug!(
                    ?error,
                    "Failed to build Discogs HTTP client with custom user agent, falling back to default client"
                );
                Client::new()
            });

        Self {
            token,
            client,
            // Discogs allows 60 authenticated requests/min (~1/sec); enforce that interval.
            rate_limiter: DiscogsRateLimiter::new(
                max_concurrent_requests,
                Duration::from_secs(1),
            ),
            cache_artist: Cache::new(10_000),
            cache_album: Cache::new(10_000),
            // Trim trailing slash once at construction so every URL format is clean.
            base_url: base_url
                .unwrap_or_else(|| "https://api.discogs.com".to_string())
                .trim_end_matches('/')
                .to_string(),
        }
    }

    /// Fetches metadata for an artist by querying Discogs search and artist detail endpoints.
    #[instrument(skip(self), fields(artist = artist_name))]
    pub async fn fetch_artist_metadata(
        &self,
        artist_name: &str,
    ) -> Result<ArtistMetadata, DiscogsError> {
        if let Some(cached) = self.cache_artist.get(artist_name) {
            return Ok(cached);
        }

        let search_url = format!("{}/database/search", self.base_url);
        debug!(target: "discogs", url = %search_url, "Searching Discogs artist metadata");

        let search_response = {
            let _permit = self.rate_limiter.acquire().await?;
            self.request(self.client.get(&search_url))
                .query(&[("type", "artist"), ("q", artist_name)])
                .send()
                .await?
        };
        let search_status = search_response.status();
        let search_body = search_response.text().await?;
        let search_value = parse_discogs_body(search_status, &search_body)?;
        let search: SearchResponse = serde_json::from_value(search_value)?;

        let first = search
            .results
            .first()
            .ok_or(DiscogsError::MissingField("results[0]"))?;
        let artist_id = first
            .id
            .ok_or(DiscogsError::MissingField("results[0].id"))?;

        let artist_url = format!("{}/artists/{}", self.base_url, artist_id);
        debug!(target: "discogs", url = %artist_url, "Fetching Discogs artist detail");

        let detail_response = {
            let _permit = self.rate_limiter.acquire().await?;
            self.request(self.client.get(&artist_url)).send().await?
        };
        let detail_status = detail_response.status();
        let detail_body = detail_response.text().await?;
        let detail_value = parse_discogs_body(detail_status, &detail_body)?;
        let detail: ArtistDetailResponse = serde_json::from_value(detail_value)?;

        let metadata = ArtistMetadata {
            name: detail
                .name
                .or_else(|| first.title.clone())
                .unwrap_or_else(|| artist_name.to_string()),
            profile: detail.profile,
            genres: first.genre.clone().filter(|genres| !genres.is_empty()),
            styles: first.style.clone().filter(|styles| !styles.is_empty()),
        };

        self.cache_artist
            .insert(artist_name.to_string(), metadata.clone());
        Ok(metadata)
    }

    /// Fetches metadata for an album by querying Discogs search and release detail endpoints.
    #[instrument(skip(self), fields(artist = artist_name, album = album_name))]
    pub async fn fetch_album_metadata(
        &self,
        artist_name: &str,
        album_name: &str,
    ) -> Result<AlbumMetadata, DiscogsError> {
        let cache_key = format!("{}:{}", artist_name, album_name);
        if let Some(cached) = self.cache_album.get(&cache_key) {
            return Ok(cached);
        }

        let search_url = format!("{}/database/search", self.base_url);
        debug!(target: "discogs", url = %search_url, "Searching Discogs album metadata");

        let search_response = {
            let _permit = self.rate_limiter.acquire().await?;
            self.request(self.client.get(&search_url))
                .query(&[
                    ("type", "release"),
                    ("artist", artist_name),
                    ("release_title", album_name),
                ])
                .send()
                .await?
        };
        let search_status = search_response.status();
        let search_body = search_response.text().await?;
        let search_value = parse_discogs_body(search_status, &search_body)?;
        let search: SearchResponse = serde_json::from_value(search_value)?;

        let first = search
            .results
            .first()
            .ok_or(DiscogsError::MissingField("results[0]"))?;
        let release_id = first
            .id
            .ok_or(DiscogsError::MissingField("results[0].id"))?;

        let release_url = format!("{}/releases/{}", self.base_url, release_id);
        debug!(target: "discogs", url = %release_url, "Fetching Discogs release detail");

        let detail_response = {
            let _permit = self.rate_limiter.acquire().await?;
            self.request(self.client.get(&release_url)).send().await?
        };
        let detail_status = detail_response.status();
        let detail_body = detail_response.text().await?;
        let detail_value = parse_discogs_body(detail_status, &detail_body)?;
        let detail: ReleaseDetailResponse = serde_json::from_value(detail_value)?;

        let artist = detail
            .artists
            .as_ref()
            .and_then(|artists| artists.first())
            .and_then(|artist| artist.name.clone())
            .or_else(|| {
                first
                    .artists
                    .as_ref()
                    .and_then(|artists| artists.first())
                    .and_then(|artist| artist.name.clone())
            })
            .unwrap_or_else(|| artist_name.to_string());

        let metadata = AlbumMetadata {
            title: detail
                .title
                .or_else(|| first.title.clone())
                .unwrap_or_else(|| album_name.to_string()),
            artist,
            year: detail.year.or(first.year),
            genres: detail
                .genres
                .or_else(|| first.genre.clone())
                .filter(|genres| !genres.is_empty()),
            styles: detail
                .styles
                .or_else(|| first.style.clone())
                .filter(|styles| !styles.is_empty()),
        };

        self.cache_album.insert(cache_key, metadata.clone());
        Ok(metadata)
    }

    fn request(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.token.as_deref() {
            Some(token) if !token.trim().is_empty() => {
                // Discogs uses its own token scheme (not Bearer): "Discogs token=<value>"
                // See: https://www.discogs.com/developers/#page:authentication
                request.header("Authorization", format!("Discogs token={}", token.trim()))
            }
            _ => request,
        }
    }
}

/// Struct representing artist metadata from Discogs.
#[derive(Debug, Deserialize, Clone)]
pub struct ArtistMetadata {
    /// The artist's name as returned by the Discogs API.
    pub name: String,
    /// A free-form profile or biography text for the artist, if available.
    pub profile: Option<String>,
    /// High-level genres associated with the artist, if provided.
    pub genres: Option<Vec<String>>,
    /// More specific styles associated with the artist, if provided.
    pub styles: Option<Vec<String>>,
}

/// Struct representing album (release) metadata from Discogs.
#[derive(Debug, Deserialize, Clone)]
pub struct AlbumMetadata {
    /// The album or release title as returned by the Discogs API.
    pub title: String,
    /// The primary artist name associated with this release.
    pub artist: String,
    /// The year this release was issued, if known.
    pub year: Option<u16>,
    /// High-level genres associated with this release, if provided.
    pub genres: Option<Vec<String>>,
    /// More specific styles associated with this release, if provided.
    pub styles: Option<Vec<String>>,
}

/// Error type returned by the Discogs API client.
#[derive(Debug, Error)]
pub enum DiscogsError {
    /// An error occurred while performing the HTTP request (network or protocol failure).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// Discogs responded with a non-success HTTP status code.
    #[error("HTTP status {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },
    /// Discogs returned a JSON payload with a `message` field indicating an API-level error.
    #[error("Discogs API error: {message}")]
    Api { message: String },
    /// Failed to deserialize the Discogs JSON response into the expected types.
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
    /// A required field was absent from an otherwise successful Discogs response.
    /// The string names the missing field.
    #[error("Missing expected field: {0}")]
    MissingField(&'static str),
    /// The internal rate-limiter semaphore was closed, typically indicating shutdown
    /// or a misconfiguration of the client.
    #[error("Rate limiter closed")]
    RateLimiterClosed,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<SearchResultItem>,
}

#[derive(Debug, Deserialize, Clone)]
struct SearchResultItem {
    id: Option<u64>,
    title: Option<String>,
    year: Option<u16>,
    #[serde(default)]
    genre: Option<Vec<String>>,
    #[serde(default)]
    style: Option<Vec<String>>,
    #[serde(default)]
    artists: Option<Vec<DiscogsArtistRef>>,
}

#[derive(Debug, Deserialize)]
struct ArtistDetailResponse {
    name: Option<String>,
    profile: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseDetailResponse {
    title: Option<String>,
    year: Option<u16>,
    #[serde(default)]
    genres: Option<Vec<String>>,
    #[serde(default)]
    styles: Option<Vec<String>>,
    #[serde(default)]
    artists: Option<Vec<DiscogsArtistRef>>,
}

#[derive(Debug, Deserialize, Clone)]
struct DiscogsArtistRef {
    name: Option<String>,
}

fn parse_discogs_body(status: StatusCode, response_body: &str) -> Result<Value, DiscogsError> {
    if !status.is_success() {
        return Err(DiscogsError::HttpStatus {
            status,
            body: response_body.to_string(),
        });
    }

    let value: Value = serde_json::from_str(response_body)?;
    if let Some(message) = value.get("message").and_then(|message| message.as_str()) {
        return Err(DiscogsError::Api {
            message: message.to_string(),
        });
    }

    Ok(value)
}
