// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{anyhow, Result};
use chorrosion_domain::{
    Album, AlbumId, AlbumStatus, Artist, ArtistId, ArtistStatus, MetadataProfile, QualityProfile,
    Track,
};
use sqlx::SqlitePool;
use sqlx::Row;
use tracing::debug;
use uuid::Uuid;
use chrono::{DateTime, NaiveDateTime, Utc};

use crate::repositories::{
    AlbumRepository, ArtistRepository, MetadataProfileRepository, QualityProfileRepository,
    Repository, TrackRepository,
};

/// SQLx-backed Artist repository
#[allow(dead_code)]
pub struct SqliteArtistRepository {
    pool: SqlitePool,
}

impl SqliteArtistRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl Repository<Artist> for SqliteArtistRepository {
    async fn create(&self, entity: Artist) -> Result<Artist> {
        debug!(target: "repository", artist_id = %entity.id, "creating artist");
        // Insert artist row
        let q = r#"
            INSERT INTO artists (
                id, name, foreign_artist_id, metadata_profile_id, quality_profile_id,
                status, path, monitored, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let id_str = entity.id.to_string();
        let foreign_id = entity.foreign_artist_id.clone();
        let metadata_id = entity.metadata_profile_id.map(|p| p.to_string());
        let quality_id = entity.quality_profile_id.map(|p| p.to_string());
        let status = entity.status.to_string();
        let path = entity.path.clone();
        let monitored = entity.monitored;
        let created_at = entity.created_at.to_rfc3339();
        let updated_at = entity.updated_at.to_rfc3339();

        sqlx::query(q)
            .bind(id_str)
            .bind(entity.name.clone())
            .bind(foreign_id)
            .bind(metadata_id)
            .bind(quality_id)
            .bind(status)
            .bind(path)
            .bind(monitored)
            .bind(created_at)
            .bind(updated_at)
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<Artist>> {
        let id = id.into();
        debug!(target: "repository", %id, "fetching artist by id");
        let row = sqlx::query("SELECT * FROM artists WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_artist(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Artist>> {
        debug!(target: "repository", limit, offset, "listing artists");
        let rows = sqlx::query("SELECT * FROM artists ORDER BY name LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_artist(&r)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: Artist) -> Result<Artist> {
        debug!(target: "repository", artist_id = %entity.id, "updating artist");
        let q = r#"
            UPDATE artists SET
                name = ?,
                foreign_artist_id = ?,
                metadata_profile_id = ?,
                quality_profile_id = ?,
                status = ?,
                path = ?,
                monitored = ?,
                updated_at = ?
            WHERE id = ?
        "#;
        sqlx::query(q)
            .bind(entity.name.clone())
            .bind(entity.foreign_artist_id.clone())
            .bind(entity.metadata_profile_id.map(|p| p.to_string()))
            .bind(entity.quality_profile_id.map(|p| p.to_string()))
            .bind(entity.status.to_string())
            .bind(entity.path.clone())
            .bind(entity.monitored)
            .bind(entity.updated_at.to_rfc3339())
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let id = id.into();
        debug!(target: "repository", %id, "deleting artist");
        sqlx::query("DELETE FROM artists WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ArtistRepository for SqliteArtistRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<Artist>> {
        debug!(target: "repository", name, "fetching artist by name");
        let row = sqlx::query("SELECT * FROM artists WHERE name = ? LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| row_to_artist(&r)).transpose()?)
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Artist>> {
        debug!(target: "repository", foreign_id, "fetching artist by foreign_id");
        let row = sqlx::query("SELECT * FROM artists WHERE foreign_artist_id = ? LIMIT 1")
            .bind(foreign_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| row_to_artist(&r)).transpose()?)
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Artist>> {
        debug!(target: "repository", limit, offset, "listing monitored artists");
        let rows = sqlx::query("SELECT * FROM artists WHERE monitored = 1 ORDER BY name LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows { out.push(row_to_artist(&r)?); }
        Ok(out)
    }

    async fn get_by_status(
        &self,
        status: ArtistStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Artist>> {
        debug!(target: "repository", ?status, limit, offset, "fetching artists by status");
        let rows = sqlx::query("SELECT * FROM artists WHERE status = ? ORDER BY name LIMIT ? OFFSET ?")
            .bind(status.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows { out.push(row_to_artist(&r)?); }
        Ok(out)
    }
}

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

fn parse_uuid_opt(s: Option<String>) -> Result<Option<chorrosion_domain::ProfileId>> {
    match s {
        Some(val) => {
            let uuid = Uuid::parse_str(&val)?;
            Ok(Some(chorrosion_domain::ProfileId::from_uuid(uuid)))
        }
        None => Ok(None),
    }
}

fn parse_artist_status(s: &str) -> Result<ArtistStatus> {
    match s {
        "continuing" => Ok(ArtistStatus::Continuing),
        "ended" => Ok(ArtistStatus::Ended),
        other => Err(anyhow!("unknown artist status: {}", other)),
    }
}

fn parse_dt(s: String) -> Result<DateTime<Utc>> {
    // Try RFC3339 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Fallback to SQLite default CURRENT_TIMESTAMP format: "YYYY-MM-DD HH:MM:SS"
    let ndt = NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc))
}

fn row_to_artist(row: &sqlx::sqlite::SqliteRow) -> Result<Artist> {
    let id_str: String = row.try_get("id")?;
    let id = ArtistId::from_uuid(Uuid::parse_str(&id_str)?);

    let name: String = row.try_get("name")?;
    let foreign_artist_id: Option<String> = row.try_get("foreign_artist_id")?;
    let metadata_profile_id: Option<String> = row.try_get("metadata_profile_id")?;
    let quality_profile_id: Option<String> = row.try_get("quality_profile_id")?;
    let status_str: String = row.try_get("status")?;
    let path: Option<String> = row.try_get("path")?;
    let monitored: bool = row.try_get("monitored")?;
    let created_at_s: String = row.try_get("created_at")?;
    let updated_at_s: String = row.try_get("updated_at")?;

    Ok(Artist {
        id,
        name,
        foreign_artist_id,
        metadata_profile_id: parse_uuid_opt(metadata_profile_id)?,
        quality_profile_id: parse_uuid_opt(quality_profile_id)?,
        status: parse_artist_status(&status_str)?,
        path,
        monitored,
        created_at: parse_dt(created_at_s)?,
        updated_at: parse_dt(updated_at_s)?,
    })
}

// ============================================================================

/// SQLx-backed Album repository
#[allow(dead_code)]
pub struct SqliteAlbumRepository {
    pool: SqlitePool,
}

impl SqliteAlbumRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl Repository<Album> for SqliteAlbumRepository {
    async fn create(&self, entity: Album) -> Result<Album> {
        debug!(target: "repository", album_id = %entity.id, "creating album");
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<Album>> {
        let _id = id.into();
        debug!(target: "repository", %_id, "fetching album by id");
        Ok(None)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing albums");
        Ok(vec![])
    }

    async fn update(&self, entity: Album) -> Result<Album> {
        debug!(target: "repository", album_id = %entity.id, "updating album");
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let _id = id.into();
        debug!(target: "repository", %_id, "deleting album");
        Ok(())
    }
}

#[async_trait::async_trait]
impl AlbumRepository for SqliteAlbumRepository {
    async fn get_by_artist(
        &self,
        artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>> {
        debug!(target: "repository", %artist_id, limit, offset, "fetching albums by artist");
        Ok(vec![])
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Album>> {
        debug!(target: "repository", foreign_id, "fetching album by foreign_id");
        Ok(None)
    }

    async fn get_by_status(
        &self,
        status: AlbumStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>> {
        debug!(target: "repository", ?status, limit, offset, "fetching albums by status");
        Ok(vec![])
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing monitored albums");
        Ok(vec![])
    }
}

// ============================================================================

/// SQLx-backed Track repository
#[allow(dead_code)]
pub struct SqliteTrackRepository {
    pool: SqlitePool,
}

impl SqliteTrackRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl Repository<Track> for SqliteTrackRepository {
    async fn create(&self, entity: Track) -> Result<Track> {
        debug!(target: "repository", track_id = %entity.id, "creating track");
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<Track>> {
        let _id = id.into();
        debug!(target: "repository", %_id, "fetching track by id");
        Ok(None)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing tracks");
        Ok(vec![])
    }

    async fn update(&self, entity: Track) -> Result<Track> {
        debug!(target: "repository", track_id = %entity.id, "updating track");
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let _id = id.into();
        debug!(target: "repository", %_id, "deleting track");
        Ok(())
    }
}

#[async_trait::async_trait]
impl TrackRepository for SqliteTrackRepository {
    async fn get_by_album(&self, album_id: AlbumId, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", %album_id, limit, offset, "fetching tracks by album");
        Ok(vec![])
    }

    async fn get_by_artist(
        &self,
        artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Track>> {
        debug!(target: "repository", %artist_id, limit, offset, "fetching tracks by artist");
        Ok(vec![])
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Track>> {
        debug!(target: "repository", foreign_id, "fetching track by foreign_id");
        Ok(None)
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing monitored tracks");
        Ok(vec![])
    }

    async fn list_without_files(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing tracks without files");
        Ok(vec![])
    }
}

// ============================================================================

/// SQLx-backed Quality Profile repository
#[allow(dead_code)]
pub struct SqliteQualityProfileRepository {
    pool: SqlitePool,
}

impl SqliteQualityProfileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl Repository<QualityProfile> for SqliteQualityProfileRepository {
    async fn create(&self, entity: QualityProfile) -> Result<QualityProfile> {
        debug!(target: "repository", profile_id = %entity.id, "creating quality profile");
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<QualityProfile>> {
        let _id = id.into();
        debug!(target: "repository", %_id, "fetching quality profile by id");
        Ok(None)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<QualityProfile>> {
        debug!(target: "repository", limit, offset, "listing quality profiles");
        Ok(vec![])
    }

    async fn update(&self, entity: QualityProfile) -> Result<QualityProfile> {
        debug!(target: "repository", profile_id = %entity.id, "updating quality profile");
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let _id = id.into();
        debug!(target: "repository", %_id, "deleting quality profile");
        Ok(())
    }
}

#[async_trait::async_trait]
impl QualityProfileRepository for SqliteQualityProfileRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<QualityProfile>> {
        debug!(target: "repository", name, "fetching quality profile by name");
        Ok(None)
    }
}

// ============================================================================

/// SQLx-backed Metadata Profile repository
#[allow(dead_code)]
pub struct SqliteMetadataProfileRepository {
    pool: SqlitePool,
}

impl SqliteMetadataProfileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl Repository<MetadataProfile> for SqliteMetadataProfileRepository {
    async fn create(&self, entity: MetadataProfile) -> Result<MetadataProfile> {
        debug!(target: "repository", profile_id = %entity.id, "creating metadata profile");
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<MetadataProfile>> {
        let _id = id.into();
        debug!(target: "repository", %_id, "fetching metadata profile by id");
        Ok(None)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<MetadataProfile>> {
        debug!(target: "repository", limit, offset, "listing metadata profiles");
        Ok(vec![])
    }

    async fn update(&self, entity: MetadataProfile) -> Result<MetadataProfile> {
        debug!(target: "repository", profile_id = %entity.id, "updating metadata profile");
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let _id = id.into();
        debug!(target: "repository", %_id, "deleting metadata profile");
        Ok(())
    }
}

#[async_trait::async_trait]
impl MetadataProfileRepository for SqliteMetadataProfileRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<MetadataProfile>> {
        debug!(target: "repository", name, "fetching metadata profile by name");
        Ok(None)
    }
}

// ============================================================================
// Tests (basic CRUD happy path for Artist)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");

        sqlx::migrate!("../../migrations").run(&pool).await.expect("migrate");
        pool
    }

    #[tokio::test]
    async fn artist_create_and_get_by_id_round_trip() {
        let pool = setup_pool().await;
        let repo = SqliteArtistRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Test Artist");
        let id = artist.id;

        let created = repo.create(artist).await.expect("create artist");
        assert_eq!(created.id, id);

        let fetched = repo
            .get_by_id(id.to_string())
            .await
            .expect("fetch artist")
            .expect("artist exists");
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.name, "Test Artist");
        assert!(fetched.monitored);
    }
}
