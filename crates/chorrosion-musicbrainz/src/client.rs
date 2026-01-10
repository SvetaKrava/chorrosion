// SPDX-License-Identifier: GPL-3.0-or-later

use crate::error::{MusicBrainzError, Result};
use crate::models::{
    Album, AlbumSearchResult, Artist, ArtistSearchResult, CoverArtResponse, Recording, SearchQuery,
    SearchResponse,
};
use crate::rate_limiter::RateLimiter;
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, trace};
use url::Url;
use uuid::Uuid;

const MUSICBRAINZ_API_BASE: &str = "https://musicbrainz.org/ws/2";
const COVER_ART_ARCHIVE_BASE: &str = "https://coverartarchive.org";
const USER_AGENT: &str = concat!(
    "Chorrosion/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/SvetaKrava/chorrosion )"
);

/// MusicBrainz API client with rate limiting.
#[derive(Debug, Clone)]
pub struct MusicBrainzClient {
    client: Client,
    base_url: String,
    cover_art_base_url: String,
    rate_limiter: RateLimiter,
    cover_art_cache: Arc<Mutex<HashMap<Uuid, CoverArtResponse>>>,
}

impl MusicBrainzClient {
    /// Create a new MusicBrainz client with default settings.
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// Create a client builder for custom configuration.
    pub fn builder() -> MusicBrainzClientBuilder {
        MusicBrainzClientBuilder::default()
    }

    /// Search for artists by name.
    ///
    /// # Arguments
    /// * `query` - Search query parameters (query string, limit, offset).
    ///
    /// # Example
    /// ```no_run
    /// # use chorrosion_musicbrainz::{MusicBrainzClient, SearchQuery};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MusicBrainzClient::new()?;
    /// let query = SearchQuery::new("Radiohead").limit(10);
    /// let response = client.search_artists(query).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search_artists(
        &self,
        query: SearchQuery,
    ) -> Result<SearchResponse<ArtistSearchResult>> {
        let mut url = Url::parse(&format!("{}/artist", self.base_url))
            .map_err(|e| MusicBrainzError::InvalidResponse(e.to_string()))?;

        url.query_pairs_mut()
            .append_pair("query", &query.query)
            .append_pair("fmt", "json");

        if let Some(limit) = query.limit {
            url.query_pairs_mut()
                .append_pair("limit", &limit.to_string());
        }

        if let Some(offset) = query.offset {
            url.query_pairs_mut()
                .append_pair("offset", &offset.to_string());
        }

        self.get(url.as_str()).await
    }

    /// Look up an artist by MusicBrainz ID.
    ///
    /// # Arguments
    /// * `mbid` - MusicBrainz artist ID.
    ///
    /// # Example
    /// ```no_run
    /// # use chorrosion_musicbrainz::MusicBrainzClient;
    /// # use uuid::Uuid;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MusicBrainzClient::new()?;
    /// let mbid = Uuid::parse_str("a74b1b7f-71a5-4011-9441-d0b5e4122711")?; // Radiohead
    /// let artist = client.lookup_artist(mbid).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn lookup_artist(&self, mbid: Uuid) -> Result<Artist> {
        let url = format!("{}/artist/{}?fmt=json", self.base_url, mbid);
        self.get(&url).await
    }

    /// Search for albums (release groups) by title or artist.
    ///
    /// # Arguments
    /// * `query` - Search query parameters (query string, limit, offset).
    ///
    /// # Example
    /// ```no_run
    /// # use chorrosion_musicbrainz::{MusicBrainzClient, SearchQuery};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MusicBrainzClient::new()?;
    /// let query = SearchQuery::new("OK Computer").limit(10);
    /// let response = client.search_albums(query).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search_albums(
        &self,
        query: SearchQuery,
    ) -> Result<SearchResponse<AlbumSearchResult>> {
        let mut url = Url::parse(&format!("{}/release-group", self.base_url))
            .map_err(|e| MusicBrainzError::InvalidResponse(e.to_string()))?;

        url.query_pairs_mut()
            .append_pair("query", &query.query)
            .append_pair("fmt", "json");

        if let Some(limit) = query.limit {
            url.query_pairs_mut()
                .append_pair("limit", &limit.to_string());
        }

        if let Some(offset) = query.offset {
            url.query_pairs_mut()
                .append_pair("offset", &offset.to_string());
        }

        self.get(url.as_str()).await
    }

    /// Look up an album (release group) by MusicBrainz ID.
    ///
    /// # Arguments
    /// * `mbid` - MusicBrainz release group ID.
    ///
    /// # Example
    /// ```no_run
    /// # use chorrosion_musicbrainz::MusicBrainzClient;
    /// # use uuid::Uuid;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MusicBrainzClient::new()?;
    /// let mbid = Uuid::parse_str("b1392450-e666-3926-a536-22c65f834433")?; // OK Computer
    /// let album = client.lookup_album(mbid).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn lookup_album(&self, mbid: Uuid) -> Result<Album> {
        let url = format!(
            "{}/release-group/{}?fmt=json&inc=artist-credits",
            self.base_url, mbid
        );
        self.get(&url).await
    }

    /// Look up a recording (track) by MusicBrainz ID, including artist credits and releases.
    ///
    /// # Arguments
    /// * `mbid` - MusicBrainz recording ID.
    ///
    /// # Example
    /// ```no_run
    /// # use chorrosion_musicbrainz::MusicBrainzClient;
    /// # use uuid::Uuid;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = MusicBrainzClient::new()?;
    /// let mbid = Uuid::parse_str("e5a3f0c4-1fae-4f2e-8f76-0c3b4f1e4fa6")?; // Paranoid Android
    /// let recording = client.lookup_recording(mbid).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn lookup_recording(&self, mbid: Uuid) -> Result<Recording> {
        let url = format!(
            "{}/recording/{}?fmt=json&inc=artists+releases+release-groups",
            self.base_url, mbid
        );
        self.get(&url).await
    }

    /// Fetch cover art metadata for a release group from the Cover Art Archive.
    /// Results are cached in-memory for the lifetime of the client.
    pub async fn fetch_cover_art(&self, release_group_mbid: Uuid) -> Result<CoverArtResponse> {
        if let Some(cached) = self
            .cover_art_cache
            .lock()
            .await
            .get(&release_group_mbid)
            .cloned()
        {
            return Ok(cached);
        }

        let url = format!(
            "{}/release-group/{}",
            self.cover_art_base_url, release_group_mbid
        );
        let response: CoverArtResponse = self.get(&url).await?;

        self.cover_art_cache
            .lock()
            .await
            .insert(release_group_mbid, response.clone());

        Ok(response)
    }

    /// Internal method to perform rate-limited GET requests.
    async fn get<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let _permit = self.rate_limiter.acquire().await;

        trace!(target: "musicbrainz", "GET {}", url);

        let response = self
            .client
            .get(url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        let status = response.status();
        debug!(target: "musicbrainz", "response status: {}", status);

        if status == 404 {
            return Err(MusicBrainzError::NotFound(url.to_string()));
        }

        if status == 503 {
            return Err(MusicBrainzError::RateLimitExceeded);
        }

        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(MusicBrainzError::ApiError {
                status: status.as_u16(),
                message,
            });
        }

        let body = response.text().await?;
        trace!(target: "musicbrainz", "response body: {}", body);

        serde_json::from_str(&body).map_err(|e| {
            MusicBrainzError::InvalidResponse(format!("Failed to parse response: {}", e))
        })
    }
}

impl Default for MusicBrainzClient {
    fn default() -> Self {
        // Default should be infallible; if building the configured client fails,
        // fall back to a basic reqwest client while keeping sensible defaults.
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| Client::new());

        let rate_limiter = RateLimiter::new(Duration::from_secs(1));

        MusicBrainzClient {
            client,
            base_url: MUSICBRAINZ_API_BASE.to_string(),
            cover_art_base_url: COVER_ART_ARCHIVE_BASE.to_string(),
            rate_limiter,
            cover_art_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Builder for configuring a MusicBrainz client.
#[derive(Debug)]
pub struct MusicBrainzClientBuilder {
    base_url: String,
    cover_art_base_url: String,
    timeout: Duration,
    rate_limit_interval: Duration,
}

impl Default for MusicBrainzClientBuilder {
    fn default() -> Self {
        Self {
            base_url: MUSICBRAINZ_API_BASE.to_string(),
            cover_art_base_url: COVER_ART_ARCHIVE_BASE.to_string(),
            timeout: Duration::from_secs(30),
            rate_limit_interval: Duration::from_secs(1),
        }
    }
}

impl MusicBrainzClientBuilder {
    /// Set a custom base URL (useful for testing with mock servers).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set a custom Cover Art Archive base URL (useful for testing).
    pub fn cover_art_base_url(mut self, url: impl Into<String>) -> Self {
        self.cover_art_base_url = url.into();
        self
    }

    /// Set request timeout duration.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set rate limit interval between requests.
    pub fn rate_limit_interval(mut self, interval: Duration) -> Self {
        self.rate_limit_interval = interval;
        self
    }

    /// Build the MusicBrainz client.
    pub fn build(self) -> Result<MusicBrainzClient> {
        let client = Client::builder()
            .timeout(self.timeout)
            .user_agent(USER_AGENT)
            .build()?;

        let rate_limiter = RateLimiter::new(self.rate_limit_interval);

        Ok(MusicBrainzClient {
            client,
            base_url: self.base_url,
            cover_art_base_url: self.cover_art_base_url,
            rate_limiter,
            cover_art_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
