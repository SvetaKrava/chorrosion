// SPDX-License-Identifier: GPL-3.0-or-later
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtistRelationshipId(pub Uuid);

impl ArtistRelationshipId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ArtistRelationshipId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ArtistRelationshipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackFileId(pub Uuid);

impl TrackFileId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for TrackFileId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrackFileId {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseDatePrecision {
    Year,
    Month,
    Day,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDate {
    pub year: i32,
    pub month: Option<u32>,
    pub day: Option<u32>,
}

impl ReleaseDate {
    pub fn new(year: i32, month: Option<u32>, day: Option<u32>) -> Self {
        Self { year, month, day }
    }

    pub fn precision(&self) -> ReleaseDatePrecision {
        match (self.month, self.day) {
            (None, _) => ReleaseDatePrecision::Year,
            (Some(_), None) => ReleaseDatePrecision::Month,
            (Some(_), Some(_)) => ReleaseDatePrecision::Day,
        }
    }

    /// Parse a date string in various formats.
    ///
    /// Supported formats:
    /// - Year only: `2024`, `"2024"`
    /// - Year-Month: `2024-12`, `2024/12`, `202412`
    /// - Full date: `2024-12-31`, `2024/12/31`, `20241231`
    /// - ISO 8601 with timezone: `2024-12-31T00:00:00Z`, `2024-12-31T12:34:56+00:00`
    ///
    /// Returns `None` if the string cannot be parsed or contains invalid date values.
    pub fn parse_str(s: &str) -> Option<Self> {
        let s = s.trim();

        // Try ISO 8601 datetime formats first (with timezone)
        if (s.contains('T') || s.contains('Z') || s.contains('+')) && s.len() > 10 {
            if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                let date = dt.date_naive();
                let year = date.year();
                Self::validate_year(year)?;
                return Some(Self {
                    year,
                    month: Some(date.month()),
                    day: Some(date.day()),
                });
            }
            // Try parsing as UTC datetime
            if let Ok(dt) = s.parse::<DateTime<Utc>>() {
                let date = dt.date_naive();
                let year = date.year();
                Self::validate_year(year)?;
                return Some(Self {
                    year,
                    month: Some(date.month()),
                    day: Some(date.day()),
                });
            }
        }

        // Try compact format (YYYYMMDD, YYYYMM, or YYYY)
        if !s.contains('-') && !s.contains('/') {
            return Self::parse_compact(s);
        }

        // Try delimited formats (YYYY-MM-DD or YYYY/MM/DD)
        let delimiter = if s.contains('-') {
            '-'
        } else if s.contains('/') {
            '/'
        } else {
            return None;
        };
        let parts: Vec<&str> = s.split(delimiter).collect();

        match parts.len() {
            1 => {
                // Year only
                let year = parts[0].parse().ok()?;
                Self::validate_year(year)?;
                Some(Self {
                    year,
                    month: None,
                    day: None,
                })
            }
            2 => {
                // Year-Month
                let year = parts[0].parse().ok()?;
                let month: u32 = parts[1].parse().ok()?;
                Self::validate_year(year)?;
                Self::validate_month(month)?;
                Some(Self {
                    year,
                    month: Some(month),
                    day: None,
                })
            }
            3 => {
                // Full date
                let year = parts[0].parse().ok()?;
                let month: u32 = parts[1].parse().ok()?;
                let day: u32 = parts[2].parse().ok()?;
                Self::validate_year(year)?;
                Self::validate_month(month)?;
                Self::validate_day(year, month, day)?;
                Some(Self {
                    year,
                    month: Some(month),
                    day: Some(day),
                })
            }
            _ => None,
        }
    }

    /// Parse compact date formats (YYYYMMDD or YYYYMM)
    fn parse_compact(s: &str) -> Option<Self> {
        match s.len() {
            4 => {
                // Year only (YYYY)
                let year = s.parse().ok()?;
                Self::validate_year(year)?;
                Some(Self {
                    year,
                    month: None,
                    day: None,
                })
            }
            6 => {
                // YYYYMM
                let year = s[0..4].parse().ok()?;
                let month = s[4..6].parse().ok()?;
                Self::validate_year(year)?;
                Self::validate_month(month)?;
                Some(Self {
                    year,
                    month: Some(month),
                    day: None,
                })
            }
            8 => {
                // YYYYMMDD
                let year = s[0..4].parse().ok()?;
                let month = s[4..6].parse().ok()?;
                let day = s[6..8].parse().ok()?;
                Self::validate_year(year)?;
                Self::validate_month(month)?;
                Self::validate_day(year, month, day)?;
                Some(Self {
                    year,
                    month: Some(month),
                    day: Some(day),
                })
            }
            _ => None,
        }
    }

    /// Validate year (reasonable range: 1900-2100)
    fn validate_year(year: i32) -> Option<()> {
        if (1900..=2100).contains(&year) {
            Some(())
        } else {
            None
        }
    }

    /// Validate month (1-12)
    fn validate_month(month: u32) -> Option<()> {
        if (1..=12).contains(&month) {
            Some(())
        } else {
            None
        }
    }

    /// Validate day using chrono's date validation (handles leap years, month lengths)
    fn validate_day(year: i32, month: u32, day: u32) -> Option<()> {
        NaiveDate::from_ymd_opt(year, month, day)?;
        Some(())
    }

    /// Convert to a NaiveDate, using the first of the month/year when precision is lower
    pub fn to_naive_date_opt(&self) -> Option<NaiveDate> {
        match (self.month, self.day) {
            (Some(m), Some(d)) => NaiveDate::from_ymd_opt(self.year, m, d),
            (Some(m), None) => NaiveDate::from_ymd_opt(self.year, m, 1),
            (None, _) => NaiveDate::from_ymd_opt(self.year, 1, 1),
        }
    }

    /// Convert to ISO 8601 string representation
    pub fn to_iso8601(&self) -> String {
        match (self.month, self.day) {
            (Some(m), Some(d)) => format!("{:04}-{:02}-{:02}", self.year, m, d),
            (Some(m), None) => format!("{:04}-{:02}", self.year, m),
            (None, _) => format!("{:04}", self.year),
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
    pub musicbrainz_artist_id: Option<String>,
    pub metadata_profile_id: Option<ProfileId>,
    pub quality_profile_id: Option<ProfileId>,
    pub status: ArtistStatus,
    pub path: Option<String>,
    pub monitored: bool,
    pub artist_type: Option<String>,
    pub sort_name: Option<String>,
    pub country: Option<String>,
    pub disambiguation: Option<String>,
    pub genre_tags: Option<String>,
    pub style_tags: Option<String>,
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
            musicbrainz_artist_id: None,
            metadata_profile_id: None,
            quality_profile_id: None,
            status: ArtistStatus::Continuing,
            path: None,
            monitored: true,
            artist_type: None,
            sort_name: None,
            country: None,
            disambiguation: None,
            genre_tags: None,
            style_tags: None,
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
    pub musicbrainz_release_group_id: Option<String>,
    pub musicbrainz_release_id: Option<String>,
    pub title: String,
    pub release_date: Option<chrono::NaiveDate>,
    pub album_type: Option<String>,
    pub primary_type: Option<String>,
    pub secondary_types: Option<String>,
    pub first_release_date: Option<String>,
    pub genre_tags: Option<String>,
    pub style_tags: Option<String>,
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
            musicbrainz_release_group_id: None,
            musicbrainz_release_id: None,
            title: title.into(),
            release_date: None,
            album_type: None,
            primary_type: None,
            secondary_types: None,
            first_release_date: None,
            genre_tags: None,
            style_tags: None,
            status: AlbumStatus::Wanted,
            monitored: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistRelationship {
    pub id: ArtistRelationshipId,
    pub source_artist_id: ArtistId,
    pub related_artist_id: ArtistId,
    pub relationship_type: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ArtistRelationship {
    pub fn new(
        source_artist_id: ArtistId,
        related_artist_id: ArtistId,
        relationship_type: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: ArtistRelationshipId::new(),
            source_artist_id,
            related_artist_id,
            relationship_type: relationship_type.into(),
            description: None,
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
    pub musicbrainz_recording_id: Option<String>,
    pub match_confidence: Option<f32>,
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
            musicbrainz_recording_id: None,
            match_confidence: None,
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

// ============================================================================
// Track File (represents a physical audio file associated to a Track)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackFile {
    pub id: TrackFileId,
    pub track_id: TrackId,
    pub path: String,
    pub size_bytes: u64,
    pub duration_ms: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub channels: Option<u8>,
    pub codec: Option<String>,
    pub hash: Option<String>,
    pub fingerprint_hash: Option<String>,
    pub fingerprint_duration: Option<u32>,
    pub fingerprint_computed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TrackFile {
    pub fn new(track_id: TrackId, path: impl Into<String>, size_bytes: u64) -> Self {
        let now = Utc::now();
        Self {
            id: TrackFileId::new(),
            track_id,
            path: path.into(),
            size_bytes,
            duration_ms: None,
            bitrate_kbps: None,
            channels: None,
            codec: None,
            hash: None,
            fingerprint_hash: None,
            fingerprint_duration: None,
            fingerprint_computed_at: None,
            created_at: now,
            updated_at: now,
        }
    }
}

// ============================================================================
// Domain Validation
// ============================================================================

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: &'static str,
    pub message: String,
}

pub trait Validate {
    fn validate(&self) -> Result<(), Vec<ValidationError>>;
}

impl Validate for Artist {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push(ValidationError {
                field: "name",
                message: "name cannot be empty".into(),
            });
        }
        if let Some(path) = &self.path {
            if path.trim().is_empty() {
                errors.push(ValidationError {
                    field: "path",
                    message: "path cannot be empty when provided".into(),
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Validate for Album {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        if self.title.trim().is_empty() {
            errors.push(ValidationError {
                field: "title",
                message: "title cannot be empty".into(),
            });
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Validate for Track {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        if self.title.trim().is_empty() {
            errors.push(ValidationError {
                field: "title",
                message: "title cannot be empty".into(),
            });
        }
        if let Some(n) = self.track_number {
            if n == 0 {
                errors.push(ValidationError {
                    field: "track_number",
                    message: "track number must be >= 1".into(),
                });
            }
        }
        if let Some(d) = self.duration_ms {
            if d == 0 {
                errors.push(ValidationError {
                    field: "duration_ms",
                    message: "duration must be > 0".into(),
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Validate for QualityProfile {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push(ValidationError {
                field: "name",
                message: "name cannot be empty".into(),
            });
        }
        if self.allowed_qualities.is_empty() {
            errors.push(ValidationError {
                field: "allowed_qualities",
                message: "at least one quality must be allowed".into(),
            });
        }
        if let Some(cutoff) = &self.cutoff_quality {
            if !self
                .allowed_qualities
                .iter()
                .any(|q| q.eq_ignore_ascii_case(cutoff))
            {
                errors.push(ValidationError {
                    field: "cutoff_quality",
                    message: "cutoff must be one of allowed_qualities".into(),
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Validate for MetadataProfile {
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push(ValidationError {
                field: "name",
                message: "name cannot be empty".into(),
            });
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ============================================================================
// File Path Generation Utilities
// ============================================================================

fn sanitize_component(input: &str) -> String {
    // Remove characters invalid on Windows and common problematic ones
    let banned = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    input
        .chars()
        .map(|c| if banned.contains(&c) { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn generate_track_path(
    base: &Path,
    artist: &str,
    album: &str,
    track_number: Option<u32>,
    track_title: &str,
    extension: &str,
) -> PathBuf {
    let artist_component = sanitize_component(artist);
    let album_component = sanitize_component(album);
    let title_component = sanitize_component(track_title);
    let file_stem = match track_number {
        Some(n) if n > 0 => format!("{:02} - {}", n, title_component),
        _ => title_component,
    };
    let file_name = if extension.is_empty() {
        file_stem
    } else {
        format!("{}.{}", file_stem, extension.trim_start_matches('.'))
    };
    base.join(artist_component)
        .join(album_component)
        .join(file_name)
}

// ============================================================================
// Domain Events (lightweight scaffolding)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEvent<TPayload> {
    pub name: &'static str,
    pub occurred_at: DateTime<Utc>,
    pub payload: TPayload,
}

impl<TPayload> DomainEvent<TPayload> {
    pub fn new(name: &'static str, payload: TPayload) -> Self {
        Self {
            name,
            occurred_at: Utc::now(),
            payload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackFileImportedPayload {
    pub track_id: TrackId,
    pub track_file_id: TrackFileId,
    pub path: String,
}

pub type TrackFileImported = DomainEvent<TrackFileImportedPayload>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistCreatedPayload {
    pub artist_id: ArtistId,
    pub name: String,
    pub monitored: bool,
}

pub type ArtistCreated = DomainEvent<ArtistCreatedPayload>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistUpdatedPayload {
    pub artist_id: ArtistId,
    pub name: String,
    pub monitored: bool,
}

pub type ArtistUpdated = DomainEvent<ArtistUpdatedPayload>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumCreatedPayload {
    pub album_id: AlbumId,
    pub artist_id: ArtistId,
    pub title: String,
    pub monitored: bool,
}

pub type AlbumCreated = DomainEvent<AlbumCreatedPayload>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumUpdatedPayload {
    pub album_id: AlbumId,
    pub artist_id: ArtistId,
    pub title: String,
    pub monitored: bool,
}

pub type AlbumUpdated = DomainEvent<AlbumUpdatedPayload>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackCreatedPayload {
    pub track_id: TrackId,
    pub album_id: AlbumId,
    pub artist_id: ArtistId,
    pub title: String,
}

pub type TrackCreated = DomainEvent<TrackCreatedPayload>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackUpdatedPayload {
    pub track_id: TrackId,
    pub album_id: AlbumId,
    pub artist_id: ArtistId,
    pub title: String,
}

pub type TrackUpdated = DomainEvent<TrackUpdatedPayload>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_date_precision_and_parse() {
        let y = ReleaseDate::parse_str("2024").unwrap();
        assert_eq!(y.precision(), ReleaseDatePrecision::Year);
        assert_eq!(y.to_naive_date_opt(), NaiveDate::from_ymd_opt(2024, 1, 1));

        let ym = ReleaseDate::parse_str("2024-12").unwrap();
        assert_eq!(ym.precision(), ReleaseDatePrecision::Month);
        assert_eq!(ym.to_naive_date_opt(), NaiveDate::from_ymd_opt(2024, 12, 1));

        let ymd = ReleaseDate::parse_str("2024-12-31").unwrap();
        assert_eq!(ymd.precision(), ReleaseDatePrecision::Day);
        assert_eq!(
            ymd.to_naive_date_opt(),
            NaiveDate::from_ymd_opt(2024, 12, 31)
        );
    }

    #[test]
    fn release_date_parse_various_formats() {
        // Slash delimiters
        let d = ReleaseDate::parse_str("2024/12/31").unwrap();
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, Some(12));
        assert_eq!(d.day, Some(31));

        // Year-month with slash
        let ym = ReleaseDate::parse_str("2024/06").unwrap();
        assert_eq!(ym.year, 2024);
        assert_eq!(ym.month, Some(6));
        assert_eq!(ym.day, None);

        // Compact formats
        let compact_full = ReleaseDate::parse_str("20241231").unwrap();
        assert_eq!(compact_full.year, 2024);
        assert_eq!(compact_full.month, Some(12));
        assert_eq!(compact_full.day, Some(31));

        let compact_ym = ReleaseDate::parse_str("202406").unwrap();
        assert_eq!(compact_ym.year, 2024);
        assert_eq!(compact_ym.month, Some(6));
        assert_eq!(compact_ym.day, None);

        let compact_y = ReleaseDate::parse_str("2024").unwrap();
        assert_eq!(compact_y.year, 2024);
        assert_eq!(compact_y.month, None);
        assert_eq!(compact_y.day, None);
    }

    #[test]
    fn release_date_parse_iso8601_with_timezone() {
        // RFC3339 format with timezone
        let dt = ReleaseDate::parse_str("2024-12-31T23:59:59Z").unwrap();
        assert_eq!(dt.year, 2024);
        assert_eq!(dt.month, Some(12));
        assert_eq!(dt.day, Some(31));

        // With offset
        let dt_offset = ReleaseDate::parse_str("2024-06-15T12:34:56+02:00").unwrap();
        assert_eq!(dt_offset.year, 2024);
        assert_eq!(dt_offset.month, Some(6));
        assert_eq!(dt_offset.day, Some(15));

        // Negative offset
        let dt_neg = ReleaseDate::parse_str("2024-01-01T00:00:00-05:00").unwrap();
        assert_eq!(dt_neg.year, 2024);
        assert_eq!(dt_neg.month, Some(1));
        assert_eq!(dt_neg.day, Some(1));

        // Year validation for RFC3339 formats (outside valid range should fail)
        assert!(ReleaseDate::parse_str("1850-01-01T00:00:00Z").is_none());
        assert!(ReleaseDate::parse_str("2150-12-31T23:59:59Z").is_none());
    }

    #[test]
    fn release_date_validation() {
        // Invalid month
        assert!(ReleaseDate::parse_str("2024-13").is_none());
        assert!(ReleaseDate::parse_str("2024-00").is_none());

        // Invalid day (February 30th)
        assert!(ReleaseDate::parse_str("2024-02-30").is_none());

        // Invalid day (April 31st)
        assert!(ReleaseDate::parse_str("2024-04-31").is_none());

        // Invalid day (day 32)
        assert!(ReleaseDate::parse_str("2024-01-32").is_none());

        // Invalid year (too old)
        assert!(ReleaseDate::parse_str("1899-01-01").is_none());

        // Invalid year (too far future)
        assert!(ReleaseDate::parse_str("2101-01-01").is_none());
    }

    #[test]
    fn release_date_leap_year_handling() {
        // Valid leap year (2024 is a leap year)
        let leap = ReleaseDate::parse_str("2024-02-29").unwrap();
        assert_eq!(leap.day, Some(29));

        // Invalid leap year date (2023 is not a leap year)
        assert!(ReleaseDate::parse_str("2023-02-29").is_none());

        // Valid non-leap year
        let non_leap = ReleaseDate::parse_str("2023-02-28").unwrap();
        assert_eq!(non_leap.day, Some(28));
    }

    #[test]
    fn release_date_to_iso8601() {
        let y = ReleaseDate::new(2024, None, None);
        assert_eq!(y.to_iso8601(), "2024");

        let ym = ReleaseDate::new(2024, Some(6), None);
        assert_eq!(ym.to_iso8601(), "2024-06");

        let ymd = ReleaseDate::new(2024, Some(12), Some(31));
        assert_eq!(ymd.to_iso8601(), "2024-12-31");
    }

    #[test]
    fn release_date_whitespace_trimming() {
        let d = ReleaseDate::parse_str("  2024-12-31  ").unwrap();
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, Some(12));
        assert_eq!(d.day, Some(31));

        let y = ReleaseDate::parse_str(" 2024 ").unwrap();
        assert_eq!(y.year, 2024);
    }

    #[test]
    fn release_date_invalid_formats() {
        // Empty string
        assert!(ReleaseDate::parse_str("").is_none());

        // Invalid separators
        assert!(ReleaseDate::parse_str("2024.12.31").is_none());

        // Too many parts
        assert!(ReleaseDate::parse_str("2024-12-31-01").is_none());

        // Non-numeric
        assert!(ReleaseDate::parse_str("abcd-ef-gh").is_none());

        // Partial numeric
        assert!(ReleaseDate::parse_str("2024-1a").is_none());
    }

    #[test]
    fn release_date_edge_cases() {
        // Century boundaries
        let y2k = ReleaseDate::parse_str("2000-01-01").unwrap();
        assert_eq!(y2k.year, 2000);

        // Minimum valid year
        let min = ReleaseDate::parse_str("1900-01-01").unwrap();
        assert_eq!(min.year, 1900);

        // Maximum valid year
        let max = ReleaseDate::parse_str("2100-12-31").unwrap();
        assert_eq!(max.year, 2100);
    }

    #[test]
    fn quality_profile_validation_cutoff_must_be_allowed() {
        let mut qp = QualityProfile::new("Default", vec!["FLAC".into(), "MP3 320".into()]);
        qp.cutoff_quality = Some("AAC".into());
        let errs = qp.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.field == "cutoff_quality"));
    }

    #[test]
    fn generate_track_path_sanitizes_and_formats() {
        let base = PathBuf::from("/music");
        let path = generate_track_path(
            &base,
            "Arti:st?",
            "Alb*um|",
            Some(1),
            "Intro/Opening",
            "flac",
        );
        let expected_end = Path::new("Arti st")
            .join("Alb um")
            .join("01 - Intro Opening.flac");
        assert!(path.ends_with(expected_end));
    }

    #[test]
    fn trackfile_constructor_defaults() {
        let tf = TrackFile::new(TrackId::new(), "C:/media/file.flac", 1234);
        assert_eq!(tf.size_bytes, 1234);
        assert!(tf.duration_ms.is_none());
        assert!(tf.hash.is_none());
    }

    #[test]
    fn artist_created_event() {
        let payload = ArtistCreatedPayload {
            artist_id: ArtistId::new(),
            name: "Test Artist".into(),
            monitored: true,
        };
        let event: ArtistCreated = DomainEvent::new("artist.created", payload);
        assert_eq!(event.name, "artist.created");
        assert_eq!(event.payload.name, "Test Artist");
    }

    #[test]
    fn album_created_event() {
        let artist_id = ArtistId::new();
        let payload = AlbumCreatedPayload {
            album_id: AlbumId::new(),
            artist_id,
            title: "Test Album".into(),
            monitored: true,
        };
        let event: AlbumCreated = DomainEvent::new("album.created", payload);
        assert_eq!(event.name, "album.created");
        assert_eq!(event.payload.title, "Test Album");
    }

    #[test]
    fn track_created_event() {
        let album_id = AlbumId::new();
        let artist_id = ArtistId::new();
        let payload = TrackCreatedPayload {
            track_id: TrackId::new(),
            album_id,
            artist_id,
            title: "Test Track".into(),
        };
        let event: TrackCreated = DomainEvent::new("track.created", payload);
        assert_eq!(event.name, "track.created");
        assert_eq!(event.payload.title, "Test Track");
    }

    #[test]
    fn artist_updated_event() {
        let payload = ArtistUpdatedPayload {
            artist_id: ArtistId::new(),
            name: "Updated Artist".into(),
            monitored: false,
        };
        let event: ArtistUpdated = DomainEvent::new("artist.updated", payload);
        assert_eq!(event.name, "artist.updated");
        assert_eq!(event.payload.name, "Updated Artist");
        assert!(!event.payload.monitored);
        let artist_id = ArtistId::new();
        let payload = ArtistUpdatedPayload {
            artist_id,
            name: "Updated Again".into(),
            monitored: true,
        };
        let event: ArtistUpdated = DomainEvent::new("artist.updated", payload);
        assert_eq!(event.name, "artist.updated");
        assert_eq!(event.payload.artist_id, artist_id);
    }

    #[test]
    fn album_updated_event() {
        let artist_id = ArtistId::new();
        let payload = AlbumUpdatedPayload {
            album_id: AlbumId::new(),
            artist_id,
            title: "Updated Album".into(),
            monitored: false,
        };
        let event: AlbumUpdated = DomainEvent::new("album.updated", payload);
        assert_eq!(event.name, "album.updated");
        assert_eq!(event.payload.title, "Updated Album");
        assert!(!event.payload.monitored);
        let album_id = AlbumId::new();
        let artist_id2 = ArtistId::new();
        let payload = AlbumUpdatedPayload {
            album_id,
            artist_id: artist_id2,
            title: "Updated Again".into(),
            monitored: true,
        };
        let event: AlbumUpdated = DomainEvent::new("album.updated", payload);
        assert_eq!(event.name, "album.updated");
        assert_eq!(event.payload.album_id, album_id);
    }

    #[test]
    fn track_updated_event() {
        let album_id = AlbumId::new();
        let artist_id = ArtistId::new();
        let payload = TrackUpdatedPayload {
            track_id: TrackId::new(),
            album_id,
            artist_id,
            title: "Updated Track".into(),
        };
        let event: TrackUpdated = DomainEvent::new("track.updated", payload);
        assert_eq!(event.name, "track.updated");
        assert_eq!(event.payload.title, "Updated Track");
        let track_id = TrackId::new();
        let album_id2 = AlbumId::new();
        let artist_id2 = ArtistId::new();
        let payload = TrackUpdatedPayload {
            track_id,
            album_id: album_id2,
            artist_id: artist_id2,
            title: "Updated Again".into(),
        };
        let event: TrackUpdated = DomainEvent::new("track.updated", payload);
        assert_eq!(event.name, "track.updated");
        assert_eq!(event.payload.track_id, track_id);
    }
}
