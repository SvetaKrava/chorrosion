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
            .bind(id_str)                 // 1: id
            .bind(entity.name.clone())    // 2: name
            .bind(foreign_id)             // 3: foreign_artist_id
            .bind(metadata_id)            // 4: metadata_profile_id
            .bind(quality_id)             // 5: quality_profile_id
            .bind(status)                 // 6: status
            .bind(path)                   // 7: path
            .bind(monitored)              // 8: monitored
            .bind(created_at)             // 9: created_at
            .bind(updated_at)             // 10: updated_at
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
        let result = sqlx::query("DELETE FROM artists WHERE id = ?")
            .bind(&id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(anyhow!("artist not found: {}", id));
        }
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

fn parse_album_status(s: &str) -> Result<AlbumStatus> {
    match s {
        "wanted" => Ok(AlbumStatus::Wanted),
        "released" => Ok(AlbumStatus::Released),
        "announced" => Ok(AlbumStatus::Announced),
        other => Err(anyhow!("unknown album status: {}", other)),
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

fn row_to_album(row: &sqlx::sqlite::SqliteRow) -> Result<Album> {
    let id_str: String = row.try_get("id")?;
    let id = AlbumId::from_uuid(Uuid::parse_str(&id_str)?);

    let artist_id_str: String = row.try_get("artist_id")?;
    let artist_id = ArtistId::from_uuid(Uuid::parse_str(&artist_id_str)?);

    let foreign_album_id: Option<String> = row.try_get("foreign_album_id")?;
    let title: String = row.try_get("title")?;
    let release_date: Option<String> = row.try_get("release_date")?;
    let album_type: Option<String> = row.try_get("album_type")?;
    let status_str: String = row.try_get("status")?;
    let monitored: bool = row.try_get("monitored")?;
    let created_at_s: String = row.try_get("created_at")?;
    let updated_at_s: String = row.try_get("updated_at")?;

    Ok(Album {
        id,
        artist_id,
        foreign_album_id,
        title,
        release_date: release_date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        album_type,
        status: parse_album_status(&status_str)?,
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
        let q = r#"
            INSERT INTO albums (
                id, artist_id, foreign_album_id, title, release_date,
                album_type, status, monitored, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let id_str = entity.id.to_string();
        let artist_id_str = entity.artist_id.to_string();
        let foreign_id = entity.foreign_album_id.clone();
        let title = entity.title.clone();
        let release_date = entity.release_date.map(|d| d.format("%Y-%m-%d").to_string());
        let album_type = entity.album_type.clone();
        let status = entity.status.to_string();
        let monitored = entity.monitored;
        let created_at = entity.created_at.to_rfc3339();
        let updated_at = entity.updated_at.to_rfc3339();

        sqlx::query(q)
            .bind(id_str)
            .bind(artist_id_str)
            .bind(foreign_id)
            .bind(title)
            .bind(release_date)
            .bind(album_type)
            .bind(status)
            .bind(monitored)
            .bind(created_at)
            .bind(updated_at)
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<Album>> {
        let id = id.into();
        debug!(target: "repository", %id, "fetching album by id");
        let row = sqlx::query("SELECT * FROM albums WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_album(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing albums");
        let rows = sqlx::query("SELECT * FROM albums ORDER BY title LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_album(&r)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: Album) -> Result<Album> {
        debug!(target: "repository", album_id = %entity.id, "updating album");
        let q = r#"
            UPDATE albums SET
                artist_id = ?,
                foreign_album_id = ?,
                title = ?,
                release_date = ?,
                album_type = ?,
                status = ?,
                monitored = ?,
                updated_at = ?
            WHERE id = ?
        "#;
        sqlx::query(q)
            .bind(entity.artist_id.to_string())
            .bind(entity.foreign_album_id.clone())
            .bind(entity.title.clone())
            .bind(entity.release_date.map(|d| d.format("%Y-%m-%d").to_string()))
            .bind(entity.album_type.clone())
            .bind(entity.status.to_string())
            .bind(entity.monitored)
            .bind(entity.updated_at.to_rfc3339())
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let id = id.into();
        debug!(target: "repository", %id, "deleting album");
        let result = sqlx::query("DELETE FROM albums WHERE id = ?")
            .bind(&id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(anyhow!("album not found: {}", id));
        }
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
        let rows = sqlx::query("SELECT * FROM albums WHERE artist_id = ? ORDER BY title LIMIT ? OFFSET ?")
            .bind(artist_id.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_album(&r)?);
        }
        Ok(out)
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Album>> {
        debug!(target: "repository", foreign_id, "fetching album by foreign_id");
        let row = sqlx::query("SELECT * FROM albums WHERE foreign_album_id = ? LIMIT 1")
            .bind(foreign_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| row_to_album(&r)).transpose()?)
    }

    async fn get_by_status(
        &self,
        status: AlbumStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>> {
        debug!(target: "repository", ?status, limit, offset, "fetching albums by status");
        let rows = sqlx::query("SELECT * FROM albums WHERE status = ? ORDER BY title LIMIT ? OFFSET ?")
            .bind(status.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_album(&r)?);
        }
        Ok(out)
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing monitored albums");
        let rows = sqlx::query("SELECT * FROM albums WHERE monitored = 1 ORDER BY title LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_album(&r)?);
        }
        Ok(out)
    }

    async fn get_by_album_type(
        &self,
        album_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>> {
        debug!(target: "repository", album_type, limit, offset, "fetching albums by type");
        let rows = sqlx::query("SELECT * FROM albums WHERE album_type = ? ORDER BY title LIMIT ? OFFSET ?")
            .bind(album_type)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_album(&r)?);
        }
        Ok(out)
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

    #[tokio::test]
    async fn artist_get_by_name_and_foreign_id() {
        let pool = setup_pool().await;
        let repo = SqliteArtistRepository::new(pool.clone());

        let mut artist = chorrosion_domain::Artist::new("Alpha");
        artist.foreign_artist_id = Some("mbid:alpha".to_string());
        repo.create(artist.clone()).await.expect("create alpha");

        let by_name = repo.get_by_name("Alpha").await.expect("by name").expect("exists");
        assert_eq!(by_name.name, "Alpha");

        let by_foreign = repo
            .get_by_foreign_id("mbid:alpha")
            .await
            .expect("by foreign")
            .expect("exists");
        assert_eq!(by_foreign.id, by_name.id);
    }

    #[tokio::test]
    async fn artist_list_monitored_and_status_filters() {
        let pool = setup_pool().await;
        let repo = SqliteArtistRepository::new(pool.clone());

        // A: monitored=true, continuing
        let a = chorrosion_domain::Artist::new("A");
        repo.create(a.clone()).await.expect("create A");

        // B: monitored=false, continuing
        let mut b = chorrosion_domain::Artist::new("B");
        b.monitored = false;
        repo.create(b.clone()).await.expect("create B");

        // C: monitored=true, ended
        let mut c = chorrosion_domain::Artist::new("C");
        c.status = chorrosion_domain::ArtistStatus::Ended;
        repo.create(c.clone()).await.expect("create C");

        let monitored = repo.list_monitored(10, 0).await.expect("monitored");
        assert!(monitored.iter().all(|x| x.monitored));
        assert!(monitored.iter().any(|x| x.name == "A"));
        assert!(monitored.iter().any(|x| x.name == "C"));
        assert!(monitored.iter().all(|x| x.name != "B"));

        let continuing = repo
            .get_by_status(chorrosion_domain::ArtistStatus::Continuing, 10, 0)
            .await
            .expect("continuing");
        assert!(continuing.iter().any(|x| x.name == "A"));
        assert!(continuing.iter().any(|x| x.name == "B"));
        assert!(continuing.iter().all(|x| x.name != "C"));

        let ended = repo
            .get_by_status(chorrosion_domain::ArtistStatus::Ended, 10, 0)
            .await
            .expect("ended");
        assert!(ended.iter().any(|x| x.name == "C"));
        assert!(ended.iter().all(|x| x.name != "A" && x.name != "B"));
    }

    #[tokio::test]
    async fn artist_update_and_delete_flow() {
        let pool = setup_pool().await;
        let repo = SqliteArtistRepository::new(pool.clone());

        let mut artist = chorrosion_domain::Artist::new("Before");
        let id = artist.id;
        let created = repo.create(artist.clone()).await.expect("create");
        assert_eq!(created.name, "Before");

        // Update fields
        artist.name = "After".to_string();
        artist.path = Some("/music/after".to_string());
        artist.monitored = false;
        let updated = repo.update(artist.clone()).await.expect("update");
        assert_eq!(updated.name, "After");
        assert!(!updated.monitored);

        let fetched = repo.get_by_id(id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.name, "After");
        assert_eq!(fetched.path.as_deref(), Some("/music/after"));

        // Delete and ensure gone
        repo.delete(id.to_string()).await.expect("delete");
        let absent = repo.get_by_id(id.to_string()).await.expect("get");
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn artist_list_ordering_and_pagination() {
        let pool = setup_pool().await;
        let repo = SqliteArtistRepository::new(pool.clone());

        // Intentionally insert in shuffled order
        for name in ["Charlie", "Alpha", "Bravo", "Echo", "Delta"] {
            let mut a = chorrosion_domain::Artist::new(name);
            // Slightly tweak updated_at to avoid any tie-break edge cases
            a.updated_at += chrono::Duration::milliseconds(1);
            repo.create(a).await.expect("create");
        }

        // Verify global ordering is by name ASC
        let all = repo.list(10, 0).await.expect("list all");
        let names: Vec<_> = all.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "Bravo", "Charlie", "Delta", "Echo"]);

        // Pagination windows
        let page1 = repo.list(2, 0).await.expect("page1");
        let n1: Vec<_> = page1.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(n1, vec!["Alpha", "Bravo"]);

        let page2 = repo.list(2, 2).await.expect("page2");
        let n2: Vec<_> = page2.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(n2, vec!["Charlie", "Delta"]);

        let page3 = repo.list(2, 4).await.expect("page3");
        let n3: Vec<_> = page3.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(n3, vec!["Echo"]);

        let empty = repo.list(2, 6).await.expect("empty");
        assert!(empty.is_empty());
    }

    // ======================================================================
    // Album Repository Tests
    // ======================================================================

    #[tokio::test]
    async fn album_create_and_get_by_id_round_trip() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());

        // Create artist first (FK constraint)
        let artist = chorrosion_domain::Artist::new("Test Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        // Create album
        let mut album = chorrosion_domain::Album::new(artist_id, "Test Album");
        album.album_type = Some("studio".to_string());
        album.status = AlbumStatus::Released;
        let album_id = album.id;

        let created = album_repo.create(album).await.expect("create album");
        assert_eq!(created.id, album_id);
        assert_eq!(created.artist_id, artist_id);

        // Fetch and verify
        let fetched = album_repo
            .get_by_id(album_id.to_string())
            .await
            .expect("fetch album")
            .expect("album exists");
        assert_eq!(fetched.id, album_id);
        assert_eq!(fetched.title, "Test Album");
        assert_eq!(fetched.artist_id, artist_id);
        assert_eq!(fetched.album_type.as_deref(), Some("studio"));
        assert!(fetched.monitored);
    }

    #[tokio::test]
    async fn album_get_by_artist_and_foreign_id() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());

        // Create two artists
        let artist_a = chorrosion_domain::Artist::new("Artist A");
        let id_a = artist_a.id;
        artist_repo.create(artist_a).await.expect("create A");

        let artist_b = chorrosion_domain::Artist::new("Artist B");
        let id_b = artist_b.id;
        artist_repo.create(artist_b).await.expect("create B");

        // Create albums for artist A
        let mut album1 = chorrosion_domain::Album::new(id_a, "Album 1");
        album1.foreign_album_id = Some("mbid:album1".to_string());
        album_repo.create(album1.clone()).await.expect("create 1");

        let album2 = chorrosion_domain::Album::new(id_a, "Album 2");
        album_repo.create(album2.clone()).await.expect("create 2");

        // Create album for artist B
        let album3 = chorrosion_domain::Album::new(id_b, "Album 3");
        album_repo.create(album3.clone()).await.expect("create 3");

        // Test get_by_artist
        let albums_a = album_repo.get_by_artist(id_a, 10, 0).await.expect("get by artist A");
        assert_eq!(albums_a.len(), 2);
        assert!(albums_a.iter().all(|a| a.artist_id == id_a));

        let albums_b = album_repo.get_by_artist(id_b, 10, 0).await.expect("get by artist B");
        assert_eq!(albums_b.len(), 1);
        assert_eq!(albums_b[0].title, "Album 3");

        // Test get_by_foreign_id
        let by_foreign = album_repo
            .get_by_foreign_id("mbid:album1")
            .await
            .expect("by foreign")
            .expect("exists");
        assert_eq!(by_foreign.id, album1.id);
        assert_eq!(by_foreign.title, "Album 1");
    }

    #[tokio::test]
    async fn album_list_monitored_and_status_filters() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Test Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        // A: monitored=true, wanted
        let a = chorrosion_domain::Album::new(artist_id, "A");
        album_repo.create(a.clone()).await.expect("create A");

        // B: monitored=false, released
        let mut b = chorrosion_domain::Album::new(artist_id, "B");
        b.monitored = false;
        b.status = AlbumStatus::Released;
        album_repo.create(b.clone()).await.expect("create B");

        // C: monitored=true, announced
        let mut c = chorrosion_domain::Album::new(artist_id, "C");
        c.status = AlbumStatus::Announced;
        album_repo.create(c.clone()).await.expect("create C");

        // D: monitored=true, released
        let mut d = chorrosion_domain::Album::new(artist_id, "D");
        d.status = AlbumStatus::Released;
        album_repo.create(d.clone()).await.expect("create D");

        // Test monitored
        let monitored = album_repo.list_monitored(10, 0).await.expect("monitored");
        assert_eq!(monitored.len(), 3);
        assert!(monitored.iter().all(|x| x.monitored));
        assert!(monitored.iter().any(|x| x.title == "A"));
        assert!(monitored.iter().any(|x| x.title == "C"));
        assert!(monitored.iter().any(|x| x.title == "D"));
        assert!(monitored.iter().all(|x| x.title != "B"));

        // Test by status
        let wanted = album_repo
            .get_by_status(AlbumStatus::Wanted, 10, 0)
            .await
            .expect("wanted");
        assert_eq!(wanted.len(), 1);
        assert_eq!(wanted[0].title, "A");

        let released = album_repo
            .get_by_status(AlbumStatus::Released, 10, 0)
            .await
            .expect("released");
        assert_eq!(released.len(), 2);
        assert!(released.iter().any(|x| x.title == "B"));
        assert!(released.iter().any(|x| x.title == "D"));

        let announced = album_repo
            .get_by_status(AlbumStatus::Announced, 10, 0)
            .await
            .expect("announced");
        assert_eq!(announced.len(), 1);
        assert_eq!(announced[0].title, "C");
    }

    #[tokio::test]
    async fn album_get_by_album_type() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Test Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        // Create albums with different types
        let mut studio1 = chorrosion_domain::Album::new(artist_id, "Studio 1");
        studio1.album_type = Some("studio".to_string());
        album_repo.create(studio1).await.expect("create studio1");

        let mut studio2 = chorrosion_domain::Album::new(artist_id, "Studio 2");
        studio2.album_type = Some("studio".to_string());
        album_repo.create(studio2).await.expect("create studio2");

        let mut live = chorrosion_domain::Album::new(artist_id, "Live");
        live.album_type = Some("live".to_string());
        album_repo.create(live).await.expect("create live");

        let mut compilation = chorrosion_domain::Album::new(artist_id, "Compilation");
        compilation.album_type = Some("compilation".to_string());
        album_repo.create(compilation).await.expect("create compilation");

        // Test get_by_album_type
        let studio = album_repo.get_by_album_type("studio", 10, 0).await.expect("studio");
        assert_eq!(studio.len(), 2);
        assert!(studio.iter().all(|a| a.album_type.as_deref() == Some("studio")));

        let live_albums = album_repo.get_by_album_type("live", 10, 0).await.expect("live");
        assert_eq!(live_albums.len(), 1);
        assert_eq!(live_albums[0].title, "Live");

        let comps = album_repo.get_by_album_type("compilation", 10, 0).await.expect("compilation");
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].title, "Compilation");
    }

    #[tokio::test]
    async fn album_update_and_delete_flow() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        let mut album = chorrosion_domain::Album::new(artist_id, "Before");
        let album_id = album.id;
        let created = album_repo.create(album.clone()).await.expect("create");
        assert_eq!(created.title, "Before");

        // Update fields
        album.title = "After".to_string();
        album.album_type = Some("live".to_string());
        album.monitored = false;
        album.status = AlbumStatus::Released;
        album.release_date = Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        let updated = album_repo.update(album.clone()).await.expect("update");
        assert_eq!(updated.title, "After");
        assert!(!updated.monitored);

        let fetched = album_repo.get_by_id(album_id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.title, "After");
        assert_eq!(fetched.album_type.as_deref(), Some("live"));
        assert_eq!(fetched.release_date, Some(chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));

        // Delete and ensure gone
        album_repo.delete(album_id.to_string()).await.expect("delete");
        let absent = album_repo.get_by_id(album_id.to_string()).await.expect("get");
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn album_list_ordering_and_pagination() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        // Create albums in shuffled order
        for title in ["Zebra", "Alpha", "Bravo", "Echo", "Delta"] {
            let album = chorrosion_domain::Album::new(artist_id, title);
            album_repo.create(album).await.expect("create");
        }

        // Verify global ordering is by title ASC
        let all = album_repo.list(10, 0).await.expect("list all");
        let titles: Vec<_> = all.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "Delta", "Echo", "Zebra"]);

        // Pagination windows
        let page1 = album_repo.list(2, 0).await.expect("page1");
        let t1: Vec<_> = page1.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(t1, vec!["Alpha", "Bravo"]);

        let page2 = album_repo.list(2, 2).await.expect("page2");
        let t2: Vec<_> = page2.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(t2, vec!["Delta", "Echo"]);

        let page3 = album_repo.list(2, 4).await.expect("page3");
        let t3: Vec<_> = page3.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(t3, vec!["Zebra"]);

        let empty = album_repo.list(2, 6).await.expect("empty");
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn album_cascading_delete_on_artist_removal() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        // Create albums
        let album1 = chorrosion_domain::Album::new(artist_id, "Album 1");
        let album1_id = album1.id;
        album_repo.create(album1).await.expect("create album1");

        let album2 = chorrosion_domain::Album::new(artist_id, "Album 2");
        let album2_id = album2.id;
        album_repo.create(album2).await.expect("create album2");

        // Verify albums exist
        let albums = album_repo.get_by_artist(artist_id, 10, 0).await.expect("get albums");
        assert_eq!(albums.len(), 2);

        // Delete artist (should cascade to albums due to FK constraint)
        artist_repo.delete(artist_id.to_string()).await.expect("delete artist");

        // Verify albums are also deleted
        let absent1 = album_repo.get_by_id(album1_id.to_string()).await.expect("get1");
        assert!(absent1.is_none());

        let absent2 = album_repo.get_by_id(album2_id.to_string()).await.expect("get2");
        assert!(absent2.is_none());
    }
}
