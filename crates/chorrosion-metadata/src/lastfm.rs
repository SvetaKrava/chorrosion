//! Last.fm API client implementation

use moka::sync::Cache;
use reqwest::Client;
use serde::Deserialize;
use serde_json;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::instrument;

/// Struct representing the Last.fm API client.
pub struct LastFmClient {
    api_key: String,
    client: Client,
    rate_limiter: Arc<Semaphore>,
    cache_artist: Cache<String, ArtistMetadata>,
    cache_album: Cache<String, AlbumMetadata>,
    base_url: String,
}

impl LastFmClient {
    /// Creates a new Last.fm API client.
    pub fn new(api_key: String, base_url: Option<String>) -> Self {
        let base_url = base_url.unwrap_or_else(|| "https://ws.audioscrobbler.com/2.0/".to_string());
        let client = Self {
            api_key,
            client: Client::new(),
            rate_limiter: Arc::new(Semaphore::new(1)),
            cache_artist: Cache::new(10_000),
            cache_album: Cache::new(10_000),
            base_url,
        };
        println!("Mock server base URL: {}", client.base_url);
        client
    }

    /// Creates a new Last.fm API client with concurrency limiting and caching.
    ///
    /// `max_concurrent_requests` controls the maximum number of simultaneous in-flight
    /// HTTP requests this client will perform.
    pub fn new_with_limits(api_key: String, max_concurrent_requests: usize) -> Self {
        let client = Client::new();
        let rate_limiter = Arc::new(Semaphore::new(max_concurrent_requests));
        let cache_artist = Cache::new(10_000); // Cache up to 10,000 entries
        let cache_album = Cache::new(10_000); // Cache up to 10,000 entries

        Self {
            api_key,
            client,
            rate_limiter,
            cache_artist,
            cache_album,
            base_url: "https://ws.audioscrobbler.com/2.0/".to_string(),
        }
    }

    /// Fetches metadata for an artist.
    #[instrument(skip(self), fields(artist = artist_name))]
    pub async fn fetch_artist_metadata(&self, artist_name: &str) -> Result<ArtistMetadata, LastFmError> {
        if let Some(cached) = self.cache_artist.get(artist_name) {
            return Ok(cached);
        }

        let _permit = self.rate_limiter.acquire().await.map_err(|_| LastFmError::RateLimiterClosed)?;
        let url = &self.base_url;
        let params = [
            ("method", "artist.getinfo"),
            ("artist", artist_name),
            ("api_key", &self.api_key),
            ("format", "json"),
        ];

        println!("Request URL: {}", url);
        println!("Request Params: {:?}", params);
        println!("Sending request to mock server: {}", url);

        let response = self.client.get(url).query(&params).send().await?;
        let response_body = response.text().await?;
        println!("Raw response: {:?}", response_body);
        let metadata: ArtistMetadata = serde_json::from_str(&response_body)?;
        self.cache_artist.insert(artist_name.to_string(), metadata.clone());
        Ok(metadata)
    }

    /// Fetches metadata for an album.
    #[instrument(skip(self), fields(artist = artist_name, album = album_name))]
    pub async fn fetch_album_metadata(&self, artist_name: &str, album_name: &str) -> Result<AlbumMetadata, LastFmError> {
        let cache_key = format!("{}:{}", artist_name, album_name);
        if let Some(cached) = self.cache_album.get(&cache_key) {
            return Ok(cached);
        }

        let _permit = self.rate_limiter.acquire().await.map_err(|_| LastFmError::RateLimiterClosed)?;
        let url = &self.base_url;
        let params = [
            ("method", "album.getinfo"),
            ("artist", artist_name),
            ("album", album_name),
            ("api_key", &self.api_key),
            ("format", "json"),
        ];

        println!("Request URL: {}", url);
        println!("Request Params: {:?}", params);

        let response = self.client.get(url).query(&params).send().await?;
        let response_body = response.text().await?;
        println!("Raw response: {:?}", response_body);
        let metadata: AlbumMetadata = serde_json::from_str(&response_body)?;
        self.cache_album.insert(cache_key, metadata.clone());
        Ok(metadata)
    }
}

/// Struct representing artist metadata.
#[derive(Debug, Deserialize, Clone)]
pub struct ArtistMetadata {
    pub name: String,
    pub bio: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Struct representing album metadata.
#[derive(Debug, Deserialize, Clone)]
pub struct AlbumMetadata {
    pub title: String,
    pub artist: String,
    pub tracks: Option<Vec<String>>,
}

#[derive(Debug, Error)]
pub enum LastFmError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
    #[error("Rate limiter closed")]
    RateLimiterClosed,
}