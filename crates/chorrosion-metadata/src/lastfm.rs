//! Last.fm API client implementation

use moka::sync::Cache;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::{self, Value};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{debug, instrument};

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
        debug!(base_url = %client.base_url, "Initialized Last.fm client");
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

        debug!(url = %url, "Fetching artist metadata");

        let response = self.client.get(url).query(&params).send().await?;
        let status = response.status();
        let response_body = response.text().await?;
        let value = parse_lastfm_body(status, &response_body)?;
        let metadata = parse_artist_metadata(value)?;
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

        debug!(url = %url, "Fetching album metadata");

        let response = self.client.get(url).query(&params).send().await?;
        let status = response.status();
        let response_body = response.text().await?;
        let value = parse_lastfm_body(status, &response_body)?;
        let metadata = parse_album_metadata(value)?;
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
    #[error("HTTP status {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },
    #[error("Last.fm API error {code}: {message}")]
    Api { code: i64, message: String },
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
    #[error("Missing expected field: {0}")]
    MissingField(&'static str),
    #[error("Rate limiter closed")]
    RateLimiterClosed,
}

#[derive(Debug, Deserialize)]
struct LastFmArtistResponse {
    artist: LastFmArtistPayload,
}

#[derive(Debug, Deserialize)]
struct LastFmAlbumResponse {
    album: LastFmAlbumPayload,
}

#[derive(Debug, Deserialize)]
struct LastFmArtistPayload {
    name: String,
    bio: Option<LastFmBio>,
    tags: Option<LastFmTags>,
}

#[derive(Debug, Deserialize)]
struct LastFmBio {
    summary: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LastFmTags {
    tag: Vec<LastFmTag>,
}

#[derive(Debug, Deserialize)]
struct LastFmTag {
    name: String,
}

#[derive(Debug, Deserialize)]
struct LastFmAlbumPayload {
    name: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    tracks: Option<LastFmTracks>,
}

#[derive(Debug, Deserialize)]
struct LastFmTracks {
    track: Vec<LastFmTrack>,
}

#[derive(Debug, Deserialize)]
struct LastFmTrack {
    name: String,
}

fn parse_lastfm_body(status: StatusCode, response_body: &str) -> Result<Value, LastFmError> {
    if !status.is_success() {
        return Err(LastFmError::HttpStatus {
            status,
            body: response_body.to_string(),
        });
    }

    let value: Value = serde_json::from_str(response_body)?;
    if let Some(code) = value.get("error").and_then(|v| v.as_i64()) {
        let message = value
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Last.fm error")
            .to_string();
        return Err(LastFmError::Api { code, message });
    }

    Ok(value)
}

fn parse_artist_metadata(value: Value) -> Result<ArtistMetadata, LastFmError> {
    if let Ok(response) = serde_json::from_value::<LastFmArtistResponse>(value.clone()) {
        return Ok(ArtistMetadata {
            name: response.artist.name,
            bio: response
                .artist
                .bio
                .and_then(|bio| bio.summary.or(bio.content)),
            tags: response
                .artist
                .tags
                .map(|tags| tags.tag.into_iter().map(|tag| tag.name).collect())
                .filter(|tags: &Vec<String>| !tags.is_empty()),
        });
    }

    let metadata: ArtistMetadata = serde_json::from_value(value)?;
    Ok(metadata)
}

fn parse_album_metadata(value: Value) -> Result<AlbumMetadata, LastFmError> {
    if let Ok(response) = serde_json::from_value::<LastFmAlbumResponse>(value.clone()) {
        let title = response
            .album
            .title
            .or(response.album.name)
            .ok_or(LastFmError::MissingField("album.title"))?;
        let artist = response
            .album
            .artist
            .ok_or(LastFmError::MissingField("album.artist"))?;
        let tracks = response
            .album
            .tracks
            .map(|tracks| tracks.track.into_iter().map(|track| track.name).collect())
            .filter(|tracks: &Vec<String>| !tracks.is_empty());

        return Ok(AlbumMetadata {
            title,
            artist,
            tracks,
        });
    }

    let metadata: AlbumMetadata = serde_json::from_value(value)?;
    Ok(metadata)
}