// SPDX-License-Identifier: GPL-3.0-or-later
use anyhow::{anyhow, Result};
use chorrosion_domain::{
    Album, AlbumId, AlbumStatus, Artist, ArtistId, ArtistStatus, MetadataProfile, ProfileId,
    QualityProfile, Track, TrackFile, TrackFileId, TrackId,
};
use sqlx::SqlitePool;
use sqlx::Row;
use tracing::debug;
use uuid::Uuid;
use chrono::{DateTime, NaiveDateTime, Utc};

use crate::repositories::{
    AlbumRepository, ArtistRepository, MetadataProfileRepository, QualityProfileRepository,
    Repository, TrackRepository, TrackFileRepository,
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

fn row_to_track(row: &sqlx::sqlite::SqliteRow) -> Result<Track> {
    let id_str: String = row.try_get("id")?;
    let id = chorrosion_domain::TrackId::from_uuid(Uuid::parse_str(&id_str)?);

    let album_id_str: String = row.try_get("album_id")?;
    let album_id = AlbumId::from_uuid(Uuid::parse_str(&album_id_str)?);

    let artist_id_str: String = row.try_get("artist_id")?;
    let artist_id = ArtistId::from_uuid(Uuid::parse_str(&artist_id_str)?);

    let foreign_track_id: Option<String> = row.try_get("foreign_track_id")?;
    let title: String = row.try_get("title")?;
    let track_number: Option<i32> = row.try_get("track_number")?;
    let duration_ms: Option<i32> = row.try_get("duration_ms")?;
    let has_file: bool = row.try_get("has_file")?;
    let monitored: bool = row.try_get("monitored")?;
    let musicbrainz_recording_id: Option<String> = row.try_get("musicbrainz_recording_id")?;
    let match_confidence: Option<f64> = row.try_get("match_confidence")?;
    let created_at_s: String = row.try_get("created_at")?;
    let updated_at_s: String = row.try_get("updated_at")?;

    Ok(Track {
        id,
        album_id,
        artist_id,
        foreign_track_id,
        title,
        track_number: track_number.map(|n| n as u32),
        duration_ms: duration_ms.map(|n| n as u32),
        has_file,
        monitored,
        musicbrainz_recording_id,
        match_confidence: match_confidence.map(|s| s as f32),
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
        let q = r#"
            INSERT INTO tracks (
                id, album_id, artist_id, foreign_track_id, title, track_number,
                duration_ms, has_file, monitored, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let id_str = entity.id.to_string();
        let album_id_str = entity.album_id.to_string();
        let artist_id_str = entity.artist_id.to_string();
        let foreign_id = entity.foreign_track_id.clone();
        let title = entity.title.clone();
        let track_number = entity.track_number.map(|n| n as i32);
        let duration_ms = entity.duration_ms.map(|n| n as i32);
        let has_file = entity.has_file;
        let monitored = entity.monitored;
        let created_at = entity.created_at.to_rfc3339();
        let updated_at = entity.updated_at.to_rfc3339();

        sqlx::query(q)
            .bind(id_str)
            .bind(album_id_str)
            .bind(artist_id_str)
            .bind(foreign_id)
            .bind(title)
            .bind(track_number)
            .bind(duration_ms)
            .bind(has_file)
            .bind(monitored)
            .bind(created_at)
            .bind(updated_at)
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<Track>> {
        let id = id.into();
        debug!(target: "repository", %id, "fetching track by id");
        let row = sqlx::query("SELECT * FROM tracks WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_track(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing tracks");
        let rows = sqlx::query("SELECT * FROM tracks ORDER BY track_number, title LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_track(&r)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: Track) -> Result<Track> {
        debug!(target: "repository", track_id = %entity.id, "updating track");
        let q = r#"
            UPDATE tracks SET
                album_id = ?,
                artist_id = ?,
                foreign_track_id = ?,
                title = ?,
                track_number = ?,
                duration_ms = ?,
                has_file = ?,
                monitored = ?,
                updated_at = ?
            WHERE id = ?
        "#;
        sqlx::query(q)
            .bind(entity.album_id.to_string())
            .bind(entity.artist_id.to_string())
            .bind(entity.foreign_track_id.clone())
            .bind(entity.title.clone())
            .bind(entity.track_number.map(|n| n as i32))
            .bind(entity.duration_ms.map(|n| n as i32))
            .bind(entity.has_file)
            .bind(entity.monitored)
            .bind(entity.updated_at.to_rfc3339())
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let id = id.into();
        debug!(target: "repository", %id, "deleting track");
        let result = sqlx::query("DELETE FROM tracks WHERE id = ?")
            .bind(&id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(anyhow!("track not found: {}", id));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl TrackRepository for SqliteTrackRepository {
    async fn get_by_album(&self, album_id: AlbumId, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", %album_id, limit, offset, "fetching tracks by album");
        let album_id_str = album_id.to_string();
        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE album_id = ? ORDER BY track_number, title LIMIT ? OFFSET ?"
        )
        .bind(album_id_str)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_track(&r)?);
        }
        Ok(out)
    }

    async fn get_by_artist(
        &self,
        artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Track>> {
        debug!(target: "repository", %artist_id, limit, offset, "fetching tracks by artist");
        let artist_id_str = artist_id.to_string();
        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE artist_id = ? ORDER BY track_number, title LIMIT ? OFFSET ?"
        )
        .bind(artist_id_str)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_track(&r)?);
        }
        Ok(out)
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Track>> {
        debug!(target: "repository", foreign_id, "fetching track by foreign_id");
        let row = sqlx::query("SELECT * FROM tracks WHERE foreign_track_id = ? LIMIT 1")
            .bind(foreign_id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_track(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing monitored tracks");
        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE monitored = 1 ORDER BY track_number, title LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_track(&r)?);
        }
        Ok(out)
    }

    async fn list_without_files(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing tracks without files");
        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE has_file = 0 ORDER BY track_number, title LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_track(&r)?);
        }
        Ok(out)
    }
}

// ============================================================================
// Helper functions for profiles
// ============================================================================

fn row_to_quality_profile(row: &sqlx::sqlite::SqliteRow) -> Result<QualityProfile> {
    let id: String = row.get("id");
    let name: String = row.get("name");
    let allowed_qualities_json: String = row.get("allowed_qualities");
    let upgrade_allowed: bool = row.get("upgrade_allowed");
    let cutoff_quality: Option<String> = row.get("cutoff_quality");

    let allowed_qualities: Vec<String> =
        serde_json::from_str(&allowed_qualities_json).unwrap_or_default();

    let profile_id = ProfileId::from_uuid(uuid::Uuid::parse_str(&id)?);

    Ok(QualityProfile {
        id: profile_id,
        name,
        allowed_qualities,
        upgrade_allowed,
        cutoff_quality,
        created_at: parse_dt(row.get("created_at"))?,
        updated_at: parse_dt(row.get("updated_at"))?,
    })
}

fn row_to_metadata_profile(row: &sqlx::sqlite::SqliteRow) -> Result<MetadataProfile> {
    let id: String = row.get("id");
    let name: String = row.get("name");
    let primary_json: Option<String> = row.get("primary_album_types");
    let secondary_json: Option<String> = row.get("secondary_album_types");
    let statuses_json: Option<String> = row.get("release_statuses");

    let primary_album_types = primary_json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();
    let secondary_album_types = secondary_json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();
    let release_statuses = statuses_json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();

    let profile_id = ProfileId::from_uuid(uuid::Uuid::parse_str(&id)?);

    Ok(MetadataProfile {
        id: profile_id,
        name,
        primary_album_types,
        secondary_album_types,
        release_statuses,
        created_at: parse_dt(row.get("created_at"))?,
        updated_at: parse_dt(row.get("updated_at"))?,
    })
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
        let id_str = entity.id.to_string();
        let qualities_json = serde_json::to_string(&entity.allowed_qualities)?;
        let created_at = entity.created_at.to_rfc3339();
        let updated_at = entity.updated_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO quality_profiles (
                id, name, allowed_qualities, upgrade_allowed, cutoff_quality, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id_str)
        .bind(entity.name.clone())
        .bind(qualities_json)
        .bind(entity.upgrade_allowed)
        .bind(entity.cutoff_quality.clone())
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<QualityProfile>> {
        let id = id.into();
        debug!(target: "repository", %id, "fetching quality profile by id");
        let row = sqlx::query("SELECT * FROM quality_profiles WHERE id = ? LIMIT 1")
            .bind(&id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_quality_profile(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<QualityProfile>> {
        debug!(target: "repository", limit, offset, "listing quality profiles");
        let rows = sqlx::query("SELECT * FROM quality_profiles ORDER BY name LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_quality_profile(&r)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: QualityProfile) -> Result<QualityProfile> {
        debug!(target: "repository", profile_id = %entity.id, "updating quality profile");
        let qualities_json = serde_json::to_string(&entity.allowed_qualities)?;
        let updated_at = entity.updated_at.to_rfc3339();

        sqlx::query(
            r#"
            UPDATE quality_profiles SET
                name = ?,
                allowed_qualities = ?,
                upgrade_allowed = ?,
                cutoff_quality = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(entity.name.clone())
        .bind(qualities_json)
        .bind(entity.upgrade_allowed)
        .bind(entity.cutoff_quality.clone())
        .bind(updated_at)
        .bind(entity.id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let id = id.into();
        debug!(target: "repository", %id, "deleting quality profile");
        let result = sqlx::query("DELETE FROM quality_profiles WHERE id = ?")
            .bind(&id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(anyhow!("quality profile not found: {}", id));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl QualityProfileRepository for SqliteQualityProfileRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<QualityProfile>> {
        debug!(target: "repository", name, "fetching quality profile by name");
        let row = sqlx::query("SELECT * FROM quality_profiles WHERE name = ? LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_quality_profile(&r)?))
        } else {
            Ok(None)
        }
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
        let id_str = entity.id.to_string();
        let primary_json = serde_json::to_string(&entity.primary_album_types)?;
        let secondary_json = serde_json::to_string(&entity.secondary_album_types)?;
        let statuses_json = serde_json::to_string(&entity.release_statuses)?;
        let created_at = entity.created_at.to_rfc3339();
        let updated_at = entity.updated_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO metadata_profiles (
                id, name, primary_album_types, secondary_album_types, release_statuses, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id_str)
        .bind(entity.name.clone())
        .bind(primary_json)
        .bind(secondary_json)
        .bind(statuses_json)
        .bind(created_at)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<MetadataProfile>> {
        let id = id.into();
        debug!(target: "repository", %id, "fetching metadata profile by id");
        let row = sqlx::query("SELECT * FROM metadata_profiles WHERE id = ? LIMIT 1")
            .bind(&id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_metadata_profile(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<MetadataProfile>> {
        debug!(target: "repository", limit, offset, "listing metadata profiles");
        let rows = sqlx::query("SELECT * FROM metadata_profiles ORDER BY name LIMIT ? OFFSET ?")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            out.push(row_to_metadata_profile(&r)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: MetadataProfile) -> Result<MetadataProfile> {
        debug!(target: "repository", profile_id = %entity.id, "updating metadata profile");
        let primary_json = serde_json::to_string(&entity.primary_album_types)?;
        let secondary_json = serde_json::to_string(&entity.secondary_album_types)?;
        let statuses_json = serde_json::to_string(&entity.release_statuses)?;
        let updated_at = entity.updated_at.to_rfc3339();

        sqlx::query(
            r#"
            UPDATE metadata_profiles SET
                name = ?,
                primary_album_types = ?,
                secondary_album_types = ?,
                release_statuses = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(entity.name.clone())
        .bind(primary_json)
        .bind(secondary_json)
        .bind(statuses_json)
        .bind(updated_at)
        .bind(entity.id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(entity)
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let id = id.into();
        debug!(target: "repository", %id, "deleting metadata profile");
        let result = sqlx::query("DELETE FROM metadata_profiles WHERE id = ?")
            .bind(&id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(anyhow!("metadata profile not found: {}", id));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl MetadataProfileRepository for SqliteMetadataProfileRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<MetadataProfile>> {
        debug!(target: "repository", name, "fetching metadata profile by name");
        let row = sqlx::query("SELECT * FROM metadata_profiles WHERE name = ? LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_metadata_profile(&r)?))
        } else {
            Ok(None)
        }
    }
}

// ============================================================================
// TrackFile Repository (SQLite)
// ============================================================================

/// SQLx-backed TrackFile repository
pub struct SqliteTrackFileRepository {
    pool: SqlitePool,
}

impl SqliteTrackFileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

/// Helper to convert a SQLx row to a TrackFile domain entity
fn row_to_track_file(row: &sqlx::sqlite::SqliteRow) -> Result<TrackFile> {
    let id_str: String = row.try_get("id")?;
    let track_id_str: String = row.try_get("track_id")?;
    let path_str: String = row.try_get("path")?;
    let size_bytes: i64 = row.try_get("size_bytes")?;
    let duration_ms: Option<i64> = row.try_get("duration_ms")?;
    let bitrate_kbps: Option<i64> = row.try_get("bitrate_kbps")?;
    let channels: Option<i64> = row.try_get("channels")?;
    let codec: Option<String> = row.try_get("codec")?;
    let hash: Option<String> = row.try_get("hash")?;
    let fingerprint_hash: Option<String> = row.try_get("fingerprint_hash")?;
    let fingerprint_duration: Option<i64> = row.try_get("fingerprint_duration")?;
    let fingerprint_computed_at: Option<String> = row.try_get("fingerprint_computed_at")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;

    Ok(TrackFile {
        id: TrackFileId(Uuid::parse_str(&id_str).map_err(|e| anyhow!("Invalid UUID: {}", e))?),
        track_id: TrackId(Uuid::parse_str(&track_id_str).map_err(|e| anyhow!("Invalid track UUID: {}", e))?),
        path: path_str,
        size_bytes: size_bytes as u64,
        duration_ms: duration_ms.map(|d| d as u32),
        bitrate_kbps: bitrate_kbps.map(|b| b as u32),
        channels: channels.map(|c| c as u8),
        codec,
        hash,
        fingerprint_hash,
        fingerprint_duration: fingerprint_duration.map(|d| d as u32),
        fingerprint_computed_at: fingerprint_computed_at
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .map_err(|e| anyhow!("Invalid fingerprint_computed_at timestamp: {}", e))?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| anyhow!("Invalid created_at: {}", e))?,
        updated_at: DateTime::parse_from_rfc3339(&updated_at)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| anyhow!("Invalid updated_at: {}", e))?,
    })
}

#[async_trait::async_trait]
impl Repository<TrackFile> for SqliteTrackFileRepository {
    async fn create(&self, entity: TrackFile) -> Result<TrackFile> {
        debug!(target: "repository", track_file_id = %entity.id, "creating track file");
        
        let q = r#"
            INSERT INTO track_files (
                id, track_id, path, size_bytes, duration_ms, bitrate_kbps,
                channels, codec, hash, fingerprint_hash, fingerprint_duration,
                fingerprint_computed_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        let id_str = entity.id.to_string();
        let track_id_str = entity.track_id.to_string();
        let path_str = &entity.path;
        let size_bytes = entity.size_bytes as i64;
        let duration_ms = entity.duration_ms.map(|d| d as i64);
        let bitrate_kbps = entity.bitrate_kbps.map(|b| b as i64);
        let channels = entity.channels.map(|c| c as i64);
        let codec = entity.codec.as_deref();
        let hash = entity.hash.as_deref();
        let fingerprint_hash = entity.fingerprint_hash.as_deref();
        let fingerprint_duration = entity.fingerprint_duration.map(|d| d as i64);
        let fingerprint_computed_at = entity.fingerprint_computed_at.map(|dt| dt.to_rfc3339());
        let created_at = entity.created_at.to_rfc3339();
        let updated_at = entity.updated_at.to_rfc3339();

        sqlx::query(q)
            .bind(&id_str)
            .bind(&track_id_str)
            .bind(path_str)
            .bind(size_bytes)
            .bind(duration_ms)
            .bind(bitrate_kbps)
            .bind(channels)
            .bind(codec)
            .bind(hash)
            .bind(fingerprint_hash)
            .bind(fingerprint_duration)
            .bind(fingerprint_computed_at.as_deref())
            .bind(&created_at)
            .bind(&updated_at)
            .execute(&self.pool)
            .await?;

        debug!(target: "repository", track_file_id = %entity.id, "track file created successfully");
        Ok(entity)
    }

    async fn get_by_id(&self, id: impl Into<String> + Send) -> Result<Option<TrackFile>> {
        let id_str = id.into();
        debug!(target: "repository", track_file_id = %id_str, "fetching track file by id");
        
        let q = "SELECT * FROM track_files WHERE id = ?";
        let row = sqlx::query(q)
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(row_to_track_file(&r)?)),
            None => Ok(None),
        }
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<TrackFile>> {
        debug!(target: "repository", limit, offset, "listing track files");
        
        let q = "SELECT * FROM track_files ORDER BY created_at DESC LIMIT ? OFFSET ?";
        let rows = sqlx::query(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_track_file).collect()
    }

    async fn update(&self, entity: TrackFile) -> Result<TrackFile> {
        debug!(target: "repository", track_file_id = %entity.id, "updating track file");
        
        let q = r#"
            UPDATE track_files SET
                path = ?, size_bytes = ?, duration_ms = ?, bitrate_kbps = ?,
                channels = ?, codec = ?, hash = ?, fingerprint_hash = ?,
                fingerprint_duration = ?, fingerprint_computed_at = ?, updated_at = ?
            WHERE id = ?
        "#;

        let id_str = entity.id.to_string();
        let path_str = &entity.path;
        let size_bytes = entity.size_bytes as i64;
        let duration_ms = entity.duration_ms.map(|d| d as i64);
        let bitrate_kbps = entity.bitrate_kbps.map(|b| b as i64);
        let channels = entity.channels.map(|c| c as i64);
        let codec = entity.codec.as_deref();
        let hash = entity.hash.as_deref();
        let fingerprint_hash = entity.fingerprint_hash.as_deref();
        let fingerprint_duration = entity.fingerprint_duration.map(|d| d as i64);
        let fingerprint_computed_at = entity.fingerprint_computed_at.map(|dt| dt.to_rfc3339());
        let updated_at = Utc::now().to_rfc3339();

        sqlx::query(q)
            .bind(path_str)
            .bind(size_bytes)
            .bind(duration_ms)
            .bind(bitrate_kbps)
            .bind(channels)
            .bind(codec)
            .bind(hash)
            .bind(fingerprint_hash)
            .bind(fingerprint_duration)
            .bind(fingerprint_computed_at.as_deref())
            .bind(&updated_at)
            .bind(&id_str)
            .execute(&self.pool)
            .await?;

        debug!(target: "repository", track_file_id = %entity.id, "track file updated successfully");
        
        // Return updated entity with new timestamp
        self.get_by_id(id_str)
            .await?
            .ok_or_else(|| anyhow!("Track file disappeared after update"))
    }

    async fn delete(&self, id: impl Into<String> + Send) -> Result<()> {
        let id_str = id.into();
        debug!(target: "repository", track_file_id = %id_str, "deleting track file");
        
        let q = "DELETE FROM track_files WHERE id = ?";
        sqlx::query(q)
            .bind(&id_str)
            .execute(&self.pool)
            .await?;

        debug!(target: "repository", track_file_id = %id_str, "track file deleted successfully");
        Ok(())
    }
}

#[async_trait::async_trait]
impl TrackFileRepository for SqliteTrackFileRepository {
    async fn get_by_track(
        &self,
        track_id: TrackId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TrackFile>> {
        debug!(target: "repository", track_id = %track_id, limit, offset, "fetching track files by track");
        
        let q = "SELECT * FROM track_files WHERE track_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?";
        let rows = sqlx::query(q)
            .bind(track_id.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_track_file).collect()
    }

    async fn get_by_path(&self, path: &str) -> Result<Option<TrackFile>> {
        debug!(target: "repository", path, "fetching track file by path");
        
        let q = "SELECT * FROM track_files WHERE path = ?";
        let row = sqlx::query(q)
            .bind(path)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(Some(row_to_track_file(&r)?)),
            None => Ok(None),
        }
    }

    async fn list_with_fingerprints(&self, limit: i64, offset: i64) -> Result<Vec<TrackFile>> {
        debug!(target: "repository", limit, offset, "listing track files with fingerprints");
        
        let q = "SELECT * FROM track_files WHERE fingerprint_hash IS NOT NULL ORDER BY created_at DESC LIMIT ? OFFSET ?";
        let rows = sqlx::query(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_track_file).collect()
    }

    async fn list_without_fingerprints(&self, limit: i64, offset: i64) -> Result<Vec<TrackFile>> {
        debug!(target: "repository", limit, offset, "listing track files without fingerprints");
        
        let q = "SELECT * FROM track_files WHERE fingerprint_hash IS NULL ORDER BY created_at DESC LIMIT ? OFFSET ?";
        let rows = sqlx::query(q)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_track_file).collect()
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

    // ========================================================================
    // Track repository tests
    // ========================================================================

    #[tokio::test]
    async fn track_create_and_get_by_id_round_trip() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        let album = chorrosion_domain::Album::new(artist_id, "Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");

        let mut track = chorrosion_domain::Track::new(album_id, artist_id, "Track 1");
        track.foreign_track_id = Some("mbid-123".to_string());
        track.track_number = Some(1);
        track.duration_ms = Some(180000);
        track.has_file = true;
        track.monitored = true;

        let created = track_repo.create(track.clone()).await.expect("create");
        assert_eq!(created.title, "Track 1");
        assert_eq!(created.track_number, Some(1));

        let fetched = track_repo.get_by_id(track.id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Track 1");
        assert_eq!(fetched.album_id, album_id);
        assert_eq!(fetched.artist_id, artist_id);
        assert_eq!(fetched.foreign_track_id.as_deref(), Some("mbid-123"));
        assert_eq!(fetched.track_number, Some(1));
        assert_eq!(fetched.duration_ms, Some(180000));
        assert!(fetched.has_file);
        assert!(fetched.monitored);
    }

    #[tokio::test]
    async fn track_get_by_album_and_artist() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());

        // Create two artists
        let artist1 = chorrosion_domain::Artist::new("Artist 1");
        let artist1_id = artist1.id;
        artist_repo.create(artist1).await.expect("create artist1");

        let artist2 = chorrosion_domain::Artist::new("Artist 2");
        let artist2_id = artist2.id;
        artist_repo.create(artist2).await.expect("create artist2");

        // Create two albums for artist1
        let album1 = chorrosion_domain::Album::new(artist1_id, "Album 1");
        let album1_id = album1.id;
        album_repo.create(album1).await.expect("create album1");

        let album2 = chorrosion_domain::Album::new(artist1_id, "Album 2");
        let album2_id = album2.id;
        album_repo.create(album2).await.expect("create album2");

        // Create tracks
        let track1 = chorrosion_domain::Track::new(album1_id, artist1_id, "Track 1");
        track_repo.create(track1).await.expect("create track1");

        let track2 = chorrosion_domain::Track::new(album1_id, artist1_id, "Track 2");
        track_repo.create(track2).await.expect("create track2");

        let track3 = chorrosion_domain::Track::new(album2_id, artist1_id, "Track 3");
        track_repo.create(track3).await.expect("create track3");

        let track4 = chorrosion_domain::Track::new(album2_id, artist2_id, "Track 4");
        track_repo.create(track4).await.expect("create track4");

        // Query by album1
        let album1_tracks = track_repo.get_by_album(album1_id, 10, 0).await.expect("get by album");
        assert_eq!(album1_tracks.len(), 2);
        assert_eq!(album1_tracks[0].title, "Track 1");
        assert_eq!(album1_tracks[1].title, "Track 2");

        // Query by artist1 (should include tracks from both albums)
        let artist1_tracks = track_repo.get_by_artist(artist1_id, 10, 0).await.expect("get by artist");
        assert_eq!(artist1_tracks.len(), 3);

        // Query by artist2
        let artist2_tracks = track_repo.get_by_artist(artist2_id, 10, 0).await.expect("get by artist");
        assert_eq!(artist2_tracks.len(), 1);
        assert_eq!(artist2_tracks[0].title, "Track 4");
    }

    #[tokio::test]
    async fn track_get_by_foreign_id() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        let album = chorrosion_domain::Album::new(artist_id, "Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");

        let mut track = chorrosion_domain::Track::new(album_id, artist_id, "Track");
        track.foreign_track_id = Some("mbid-456".to_string());
        track_repo.create(track.clone()).await.expect("create");

        let fetched = track_repo.get_by_foreign_id("mbid-456").await.unwrap().unwrap();
        assert_eq!(fetched.title, "Track");
        assert_eq!(fetched.id, track.id);

        let absent = track_repo.get_by_foreign_id("nonexistent").await.unwrap();
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn track_list_monitored_and_without_files() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        let album = chorrosion_domain::Album::new(artist_id, "Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");

        // Create tracks with different combinations
        let mut track1 = chorrosion_domain::Track::new(album_id, artist_id, "Monitored with file");
        track1.monitored = true;
        track1.has_file = true;
        track_repo.create(track1).await.expect("create");

        let mut track2 = chorrosion_domain::Track::new(album_id, artist_id, "Monitored without file");
        track2.monitored = true;
        track2.has_file = false;
        track_repo.create(track2).await.expect("create");

        let mut track3 = chorrosion_domain::Track::new(album_id, artist_id, "Not monitored without file");
        track3.monitored = false;
        track3.has_file = false;
        track_repo.create(track3).await.expect("create");

        let mut track4 = chorrosion_domain::Track::new(album_id, artist_id, "Not monitored with file");
        track4.monitored = false;
        track4.has_file = true;
        track_repo.create(track4).await.expect("create");

        // Query monitored
        let monitored = track_repo.list_monitored(10, 0).await.expect("list monitored");
        assert_eq!(monitored.len(), 2);
        let monitored_titles: Vec<_> = monitored.iter().map(|t| t.title.as_str()).collect();
        assert!(monitored_titles.contains(&"Monitored with file"));
        assert!(monitored_titles.contains(&"Monitored without file"));

        // Query without files
        let without_files = track_repo.list_without_files(10, 0).await.expect("list without files");
        assert_eq!(without_files.len(), 2);
        let without_files_titles: Vec<_> = without_files.iter().map(|t| t.title.as_str()).collect();
        assert!(without_files_titles.contains(&"Monitored without file"));
        assert!(without_files_titles.contains(&"Not monitored without file"));
    }

    #[tokio::test]
    async fn track_update_and_delete_flow() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        let album = chorrosion_domain::Album::new(artist_id, "Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");

        let mut track = chorrosion_domain::Track::new(album_id, artist_id, "Before");
        let track_id = track.id;
        track_repo.create(track.clone()).await.expect("create");

        // Update fields
        track.title = "After".to_string();
        track.track_number = Some(5);
        track.duration_ms = Some(240000);
        track.has_file = true;
        track.monitored = false;
        let updated = track_repo.update(track.clone()).await.expect("update");
        assert_eq!(updated.title, "After");

        let fetched = track_repo.get_by_id(track_id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.title, "After");
        assert_eq!(fetched.track_number, Some(5));
        assert_eq!(fetched.duration_ms, Some(240000));
        assert!(fetched.has_file);
        assert!(!fetched.monitored);

        // Delete and ensure gone
        track_repo.delete(track_id.to_string()).await.expect("delete");
        let absent = track_repo.get_by_id(track_id.to_string()).await.expect("get");
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn track_list_ordering_and_pagination() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        let album = chorrosion_domain::Album::new(artist_id, "Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");

        // Create tracks with track numbers
        for (num, title) in [(3, "Track 3"), (1, "Track 1"), (2, "Track 2")] {
            let mut track = chorrosion_domain::Track::new(album_id, artist_id, title);
            track.track_number = Some(num);
            track_repo.create(track).await.expect("create");
        }

        // Create one without track number (SQLite sorts NULL first)
        let track_no_num = chorrosion_domain::Track::new(album_id, artist_id, "Track No Number");
        track_repo.create(track_no_num).await.expect("create");

        // Verify ordering by track_number, then title
        let all = track_repo.list(10, 0).await.expect("list");
        assert_eq!(all.len(), 4);
        // SQLite sorts NULL first, so NULL, then track_number 1,2,3
        assert_eq!(all[0].title, "Track No Number");
        assert_eq!(all[1].title, "Track 1");
        assert_eq!(all[2].title, "Track 2");
        assert_eq!(all[3].title, "Track 3");

        // Test pagination
        let page1 = track_repo.list(2, 0).await.expect("list page 1");
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].title, "Track No Number");
        assert_eq!(page1[1].title, "Track 1");

        let page2 = track_repo.list(2, 2).await.expect("list page 2");
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].title, "Track 2");
        assert_eq!(page2[1].title, "Track 3");
    }

    #[tokio::test]
    async fn track_cascading_delete_on_album_removal() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        let album = chorrosion_domain::Album::new(artist_id, "Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");

        let track = chorrosion_domain::Track::new(album_id, artist_id, "Track");
        let track_id = track.id;
        track_repo.create(track).await.expect("create track");

        // Verify track exists
        let exists = track_repo.get_by_id(track_id.to_string()).await.unwrap();
        assert!(exists.is_some());

        // Delete album should cascade to tracks
        album_repo.delete(album_id.to_string()).await.expect("delete album");
        let track_check = track_repo.get_by_id(track_id.to_string()).await.expect("query");
        assert!(track_check.is_none(), "track should be gone when album deleted");
    }

    #[tokio::test]
    async fn track_cascading_delete_on_artist_removal() {
        let pool = setup_pool().await;
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());

        let artist = chorrosion_domain::Artist::new("Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");

        let album = chorrosion_domain::Album::new(artist_id, "Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");

        let track = chorrosion_domain::Track::new(album_id, artist_id, "Track");
        let track_id = track.id;
        track_repo.create(track).await.expect("create track");

        // Verify track exists
        let exists = track_repo.get_by_id(track_id.to_string()).await.unwrap();
        assert!(exists.is_some());

        // Delete artist should cascade to both albums and tracks
        artist_repo.delete(artist_id.to_string()).await.expect("delete artist");
        let track_check = track_repo.get_by_id(track_id.to_string()).await.expect("query");
        assert!(track_check.is_none(), "track should be gone when artist deleted");
    }

    // ========================================================================
    // Quality Profile repository tests
    // ========================================================================

    #[tokio::test]
    async fn quality_profile_create_and_get_by_id() {
        let pool = setup_pool().await;
        let profile_repo = SqliteQualityProfileRepository::new(pool.clone());

        let mut profile = chorrosion_domain::QualityProfile::new("Lossless", vec!["FLAC".to_string(), "WAV".to_string()]);
        profile.upgrade_allowed = true;
        profile.cutoff_quality = Some("FLAC".to_string());

        let created = profile_repo.create(profile.clone()).await.expect("create");
        assert_eq!(created.name, "Lossless");
        assert_eq!(created.allowed_qualities.len(), 2);
        assert!(created.upgrade_allowed);

        let fetched = profile_repo.get_by_id(profile.id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Lossless");
        assert_eq!(fetched.allowed_qualities, vec!["FLAC".to_string(), "WAV".to_string()]);
        assert_eq!(fetched.cutoff_quality.as_deref(), Some("FLAC"));
        assert!(fetched.upgrade_allowed);
    }

    #[tokio::test]
    async fn quality_profile_get_by_name() {
        let pool = setup_pool().await;
        let profile_repo = SqliteQualityProfileRepository::new(pool.clone());

        let profile = chorrosion_domain::QualityProfile::new("Lossy", vec!["MP3".to_string(), "AAC".to_string()]);
        profile_repo.create(profile.clone()).await.expect("create");

        let found = profile_repo.get_by_name("Lossy").await.unwrap().unwrap();
        assert_eq!(found.id, profile.id);
        assert_eq!(found.allowed_qualities, vec!["MP3".to_string(), "AAC".to_string()]);

        let absent = profile_repo.get_by_name("NonExistent").await.unwrap();
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn quality_profile_update_and_delete() {
        let pool = setup_pool().await;
        let profile_repo = SqliteQualityProfileRepository::new(pool.clone());

        let mut profile = chorrosion_domain::QualityProfile::new("Original", vec!["FLAC".to_string()]);
        let profile_id = profile.id;
        profile_repo.create(profile.clone()).await.expect("create");

        // Update
        profile.name = "Updated".to_string();
        profile.allowed_qualities.push("WAV".to_string());
        profile.upgrade_allowed = true;
        let updated = profile_repo.update(profile).await.expect("update");
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.allowed_qualities.len(), 2);

        let fetched = profile_repo.get_by_id(profile_id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Updated");

        // Delete
        profile_repo.delete(profile_id.to_string()).await.expect("delete");
        let absent = profile_repo.get_by_id(profile_id.to_string()).await.unwrap();
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn quality_profile_list_ordering() {
        let pool = setup_pool().await;
        let profile_repo = SqliteQualityProfileRepository::new(pool.clone());

        for name in ["Zebra", "Alpha", "Bravo"] {
            let profile = chorrosion_domain::QualityProfile::new(name, vec!["FLAC".to_string()]);
            profile_repo.create(profile).await.expect("create");
        }

        let profiles = profile_repo.list(10, 0).await.expect("list");
        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0].name, "Alpha");
        assert_eq!(profiles[1].name, "Bravo");
        assert_eq!(profiles[2].name, "Zebra");
    }

    #[tokio::test]
    async fn quality_profile_list_pagination() {
        let pool = setup_pool().await;
        let profile_repo = SqliteQualityProfileRepository::new(pool.clone());

        for i in 0..5 {
            let profile = chorrosion_domain::QualityProfile::new(
                format!("Profile{}", i),
                vec!["FLAC".to_string()],
            );
            profile_repo.create(profile).await.expect("create");
        }

        let page1 = profile_repo.list(2, 0).await.expect("list page 1");
        assert_eq!(page1.len(), 2);

        let page2 = profile_repo.list(2, 2).await.expect("list page 2");
        assert_eq!(page2.len(), 2);

        let page3 = profile_repo.list(2, 4).await.expect("list page 3");
        assert_eq!(page3.len(), 1);
    }

    // ========================================================================
    // Metadata Profile repository tests
    // ========================================================================

    #[tokio::test]
    async fn metadata_profile_create_and_get_by_id() {
        let pool = setup_pool().await;
        let profile_repo = SqliteMetadataProfileRepository::new(pool.clone());

        let mut profile = chorrosion_domain::MetadataProfile::new("Standard");
        profile.primary_album_types = vec!["Album".to_string(), "EP".to_string()];
        profile.secondary_album_types = vec!["Compilation".to_string()];
        profile.release_statuses = vec!["Official".to_string(), "Promotion".to_string()];

        let created = profile_repo.create(profile.clone()).await.expect("create");
        assert_eq!(created.name, "Standard");
        assert_eq!(created.primary_album_types.len(), 2);
        assert_eq!(created.secondary_album_types.len(), 1);
        assert_eq!(created.release_statuses.len(), 2);

        let fetched = profile_repo.get_by_id(profile.id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Standard");
        assert_eq!(fetched.primary_album_types, vec!["Album".to_string(), "EP".to_string()]);
        assert_eq!(fetched.secondary_album_types, vec!["Compilation".to_string()]);
        assert_eq!(fetched.release_statuses, vec!["Official".to_string(), "Promotion".to_string()]);
    }

    #[tokio::test]
    async fn metadata_profile_get_by_name() {
        let pool = setup_pool().await;
        let profile_repo = SqliteMetadataProfileRepository::new(pool.clone());

        let mut profile = chorrosion_domain::MetadataProfile::new("Jazz");
        profile.primary_album_types = vec!["Album".to_string()];
        profile_repo.create(profile.clone()).await.expect("create");

        let found = profile_repo.get_by_name("Jazz").await.unwrap().unwrap();
        assert_eq!(found.id, profile.id);
        assert_eq!(found.primary_album_types, vec!["Album".to_string()]);

        let absent = profile_repo.get_by_name("NonExistent").await.unwrap();
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn metadata_profile_update_and_delete() {
        let pool = setup_pool().await;
        let profile_repo = SqliteMetadataProfileRepository::new(pool.clone());

        let mut profile = chorrosion_domain::MetadataProfile::new("Original");
        let profile_id = profile.id;
        profile.primary_album_types = vec!["Album".to_string()];
        profile_repo.create(profile.clone()).await.expect("create");

        // Update
        profile.name = "Updated".to_string();
        profile.primary_album_types.push("EP".to_string());
        profile.release_statuses = vec!["Official".to_string()];
        let updated = profile_repo.update(profile).await.expect("update");
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.primary_album_types.len(), 2);
        assert_eq!(updated.release_statuses.len(), 1);

        let fetched = profile_repo.get_by_id(profile_id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Updated");

        // Delete
        profile_repo.delete(profile_id.to_string()).await.expect("delete");
        let absent = profile_repo.get_by_id(profile_id.to_string()).await.unwrap();
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn metadata_profile_list_ordering() {
        let pool = setup_pool().await;
        let profile_repo = SqliteMetadataProfileRepository::new(pool.clone());

        for name in ["Zebra", "Alpha", "Bravo"] {
            let profile = chorrosion_domain::MetadataProfile::new(name);
            profile_repo.create(profile).await.expect("create");
        }

        let profiles = profile_repo.list(10, 0).await.expect("list");
        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0].name, "Alpha");
        assert_eq!(profiles[1].name, "Bravo");
        assert_eq!(profiles[2].name, "Zebra");
    }

    #[tokio::test]
    async fn metadata_profile_empty_arrays() {
        let pool = setup_pool().await;
        let profile_repo = SqliteMetadataProfileRepository::new(pool.clone());

        let profile = chorrosion_domain::MetadataProfile::new("Empty");
        let created = profile_repo.create(profile.clone()).await.expect("create");

        assert!(created.primary_album_types.is_empty());
        assert!(created.secondary_album_types.is_empty());
        assert!(created.release_statuses.is_empty());

        let fetched = profile_repo.get_by_id(profile.id.to_string()).await.unwrap().unwrap();
        assert!(fetched.primary_album_types.is_empty());
        assert!(fetched.secondary_album_types.is_empty());
        assert!(fetched.release_statuses.is_empty());
    }

    // ========================================================================
    // TrackFile Repository Tests
    // ========================================================================

    #[tokio::test]
    async fn track_file_crud() {
        let pool = setup_pool().await;
        let track_file_repo = SqliteTrackFileRepository::new(pool.clone());
        
        // Create artist, album, and track first (TrackFile requires a valid track_id)
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());
        
        let artist = chorrosion_domain::Artist::new("Test Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");
        
        let album = chorrosion_domain::Album::new(artist_id, "Test Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");
        
        let track = chorrosion_domain::Track::new(
            album_id,
            artist_id,
            "Test Track".to_string(),
        );
        track_repo.create(track.clone()).await.expect("create track");

        // Create track file
        let track_file = chorrosion_domain::TrackFile::new(
            track.id,
            "/path/to/test.mp3".to_string(),
            1024,
        );
        let track_file_id = track_file.id;
        
        let created = track_file_repo.create(track_file.clone()).await.expect("create");
        assert_eq!(created.id, track_file_id);
        assert_eq!(created.track_id, track.id);
        assert_eq!(created.path, "/path/to/test.mp3");
        assert_eq!(created.size_bytes, 1024);

        // Get by id
        let fetched = track_file_repo.get_by_id(track_file_id.to_string()).await.unwrap().unwrap();
        assert_eq!(fetched.id, track_file_id);
        assert_eq!(fetched.path, "/path/to/test.mp3");

        // Update
        let mut updated_file = fetched.clone();
        updated_file.path = "/new/path/test.mp3".to_string();
        updated_file.size_bytes = 2048;
        updated_file.fingerprint_hash = Some("fingerprint_hash_value".to_string());
        updated_file.fingerprint_duration = Some(180);
        
        let updated = track_file_repo.update(updated_file).await.expect("update");
        assert_eq!(updated.path, "/new/path/test.mp3");
        assert_eq!(updated.size_bytes, 2048);
        assert_eq!(updated.fingerprint_hash, Some("fingerprint_hash_value".to_string()));
        assert_eq!(updated.fingerprint_duration, Some(180));

        // Delete
        track_file_repo.delete(track_file_id.to_string()).await.expect("delete");
        let absent = track_file_repo.get_by_id(track_file_id.to_string()).await.unwrap();
        assert!(absent.is_none());
    }

    #[tokio::test]
    async fn track_file_get_by_track() {
        let pool = setup_pool().await;
        let track_file_repo = SqliteTrackFileRepository::new(pool.clone());
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());
        
        // Create artist and album first
        let artist = chorrosion_domain::Artist::new("Test Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");
        
        let album = chorrosion_domain::Album::new(artist_id, "Test Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");
        
        // Create tracks
        let track1 = chorrosion_domain::Track::new(album_id, artist_id, "Track 1".to_string());
        let track2 = chorrosion_domain::Track::new(album_id, artist_id, "Track 2".to_string());
        track_repo.create(track1.clone()).await.expect("create track1");
        track_repo.create(track2.clone()).await.expect("create track2");

        // Create track files
        for i in 0..3 {
            let track_file = chorrosion_domain::TrackFile::new(
                track1.id,
                format!("/path/to/track1_{}.mp3", i),
                1024 * i,
            );
            track_file_repo.create(track_file).await.expect("create");
        }
        
        let track_file = chorrosion_domain::TrackFile::new(
            track2.id,
            "/path/to/track2.mp3".to_string(),
            1024,
        );
        track_file_repo.create(track_file).await.expect("create");

        // Get by track
        let files = track_file_repo.get_by_track(track1.id, 10, 0).await.expect("get_by_track");
        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| f.track_id == track1.id));
        
        let files2 = track_file_repo.get_by_track(track2.id, 10, 0).await.expect("get_by_track");
        assert_eq!(files2.len(), 1);
        assert_eq!(files2[0].track_id, track2.id);
    }

    #[tokio::test]
    async fn track_file_get_by_path() {
        let pool = setup_pool().await;
        let track_file_repo = SqliteTrackFileRepository::new(pool.clone());
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());
        
        let artist = chorrosion_domain::Artist::new("Test Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");
        
        let album = chorrosion_domain::Album::new(artist_id, "Test Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");
        
        let track = chorrosion_domain::Track::new(album_id, artist_id, "Test Track".to_string());
        track_repo.create(track.clone()).await.expect("create track");

        let track_file = chorrosion_domain::TrackFile::new(
            track.id,
            "/unique/path/test.mp3".to_string(),
            1024,
        );
        track_file_repo.create(track_file.clone()).await.expect("create");

        // Get by path
        let found = track_file_repo.get_by_path("/unique/path/test.mp3").await.expect("get_by_path");
        assert!(found.is_some());
        assert_eq!(found.unwrap().path, "/unique/path/test.mp3");

        // Not found
        let not_found = track_file_repo.get_by_path("/nonexistent.mp3").await.expect("get_by_path");
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn track_file_list_with_without_fingerprints() {
        let pool = setup_pool().await;
        let track_file_repo = SqliteTrackFileRepository::new(pool.clone());
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());
        
        let artist = chorrosion_domain::Artist::new("Test Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");
        
        let album = chorrosion_domain::Album::new(artist_id, "Test Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");
        
        let track = chorrosion_domain::Track::new(album_id, artist_id, "Test Track".to_string());
        track_repo.create(track.clone()).await.expect("create track");

        // Create files with fingerprints
        for i in 0..2 {
            let mut track_file = chorrosion_domain::TrackFile::new(
                track.id,
                format!("/path/with_fp_{}.mp3", i),
                1024,
            );
            track_file.fingerprint_hash = Some(format!("hash_{}", i));
            track_file.fingerprint_duration = Some(180);
            track_file_repo.create(track_file).await.expect("create");
        }

        // Create files without fingerprints
        for i in 0..3 {
            let track_file = chorrosion_domain::TrackFile::new(
                track.id,
                format!("/path/without_fp_{}.mp3", i),
                1024,
            );
            track_file_repo.create(track_file).await.expect("create");
        }

        // List with fingerprints
        let with_fp = track_file_repo.list_with_fingerprints(10, 0).await.expect("list_with_fingerprints");
        assert_eq!(with_fp.len(), 2);
        assert!(with_fp.iter().all(|f| f.fingerprint_hash.is_some()));

        // List without fingerprints
        let without_fp = track_file_repo.list_without_fingerprints(10, 0).await.expect("list_without_fingerprints");
        assert_eq!(without_fp.len(), 3);
        assert!(without_fp.iter().all(|f| f.fingerprint_hash.is_none()));
    }

    #[tokio::test]
    async fn track_file_list_pagination() {
        let pool = setup_pool().await;
        let track_file_repo = SqliteTrackFileRepository::new(pool.clone());
        let artist_repo = SqliteArtistRepository::new(pool.clone());
        let album_repo = SqliteAlbumRepository::new(pool.clone());
        let track_repo = SqliteTrackRepository::new(pool.clone());
        
        let artist = chorrosion_domain::Artist::new("Test Artist");
        let artist_id = artist.id;
        artist_repo.create(artist).await.expect("create artist");
        
        let album = chorrosion_domain::Album::new(artist_id, "Test Album");
        let album_id = album.id;
        album_repo.create(album).await.expect("create album");
        
        let track = chorrosion_domain::Track::new(album_id, artist_id, "Test Track".to_string());
        track_repo.create(track.clone()).await.expect("create track");

        // Create 10 track files
        for i in 0..10 {
            let track_file = chorrosion_domain::TrackFile::new(
                track.id,
                format!("/path/file_{}.mp3", i),
                1024,
            );
            track_file_repo.create(track_file).await.expect("create");
        }

        // First page
        let page1 = track_file_repo.list(5, 0).await.expect("list page 1");
        assert_eq!(page1.len(), 5);

        // Second page
        let page2 = track_file_repo.list(5, 5).await.expect("list page 2");
        assert_eq!(page2.len(), 5);

        // Verify no overlap
        let page1_ids: std::collections::HashSet<_> = page1.iter().map(|f| f.id).collect();
        let page2_ids: std::collections::HashSet<_> = page2.iter().map(|f| f.id).collect();
        assert!(page1_ids.is_disjoint(&page2_ids));
    }
}
