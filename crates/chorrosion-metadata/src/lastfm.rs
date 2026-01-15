//! Last.fm API client implementation

use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

/// Struct representing the Last.fm API client.
pub struct LastFmClient {
    api_key: String,
    client: Client,
}

impl LastFmClient {
    /// Creates a new Last.fm API client.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    /// Fetches metadata for an artist.
    pub async fn fetch_artist_metadata(&self, artist_name: &str) -> Result<ArtistMetadata, reqwest::Error> {
        let url = "https://ws.audioscrobbler.com/2.0/";
        let params = [
            ("method", "artist.getinfo"),
            ("artist", artist_name),
            ("api_key", &self.api_key),
            ("format", "json"),
        ];

        let response = self.client.get(url).query(&params).send().await?;
        let metadata = response.json::<ArtistMetadata>().await?;
        Ok(metadata)
    }

    /// Fetches metadata for an album.
    pub async fn fetch_album_metadata(&self, artist_name: &str, album_name: &str) -> Result<AlbumMetadata, reqwest::Error> {
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
        Ok(metadata)
    }
}

/// Struct representing artist metadata.
#[derive(Debug, Deserialize)]
pub struct ArtistMetadata {
    pub name: String,
    pub bio: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Struct representing album metadata.
#[derive(Debug, Deserialize)]
pub struct AlbumMetadata {
    pub title: String,
    pub artist: String,
    pub tracks: Option<Vec<String>>,
}