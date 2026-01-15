//! Last.fm API client implementation

use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use moka::sync::Cache;
use tokio::sync::Semaphore;

/// Struct representing the Last.fm API client.
pub struct LastFmClient {
    api_key: String,
    client: Client,
    rate_limiter: Arc<Semaphore>,
    cache_artist: Cache<String, ArtistMetadata>,
    cache_album: Cache<String, AlbumMetadata>,
}

impl LastFmClient {
    /// Creates a new Last.fm API client.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            rate_limiter: Arc::new(Semaphore::new(1)),
            cache_artist: Cache::new(10_000),
            cache_album: Cache::new(10_000),
        }
    }

    /// Creates a new Last.fm API client with rate limiting and caching.
    pub fn new_with_limits(api_key: String, max_requests_per_second: usize) -> Self {
        let client = Client::new();
        let rate_limiter = Arc::new(Semaphore::new(max_requests_per_second));
        let cache_artist = Cache::new(10_000); // Cache up to 10,000 entries
        let cache_album = Cache::new(10_000); // Cache up to 10,000 entries

        Self {
            api_key,
            client,
            rate_limiter,
            cache_artist,
            cache_album,
        }
    }

    /// Fetches metadata for an artist.
    pub async fn fetch_artist_metadata(&self, artist_name: &str) -> Result<ArtistMetadata, reqwest::Error> {
        if let Some(cached) = self.cache_artist.get(artist_name) {
            return Ok(cached);
        }

        let _permit = self.rate_limiter.acquire().await.unwrap();
        let url = "https://ws.audioscrobbler.com/2.0/";
        let params = [
            ("method", "artist.getinfo"),
            ("artist", artist_name),
            ("api_key", &self.api_key),
            ("format", "json"),
        ];

        let response = self.client.get(url).query(&params).send().await?;
        let metadata = response.json::<ArtistMetadata>().await?;
        self.cache_artist.insert(artist_name.to_string(), metadata.clone());
        Ok(metadata)
    }

    /// Fetches metadata for an album.
    pub async fn fetch_album_metadata(&self, artist_name: &str, album_name: &str) -> Result<AlbumMetadata, reqwest::Error> {
        let cache_key = format!("{}:{}", artist_name, album_name);
        if let Some(cached) = self.cache_album.get(&cache_key) {
            return Ok(cached);
        }

        let _permit = self.rate_limiter.acquire().await.unwrap();
        let url = "https://ws.audioscrobbler.com/2.0/";
        let params = [
            ("method", "album.getinfo"),
            ("artist", artist_name),
            ("album", album_name),
            ("api_key", &self.api_key),
            ("format", "json"),
        ];

        let response = self.client.get(url).query(&params).send().await?;
        let metadata = response.json::<AlbumMetadata>().await?;
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