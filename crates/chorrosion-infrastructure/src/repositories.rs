// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::Result;
use chorrosion_domain::{
    Album, AlbumId, AlbumStatus, Artist, ArtistId, ArtistStatus, MetadataProfile, QualityProfile,
    Track,
};

// ============================================================================
// Repository Traits
// ============================================================================

/// Generic repository for CRUD operations on a domain entity
#[async_trait::async_trait]
pub trait Repository<T>: Send + Sync {
    async fn create(&self, entity: T) -> Result<T>;
    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<T>>;
    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<T>>;
    async fn update(&self, entity: T) -> Result<T>;
    async fn delete(&self, id: impl Into<String> + Send) -> Result<()>;
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
    async fn get_by_status(
        &self,
        status: AlbumStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>>;
    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Album>>;
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
