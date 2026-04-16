// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "postgres")]

use anyhow::{anyhow, Result};
use chorrosion_domain::{
    Album, AlbumId, AlbumStatus, Artist, ArtistId, ArtistRelationship, ArtistRelationshipId,
    ArtistStatus, DownloadClientDefinition, DownloadClientDefinitionId, IndexerDefinition,
    IndexerDefinitionId, MetadataProfile, ProfileId, QualityProfile, Track, TrackFile, TrackFileId,
    TrackId,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use sqlx::postgres::PgPool;
use sqlx::Row;
use tracing::debug;
use uuid::Uuid;

use crate::profiler::QueryProfiler;
use crate::repositories::{
    AlbumRepository, ArtistRelationshipRepository, ArtistRepository,
    DownloadClientDefinitionRepository, IndexerDefinitionRepository, MetadataProfileRepository,
    QualityProfileRepository, Repository, TrackFileRepository, TrackRepository,
};

/// PostgreSQL-backed Artist repository
#[allow(dead_code)]
pub struct PostgresArtistRepository {
    pool: PgPool,
    profiler: QueryProfiler,
}

impl PostgresArtistRepository {
    pub fn new(pool: PgPool) -> Self {
        let profiler = QueryProfiler::new(pool.clone(), 0);
        Self { pool, profiler }
    }

    pub fn new_with_threshold(pool: PgPool, threshold_ms: u64) -> Self {
        let profiler = QueryProfiler::new(pool.clone(), threshold_ms);
        Self { pool, profiler }
    }
}

#[async_trait::async_trait]
impl Repository<Artist> for PostgresArtistRepository {
    async fn create(&self, entity: Artist) -> Result<Artist> {
        debug!(target: "repository", artist_id = %entity.id, "creating artist");
        let q = r#"
            INSERT INTO artists (
                id, name, foreign_artist_id, musicbrainz_artist_id, metadata_profile_id, quality_profile_id,
                status, path, monitored, artist_type, sort_name, country, disambiguation, genre_tags, style_tags, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        "#;

        let id_str = entity.id.to_string();
        let foreign_id = entity.foreign_artist_id.clone();
        let musicbrainz_id = entity.musicbrainz_artist_id.clone();
        let metadata_id = entity.metadata_profile_id.map(|p| p.to_string());
        let quality_id = entity.quality_profile_id.map(|p| p.to_string());
        let status = entity.status.to_string();
        let path = entity.path.clone();
        let monitored = entity.monitored;
        let created_at = entity.created_at;
        let updated_at = entity.updated_at;

        sqlx::query(q)
            .bind(&id_str)
            .bind(&entity.name)
            .bind(&foreign_id)
            .bind(&musicbrainz_id)
            .bind(&metadata_id)
            .bind(&quality_id)
            .bind(&status)
            .bind(&path)
            .bind(monitored)
            .bind(&entity.artist_type)
            .bind(&entity.sort_name)
            .bind(&entity.country)
            .bind(&entity.disambiguation)
            .bind(&entity.genre_tags)
            .bind(&entity.style_tags)
            .bind(created_at)
            .bind(updated_at)
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Artist>> {
        debug!(target: "repository", %id, "fetching artist by id");
        let row = self
            .profiler
            .timed("artists::get_by_id", || async {
                sqlx::query("SELECT * FROM artists WHERE id = $1 LIMIT 1")
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await
            })
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_artist(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Artist>> {
        debug!(target: "repository", limit, offset, "listing artists");
        let rows = self
            .profiler
            .timed("artists::list", || async {
                sqlx::query("SELECT * FROM artists ORDER BY name LIMIT $1 OFFSET $2")
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            })
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
                name = $1,
                foreign_artist_id = $2,
                musicbrainz_artist_id = $3,
                metadata_profile_id = $4,
                quality_profile_id = $5,
                status = $6,
                path = $7,
                monitored = $8,
                artist_type = $9,
                sort_name = $10,
                country = $11,
                disambiguation = $12,
                genre_tags = $13,
                style_tags = $14,
                updated_at = $15
            WHERE id = $16
        "#;
        sqlx::query(q)
            .bind(&entity.name)
            .bind(&entity.foreign_artist_id)
            .bind(&entity.musicbrainz_artist_id)
            .bind(entity.metadata_profile_id.map(|p| p.to_string()))
            .bind(entity.quality_profile_id.map(|p| p.to_string()))
            .bind(entity.status.to_string())
            .bind(&entity.path)
            .bind(entity.monitored)
            .bind(&entity.artist_type)
            .bind(&entity.sort_name)
            .bind(&entity.country)
            .bind(&entity.disambiguation)
            .bind(&entity.genre_tags)
            .bind(&entity.style_tags)
            .bind(entity.updated_at)
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting artist");
        let result = sqlx::query("DELETE FROM artists WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            Err(anyhow!("artist {} not found", id))
        } else {
            debug!(target: "repository", %id, "artist deleted successfully");
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl ArtistRepository for PostgresArtistRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<Artist>> {
        debug!(target: "repository", name, "fetching artist by name");
        let row = sqlx::query("SELECT * FROM artists WHERE name = $1 LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_artist(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Artist>> {
        debug!(target: "repository", foreign_id, "fetching artist by foreign id");
        let row = sqlx::query("SELECT * FROM artists WHERE foreign_artist_id = $1 LIMIT 1")
            .bind(foreign_id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_artist(&r)?)),
            None => Ok(None),
        }
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Artist>> {
        debug!(target: "repository", limit, offset, "listing monitored artists");
        let rows = sqlx::query(
            "SELECT * FROM artists WHERE monitored = true ORDER BY name LIMIT $1 OFFSET $2",
        )
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

    async fn get_by_status(
        &self,
        status: ArtistStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Artist>> {
        debug!(target: "repository", status = ?status, "fetching artists by status");
        let status_str = status.to_string();
        let rows =
            sqlx::query("SELECT * FROM artists WHERE status = $1 ORDER BY name LIMIT $2 OFFSET $3")
                .bind(&status_str)
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
}

// ============================================================================
// Album Repository
// ============================================================================

#[allow(dead_code)]
pub struct PostgresAlbumRepository {
    pool: PgPool,
    profiler: QueryProfiler,
}

impl PostgresAlbumRepository {
    pub fn new(pool: PgPool) -> Self {
        let profiler = QueryProfiler::new(pool.clone(), 0);
        Self { pool, profiler }
    }

    pub fn new_with_threshold(pool: PgPool, threshold_ms: u64) -> Self {
        let profiler = QueryProfiler::new(pool.clone(), threshold_ms);
        Self { pool, profiler }
    }
}

#[async_trait::async_trait]
impl Repository<Album> for PostgresAlbumRepository {
    async fn create(&self, entity: Album) -> Result<Album> {
        debug!(target: "repository", album_id = %entity.id, "creating album");
        let q = r#"
            INSERT INTO albums (
                id, artist_id, title, album_type, musicbrainz_release_group_id,
                musicbrainz_release_id, foreign_album_id, release_date, disambiguation,
                status, monitored, quality_profile_id, metadata_profile_id, path,
                genre_tags, style_tags, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
        "#;

        let id_str = entity.id.to_string();
        let artist_id_str = entity.artist_id.to_string();
        let quality_id = entity.quality_profile_id.map(|p| p.to_string());
        let metadata_id = entity.metadata_profile_id.map(|p| p.to_string());
        let status = entity.status.to_string();

        sqlx::query(q)
            .bind(&id_str)
            .bind(&artist_id_str)
            .bind(&entity.title)
            .bind(&entity.album_type)
            .bind(&entity.musicbrainz_release_group_id)
            .bind(&entity.musicbrainz_release_id)
            .bind(&entity.foreign_album_id)
            .bind(entity.release_date)
            .bind(&entity.disambiguation)
            .bind(&status)
            .bind(entity.monitored)
            .bind(&quality_id)
            .bind(&metadata_id)
            .bind(&entity.path)
            .bind(&entity.genre_tags)
            .bind(&entity.style_tags)
            .bind(entity.created_at)
            .bind(entity.updated_at)
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Album>> {
        debug!(target: "repository", %id, "fetching album by id");
        let row = self
            .profiler
            .timed("albums::get_by_id", || async {
                sqlx::query("SELECT * FROM albums WHERE id = $1 LIMIT 1")
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await
            })
            .await?;
        if let Some(r) = row {
            Ok(Some(row_to_album(&r)?))
        } else {
            Ok(None)
        }
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing albums");
        let rows = self
            .profiler
            .timed("albums::list", || async {
                sqlx::query("SELECT * FROM albums ORDER BY title LIMIT $1 OFFSET $2")
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(&self.pool)
                    .await
            })
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
                artist_id = $1,
                title = $2,
                album_type = $3,
                musicbrainz_release_group_id = $4,
                musicbrainz_release_id = $5,
                foreign_album_id = $6,
                release_date = $7,
                disambiguation = $8,
                status = $9,
                monitored = $10,
                quality_profile_id = $11,
                metadata_profile_id = $12,
                path = $13,
                genre_tags = $14,
                style_tags = $15,
                updated_at = $16
            WHERE id = $17
        "#;
        sqlx::query(q)
            .bind(entity.artist_id.to_string())
            .bind(&entity.title)
            .bind(&entity.album_type)
            .bind(&entity.musicbrainz_release_group_id)
            .bind(&entity.musicbrainz_release_id)
            .bind(&entity.foreign_album_id)
            .bind(entity.release_date)
            .bind(&entity.disambiguation)
            .bind(entity.status.to_string())
            .bind(entity.monitored)
            .bind(entity.quality_profile_id.map(|p| p.to_string()))
            .bind(entity.metadata_profile_id.map(|p| p.to_string()))
            .bind(&entity.path)
            .bind(&entity.genre_tags)
            .bind(&entity.style_tags)
            .bind(entity.updated_at)
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting album");
        let result = sqlx::query("DELETE FROM albums WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            Err(anyhow!("album {} not found", id))
        } else {
            debug!(target: "repository", %id, "album deleted successfully");
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl AlbumRepository for PostgresAlbumRepository {
    async fn get_by_artist(
        &self,
        artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>> {
        debug!(target: "repository", artist_id = %artist_id, "fetching albums by artist");
        let artist_id_str = artist_id.to_string();
        let rows = sqlx::query("SELECT * FROM albums WHERE artist_id = $1 ORDER BY release_date DESC, title LIMIT $2 OFFSET $3")
            .bind(&artist_id_str)
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
        debug!(target: "repository", foreign_id, "fetching album by foreign id");
        let row = sqlx::query("SELECT * FROM albums WHERE foreign_album_id = $1 LIMIT 1")
            .bind(foreign_id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_album(&r)?)),
            None => Ok(None),
        }
    }

    async fn get_by_artist_and_title(
        &self,
        artist_id: ArtistId,
        title: &str,
    ) -> Result<Option<Album>> {
        debug!(target: "repository", artist_id = %artist_id, title, "fetching album by artist and title");
        let artist_id_str = artist_id.to_string();
        let row = sqlx::query(
            "SELECT * FROM albums WHERE artist_id = $1 AND LOWER(title) = LOWER($2) LIMIT 1",
        )
        .bind(&artist_id_str)
        .bind(title)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(r) => Ok(Some(row_to_album(&r)?)),
            None => Ok(None),
        }
    }

    async fn get_by_status(
        &self,
        status: AlbumStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>> {
        debug!(target: "repository", status = ?status, "fetching albums by status");
        let status_str = status.to_string();
        let rows = sqlx::query(
            "SELECT * FROM albums WHERE status = $1 ORDER BY release_date DESC LIMIT $2 OFFSET $3",
        )
        .bind(&status_str)
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
        let rows = sqlx::query("SELECT * FROM albums WHERE monitored = true ORDER BY release_date DESC LIMIT $1 OFFSET $2")
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
        debug!(target: "repository", album_type, "fetching albums by type");
        let rows = sqlx::query("SELECT * FROM albums WHERE album_type = $1 ORDER BY release_date DESC LIMIT $2 OFFSET $3")
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

    async fn list_wanted_without_tracks(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "fetching wanted albums without tracks");
        let rows = sqlx::query(r#"
            SELECT a.* FROM albums a
            WHERE a.status = 'Wanted' AND NOT EXISTS (SELECT 1 FROM tracks t WHERE t.album_id = a.id)
            ORDER BY a.release_date DESC
            LIMIT $1 OFFSET $2
        "#)
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

    async fn list_cutoff_unmet_albums(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "fetching albums with cutoff unmet");
        let rows = sqlx::query(r#"
            SELECT a.* FROM albums a
            WHERE a.monitored = true AND a.status = 'Wanted'
            AND (SELECT COUNT(*) FROM tracks t WHERE t.album_id = a.id AND t.quality IS NOT NULL) > 0
            AND (SELECT COUNT(*) FROM tracks t WHERE t.album_id = a.id AND t.quality IS NOT NULL)
                < (SELECT array_length(allowed_qualities, 1) FROM quality_profiles qp WHERE qp.id = a.quality_profile_id)
            ORDER BY a.release_date DESC
            LIMIT $1 OFFSET $2
        "#)
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

    async fn list_upcoming_releases(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>> {
        debug!(target: "repository", from = %start, to = %end, "fetching upcoming album releases");
        let rows = sqlx::query("SELECT * FROM albums WHERE release_date >= $1 AND release_date <= $2 ORDER BY release_date LIMIT $3 OFFSET $4")
            .bind(start)
            .bind(end)
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
// Helper Functions
// ============================================================================

fn row_to_artist(row: &sqlx::postgres::PgRow) -> Result<Artist> {
    let id: String = row.try_get("id")?;
    let monitored: bool = row.try_get("monitored")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;

    Ok(Artist {
        id: ArtistId::from_uuid(Uuid::parse_str(&id)?),
        name: row.try_get("name")?,
        foreign_artist_id: row.try_get("foreign_artist_id")?,
        musicbrainz_artist_id: row.try_get("musicbrainz_artist_id")?,
        metadata_profile_id: row
            .try_get::<Option<String>, _>("metadata_profile_id")?
            .map(|id| ProfileId::from_uuid(Uuid::parse_str(&id).unwrap())),
        quality_profile_id: row
            .try_get::<Option<String>, _>("quality_profile_id")?
            .map(|id| ProfileId::from_uuid(Uuid::parse_str(&id).unwrap())),
        status: row.try_get::<String, _>("status")?.parse()?,
        path: row.try_get("path")?,
        monitored,
        artist_type: row.try_get("artist_type")?,
        sort_name: row.try_get("sort_name")?,
        country: row.try_get("country")?,
        disambiguation: row.try_get("disambiguation")?,
        genre_tags: row.try_get("genre_tags")?,
        style_tags: row.try_get("style_tags")?,
        created_at,
        updated_at,
    })
}

fn row_to_album(row: &sqlx::postgres::PgRow) -> Result<Album> {
    let id: String = row.try_get("id")?;
    let artist_id: String = row.try_get("artist_id")?;
    let monitored: bool = row.try_get("monitored")?;
    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;

    Ok(Album {
        id: AlbumId::from_uuid(Uuid::parse_str(&id)?),
        artist_id: ArtistId::from_uuid(Uuid::parse_str(&artist_id)?),
        title: row.try_get("title")?,
        album_type: row.try_get("album_type")?,
        musicbrainz_release_group_id: row.try_get("musicbrainz_release_group_id")?,
        musicbrainz_release_id: row.try_get("musicbrainz_release_id")?,
        foreign_album_id: row.try_get("foreign_album_id")?,
        release_date: row.try_get("release_date")?,
        disambiguation: row.try_get("disambiguation")?,
        status: row.try_get::<String, _>("status")?.parse()?,
        monitored,
        quality_profile_id: row
            .try_get::<Option<String>, _>("quality_profile_id")?
            .map(|id| ProfileId::from_uuid(Uuid::parse_str(&id).unwrap())),
        metadata_profile_id: row
            .try_get::<Option<String>, _>("metadata_profile_id")?
            .map(|id| ProfileId::from_uuid(Uuid::parse_str(&id).unwrap())),
        path: row.try_get("path")?,
        genre_tags: row.try_get("genre_tags")?,
        style_tags: row.try_get("style_tags")?,
        created_at,
        updated_at,
    })
}
