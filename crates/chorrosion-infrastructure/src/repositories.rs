// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use chorrosion_domain::{
    Album, AlbumId, AlbumStatus, Artist, ArtistId, ArtistRelationship, ArtistStatus,
    DownloadClientDefinition, EntityType, IndexerDefinition, MetadataProfile, QualityProfile,
    SmartPlaylist, Tag, TagId, TaggedEntity, Track, TrackFile, TrackId,
};
use chrono::NaiveDate;

// ============================================================================
// Repository Traits
// ============================================================================

/// Generic repository for CRUD operations on a domain entity
#[async_trait::async_trait]
pub trait Repository<T>: Send + Sync {
    async fn create(&self, entity: T) -> Result<T>;
    async fn get_by_id(&self, id: &str) -> Result<Option<T>>;
    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<T>>;
    async fn update(&self, entity: T) -> Result<T>;
    async fn delete(&self, id: &str) -> Result<()>;
}

/// Artist repository with specialized queries
#[async_trait::async_trait]
pub trait ArtistRepository: Repository<Artist> {
    async fn get_by_name(&self, name: &str) -> Result<Option<Artist>>;
    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Artist>>;
    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Artist>>;
    async fn get_by_status(
        &self,
        status: ArtistStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Artist>>;
}

/// Album repository with specialized queries
#[async_trait::async_trait]
pub trait AlbumRepository: Repository<Album> {
    async fn get_by_artist(
        &self,
        artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>>;
    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Album>>;
    /// Look up an album by artist and title (case-insensitive). Used for de-duplicate checks
    /// during auto-add to avoid loading thousands of albums into memory.
    async fn get_by_artist_and_title(
        &self,
        artist_id: ArtistId,
        title: &str,
    ) -> Result<Option<Album>>;
    async fn get_by_status(
        &self,
        status: AlbumStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>>;
    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Album>>;
    async fn get_by_album_type(
        &self,
        album_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>>;
    /// Return wanted albums that have no associated track records.
    async fn list_wanted_without_tracks(&self, limit: i64, offset: i64) -> Result<Vec<Album>>;
    /// Return monitored albums that have track files but whose quality does not meet
    /// the cutoff defined in the artist's quality profile.
    ///
    /// Quality ordering: earlier index in `allowed_qualities` = higher quality.
    /// A file is below cutoff when its codec's index exceeds the cutoff's index,
    /// or when the codec is unknown / absent from the allowed list.
    async fn list_cutoff_unmet_albums(&self, limit: i64, offset: i64) -> Result<Vec<Album>>;
    /// Return monitored albums whose ``release_date`` falls within [start, end] inclusive,
    /// ordered by ``release_date`` ascending.
    async fn list_upcoming_releases(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>>;
}

/// Track repository with specialized queries
#[async_trait::async_trait]
pub trait TrackRepository: Repository<Track> {
    async fn get_by_album(&self, album_id: AlbumId, limit: i64, offset: i64) -> Result<Vec<Track>>;
    async fn get_by_artist(
        &self,
        artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Track>>;
    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Track>>;
    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Track>>;
    async fn list_without_files(&self, limit: i64, offset: i64) -> Result<Vec<Track>>;
}

/// Quality profile repository
#[async_trait::async_trait]
pub trait QualityProfileRepository: Repository<QualityProfile> {
    async fn get_by_name(&self, name: &str) -> Result<Option<QualityProfile>>;
}

/// Metadata profile repository
#[async_trait::async_trait]
pub trait MetadataProfileRepository: Repository<MetadataProfile> {
    async fn get_by_name(&self, name: &str) -> Result<Option<MetadataProfile>>;
}

/// Indexer definition repository
#[async_trait::async_trait]
pub trait IndexerDefinitionRepository: Repository<IndexerDefinition> {
    async fn get_by_name(&self, name: &str) -> Result<Option<IndexerDefinition>>;
}

/// Download client definition repository
#[async_trait::async_trait]
pub trait DownloadClientDefinitionRepository: Repository<DownloadClientDefinition> {
    async fn get_by_name(&self, name: &str) -> Result<Option<DownloadClientDefinition>>;
}

/// Track file repository for managing audio files
#[async_trait::async_trait]
pub trait TrackFileRepository: Repository<TrackFile> {
    /// Get all track files for a specific track
    async fn get_by_track(
        &self,
        track_id: TrackId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TrackFile>>;

    /// Get a track file by its file path
    async fn get_by_path(&self, path: &str) -> Result<Option<TrackFile>>;

    /// List track files with fingerprints
    async fn list_with_fingerprints(&self, limit: i64, offset: i64) -> Result<Vec<TrackFile>>;

    /// List track files without fingerprints (need processing)
    async fn list_without_fingerprints(&self, limit: i64, offset: i64) -> Result<Vec<TrackFile>>;
}

/// Artist relationship repository with specialized queries for artist connections
#[async_trait::async_trait]
pub trait ArtistRelationshipRepository: Repository<ArtistRelationship> {
    /// Get all relationships where source_artist_id is the given artist
    async fn get_by_source_artist(
        &self,
        source_artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArtistRelationship>>;

    /// Get all relationships where related_artist_id is the given artist
    async fn get_by_related_artist(
        &self,
        related_artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArtistRelationship>>;

    /// Get relationships of a specific type for a source artist
    async fn get_by_type_and_source(
        &self,
        source_artist_id: ArtistId,
        relationship_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArtistRelationship>>;

    /// Check if a relationship exists between two artists
    async fn relationship_exists(
        &self,
        source_artist_id: ArtistId,
        related_artist_id: ArtistId,
        relationship_type: &str,
    ) -> Result<bool>;
}

/// Tag repository for managing user-defined tags
#[async_trait::async_trait]
pub trait TagRepository: Repository<Tag> {
    /// Get tag by name (case-insensitive lookup)
    async fn get_by_name(&self, name: &str) -> Result<Option<Tag>>;

    /// List tags by an entity type, filtered by entity_id
    async fn get_tags_for_entity(
        &self,
        entity_id: &str,
        entity_type: EntityType,
    ) -> Result<Vec<Tag>>;

    /// List all entities (artists or albums) with a specific tag
    async fn get_entities_with_tag(
        &self,
        tag_id: TagId,
        entity_type: EntityType,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<String>>;
}

/// Tagged entity repository for managing tag-entity associations
#[async_trait::async_trait]
pub trait TaggedEntityRepository: Repository<TaggedEntity> {
    /// Assign a tag to an entity
    async fn assign_tag(
        &self,
        tag_id: TagId,
        entity_id: &str,
        entity_type: EntityType,
    ) -> Result<()>;

    /// Remove a tag from an entity
    async fn remove_tag(
        &self,
        tag_id: TagId,
        entity_id: &str,
        entity_type: EntityType,
    ) -> Result<()>;

    /// Get all tags for an entity
    async fn get_tags_for_entity(
        &self,
        entity_id: &str,
        entity_type: EntityType,
    ) -> Result<Vec<TagId>>;

    /// Check if an entity has a specific tag
    async fn has_tag(
        &self,
        tag_id: TagId,
        entity_id: &str,
        entity_type: EntityType,
    ) -> Result<bool>;

    /// Remove all tags from an entity
    async fn clear_entity_tags(&self, entity_id: &str, entity_type: EntityType) -> Result<()>;
}

/// Smart playlist repository for dynamic playlist definitions.
#[async_trait::async_trait]
pub trait SmartPlaylistRepository: Repository<SmartPlaylist> {
    /// Get a smart playlist by case-insensitive name.
    async fn get_by_name(&self, name: &str) -> Result<Option<SmartPlaylist>>;
}
