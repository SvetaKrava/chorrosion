// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Artist information from MusicBrainz.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artist {
    /// MusicBrainz artist ID (MBID).
    pub id: Uuid,
    /// Artist name.
    pub name: String,
    /// Disambiguation comment (e.g., "US hip hop artist").
    #[serde(default)]
    pub disambiguation: Option<String>,
    /// Artist sort name (for sorting).
    #[serde(rename = "sort-name")]
    pub sort_name: String,
    /// Artist type (e.g., "Person", "Group").
    #[serde(rename = "type")]
    pub artist_type: Option<String>,
    /// Country code (ISO 3166-1 alpha-2).
    pub country: Option<String>,
    /// Search score (only present in search results).
    #[serde(default)]
    pub score: Option<u32>,
}

/// Album (release group) information from MusicBrainz.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Album {
    /// MusicBrainz release group ID (MBID).
    pub id: Uuid,
    /// Album title.
    pub title: String,
    /// Primary type (e.g., "Album", "EP", "Single").
    #[serde(rename = "primary-type")]
    pub primary_type: Option<String>,
    /// Secondary types (e.g., ["Compilation", "Live"]).
    #[serde(rename = "secondary-types", default)]
    pub secondary_types: Vec<String>,
    /// First release date (YYYY, YYYY-MM, or YYYY-MM-DD).
    #[serde(rename = "first-release-date")]
    pub first_release_date: Option<String>,
    /// Artist credit for the album.
    #[serde(rename = "artist-credit", default)]
    pub artist_credit: Vec<ArtistCredit>,
    /// Search score (only present in search results).
    #[serde(default)]
    pub score: Option<u32>,
}

/// Artist credit entry (artist contribution to a release).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtistCredit {
    /// Name as credited on the release.
    pub name: String,
    /// Artist details.
    pub artist: ArtistRef,
    /// Join phrase (e.g., " & ", " feat. ").
    #[serde(default)]
    pub joinphrase: Option<String>,
}

/// Reference to an artist (minimal info).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtistRef {
    /// MusicBrainz artist ID.
    pub id: Uuid,
    /// Artist name.
    pub name: String,
    /// Artist sort name.
    #[serde(rename = "sort-name")]
    pub sort_name: String,
}

/// Search query parameters.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// Search query string.
    pub query: String,
    /// Maximum number of results (default 25, max 100).
    pub limit: Option<u32>,
    /// Offset for pagination (default 0).
    pub offset: Option<u32>,
}

impl SearchQuery {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: None,
            offset: None,
        }
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// Generic search response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse<T> {
    /// Creation timestamp.
    pub created: String,
    /// Total number of results.
    pub count: u32,
    /// Offset used for this page.
    pub offset: u32,
    /// Results for this page.
    #[serde(flatten)]
    pub results: T,
}

/// Artist search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistSearchResult {
    pub artists: Vec<Artist>,
}

/// Album (release group) search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumSearchResult {
    #[serde(rename = "release-groups")]
    pub release_groups: Vec<Album>,
}
