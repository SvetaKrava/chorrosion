use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Value Objects & IDs
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtistId(pub Uuid);

impl ArtistId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ArtistId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ArtistId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AlbumId(pub Uuid);

impl AlbumId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for AlbumId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AlbumId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackId(pub Uuid);

impl TrackId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for TrackId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrackId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(pub Uuid);

impl ProfileId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Enums
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtistStatus {
    Continuing,
    Ended,
}

impl std::fmt::Display for ArtistStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Continuing => write!(f, "continuing"),
            Self::Ended => write!(f, "ended"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlbumStatus {
    Wanted,
    Released,
    Announced,
}

impl std::fmt::Display for AlbumStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wanted => write!(f, "wanted"),
            Self::Released => write!(f, "released"),
            Self::Announced => write!(f, "announced"),
        }
    }
}

// ============================================================================
// Entities
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: ArtistId,
    pub name: String,
    pub foreign_artist_id: Option<String>,
    pub metadata_profile_id: Option<ProfileId>,
    pub quality_profile_id: Option<ProfileId>,
    pub status: ArtistStatus,
    pub path: Option<String>,
    pub monitored: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Artist {
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ArtistId::new(),
            name: name.into(),
            foreign_artist_id: None,
            metadata_profile_id: None,
            quality_profile_id: None,
            status: ArtistStatus::Continuing,
            path: None,
            monitored: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: AlbumId,
    pub artist_id: ArtistId,
    pub foreign_album_id: Option<String>,
    pub title: String,
    pub release_date: Option<chrono::NaiveDate>,
    pub album_type: Option<String>,
    pub status: AlbumStatus,
    pub monitored: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Album {
    pub fn new(artist_id: ArtistId, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: AlbumId::new(),
            artist_id,
            foreign_album_id: None,
            title: title.into(),
            release_date: None,
            album_type: None,
            status: AlbumStatus::Wanted,
            monitored: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub album_id: AlbumId,
    pub artist_id: ArtistId,
    pub foreign_track_id: Option<String>,
    pub title: String,
    pub track_number: Option<u32>,
    pub duration_ms: Option<u32>,
    pub has_file: bool,
    pub monitored: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Track {
    pub fn new(album_id: AlbumId, artist_id: ArtistId, title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: TrackId::new(),
            album_id,
            artist_id,
            foreign_track_id: None,
            title: title.into(),
            track_number: None,
            duration_ms: None,
            has_file: false,
            monitored: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityProfile {
    pub id: ProfileId,
    pub name: String,
    pub allowed_qualities: Vec<String>,
    pub upgrade_allowed: bool,
    pub cutoff_quality: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl QualityProfile {
    pub fn new(name: impl Into<String>, allowed_qualities: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ProfileId::new(),
            name: name.into(),
            allowed_qualities,
            upgrade_allowed: false,
            cutoff_quality: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataProfile {
    pub id: ProfileId,
    pub name: String,
    pub primary_album_types: Vec<String>,
    pub secondary_album_types: Vec<String>,
    pub release_statuses: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MetadataProfile {
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ProfileId::new(),
            name: name.into(),
            primary_album_types: vec![],
            secondary_album_types: vec![],
            release_statuses: vec![],
            created_at: now,
            updated_at: now,
        }
    }
}
