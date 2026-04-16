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
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use sqlx::Row;
use tracing::debug;
use uuid::Uuid;

use crate::repositories::{
    AlbumRepository, ArtistRelationshipRepository, ArtistRepository,
    DownloadClientDefinitionRepository, IndexerDefinitionRepository, MetadataProfileRepository,
    QualityProfileRepository, Repository, TrackFileRepository, TrackRepository,
};

/// PostgreSQL-backed Artist repository scaffold.
pub struct PostgresArtistRepository {
    pool: PgPool,
}

impl PostgresArtistRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl Repository<Artist> for PostgresArtistRepository {
    async fn create(&self, entity: Artist) -> Result<Artist> {
        debug!(target: "repository", artist_id = %entity.id, "creating artist (postgres)");

        let q = r#"
            INSERT INTO artists (
                id, name, foreign_artist_id, musicbrainz_artist_id, metadata_profile_id, quality_profile_id,
                status, path, monitored, artist_type, sort_name, country, disambiguation, genre_tags, style_tags, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        "#;

        sqlx::query(q)
            .bind(entity.id.to_string())
            .bind(entity.name.clone())
            .bind(entity.foreign_artist_id.clone())
            .bind(entity.musicbrainz_artist_id.clone())
            .bind(entity.metadata_profile_id.map(|p| p.to_string()))
            .bind(entity.quality_profile_id.map(|p| p.to_string()))
            .bind(entity.status.to_string())
            .bind(entity.path.clone())
            .bind(entity.monitored)
            .bind(entity.artist_type.clone())
            .bind(entity.sort_name.clone())
            .bind(entity.country.clone())
            .bind(entity.disambiguation.clone())
            .bind(entity.genre_tags.clone())
            .bind(entity.style_tags.clone())
            .bind(entity.created_at.naive_utc())
            .bind(entity.updated_at.naive_utc())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Artist>> {
        debug!(target: "repository", %id, "fetching artist by id (postgres)");

        let row = sqlx::query("SELECT * FROM artists WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_artist(&r)).transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Artist>> {
        debug!(target: "repository", limit, offset, "listing artists (postgres)");

        let rows = sqlx::query("SELECT * FROM artists ORDER BY name LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_artist(&row)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: Artist) -> Result<Artist> {
        debug!(target: "repository", artist_id = %entity.id, "updating artist (postgres)");

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
            .bind(entity.name.clone())
            .bind(entity.foreign_artist_id.clone())
            .bind(entity.musicbrainz_artist_id.clone())
            .bind(entity.metadata_profile_id.map(|p| p.to_string()))
            .bind(entity.quality_profile_id.map(|p| p.to_string()))
            .bind(entity.status.to_string())
            .bind(entity.path.clone())
            .bind(entity.monitored)
            .bind(entity.artist_type.clone())
            .bind(entity.sort_name.clone())
            .bind(entity.country.clone())
            .bind(entity.disambiguation.clone())
            .bind(entity.genre_tags.clone())
            .bind(entity.style_tags.clone())
            .bind(entity.updated_at.naive_utc())
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting artist (postgres)");

        let result = sqlx::query("DELETE FROM artists WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("artist not found: {}", id));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ArtistRepository for PostgresArtistRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<Artist>> {
        debug!(target: "repository", name, "fetching artist by name (postgres)");

        // Escape '\' first so literal '%' and '_' remain literal under ILIKE ... ESCAPE '\'.
        let escaped_name = name
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");

        let row = sqlx::query("SELECT * FROM artists WHERE name ILIKE $1 ESCAPE '\\' LIMIT 1")
            .bind(escaped_name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_artist(&r)).transpose()?)
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Artist>> {
        debug!(target: "repository", foreign_id, "fetching artist by foreign_id (postgres)");

        let row = sqlx::query("SELECT * FROM artists WHERE foreign_artist_id = $1 LIMIT 1")
            .bind(foreign_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_artist(&r)).transpose()?)
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Artist>> {
        debug!(target: "repository", limit, offset, "listing monitored artists (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM artists WHERE monitored = true ORDER BY name LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_artist(&row)?);
        }
        Ok(out)
    }

    async fn get_by_status(
        &self,
        status: ArtistStatus,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Artist>> {
        debug!(target: "repository", ?status, limit, offset, "fetching artists by status (postgres)");

        let status_str = status.to_string();
        let rows =
            sqlx::query("SELECT * FROM artists WHERE status = $1 ORDER BY name LIMIT $2 OFFSET $3")
                .bind(status_str)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_artist(&row)?);
        }
        Ok(out)
    }
}

fn parse_profile_id_opt(value: Option<String>) -> Result<Option<chorrosion_domain::ProfileId>> {
    match value {
        Some(raw) => {
            let uuid = Uuid::parse_str(&raw)?;
            Ok(Some(chorrosion_domain::ProfileId::from_uuid(uuid)))
        }
        None => Ok(None),
    }
}

fn parse_artist_status(value: &str) -> Result<ArtistStatus> {
    match value {
        "continuing" => Ok(ArtistStatus::Continuing),
        "ended" => Ok(ArtistStatus::Ended),
        other => Err(anyhow!("unknown artist status: {}", other)),
    }
}

fn row_to_artist(row: &PgRow) -> Result<Artist> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let foreign_artist_id: Option<String> = row.try_get("foreign_artist_id")?;
    let musicbrainz_artist_id: Option<String> = row.try_get("musicbrainz_artist_id")?;
    let metadata_profile_id: Option<String> = row.try_get("metadata_profile_id")?;
    let quality_profile_id: Option<String> = row.try_get("quality_profile_id")?;
    let status: String = row.try_get("status")?;
    let path: Option<String> = row.try_get("path")?;
    let monitored: bool = row.try_get("monitored")?;
    let artist_type: Option<String> = row.try_get("artist_type")?;
    let sort_name: Option<String> = row.try_get("sort_name")?;
    let country: Option<String> = row.try_get("country")?;
    let disambiguation: Option<String> = row.try_get("disambiguation")?;
    let genre_tags: Option<String> = row.try_get("genre_tags")?;
    let style_tags: Option<String> = row.try_get("style_tags")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    Ok(Artist {
        id: ArtistId::from_uuid(Uuid::parse_str(&id)?),
        name,
        foreign_artist_id,
        musicbrainz_artist_id,
        metadata_profile_id: parse_profile_id_opt(metadata_profile_id)?,
        quality_profile_id: parse_profile_id_opt(quality_profile_id)?,
        status: parse_artist_status(&status)?,
        path,
        monitored,
        artist_type,
        sort_name,
        country,
        disambiguation,
        genre_tags,
        style_tags,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}

/// PostgreSQL-backed Album repository scaffold.
pub struct PostgresAlbumRepository {
    pool: PgPool,
}

impl PostgresAlbumRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// PostgreSQL-backed Track repository scaffold.
pub struct PostgresTrackRepository {
    pool: PgPool,
}

impl PostgresTrackRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// PostgreSQL-backed QualityProfile repository scaffold.
pub struct PostgresQualityProfileRepository {
    pool: PgPool,
}

impl PostgresQualityProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// PostgreSQL-backed MetadataProfile repository scaffold.
pub struct PostgresMetadataProfileRepository {
    pool: PgPool,
}

impl PostgresMetadataProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// PostgreSQL-backed IndexerDefinition repository scaffold.
pub struct PostgresIndexerDefinitionRepository {
    pool: PgPool,
}

impl PostgresIndexerDefinitionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// PostgreSQL-backed DownloadClientDefinition repository scaffold.
pub struct PostgresDownloadClientDefinitionRepository {
    pool: PgPool,
}

impl PostgresDownloadClientDefinitionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// PostgreSQL-backed TrackFile repository scaffold.
pub struct PostgresTrackFileRepository {
    pool: PgPool,
}

impl PostgresTrackFileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// PostgreSQL-backed ArtistRelationship repository scaffold.
pub struct PostgresArtistRelationshipRepository {
    pool: PgPool,
}

impl PostgresArtistRelationshipRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

// ============================================================================
// PostgresAlbumRepository
// ============================================================================

#[async_trait::async_trait]
impl Repository<Album> for PostgresAlbumRepository {
    async fn create(&self, entity: Album) -> Result<Album> {
        debug!(target: "repository", album_id = %entity.id, "creating album (postgres)");

        let q = r#"
            INSERT INTO albums (
                id, artist_id, foreign_album_id, musicbrainz_release_group_id, musicbrainz_release_id,
                title, release_date, album_type, primary_type, secondary_types, first_release_date,
                genre_tags, style_tags, status, monitored, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        "#;

        let release_date = entity
            .release_date
            .map(|d| d.format("%Y-%m-%d").to_string());

        sqlx::query(q)
            .bind(entity.id.to_string())
            .bind(entity.artist_id.to_string())
            .bind(entity.foreign_album_id.clone())
            .bind(entity.musicbrainz_release_group_id.clone())
            .bind(entity.musicbrainz_release_id.clone())
            .bind(entity.title.clone())
            .bind(release_date)
            .bind(entity.album_type.clone())
            .bind(entity.primary_type.clone())
            .bind(entity.secondary_types.clone())
            .bind(entity.first_release_date.clone())
            .bind(entity.genre_tags.clone())
            .bind(entity.style_tags.clone())
            .bind(entity.status.to_string())
            .bind(entity.monitored)
            .bind(entity.created_at.naive_utc())
            .bind(entity.updated_at.naive_utc())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Album>> {
        debug!(target: "repository", %id, "fetching album by id (postgres)");

        let row = sqlx::query("SELECT * FROM albums WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_album(&r)).transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing albums (postgres)");

        let rows = sqlx::query("SELECT * FROM albums ORDER BY title LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_album(&row)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: Album) -> Result<Album> {
        debug!(target: "repository", album_id = %entity.id, "updating album (postgres)");

        let q = r#"
            UPDATE albums SET
                artist_id = $1,
                foreign_album_id = $2,
                musicbrainz_release_group_id = $3,
                musicbrainz_release_id = $4,
                title = $5,
                release_date = $6,
                album_type = $7,
                primary_type = $8,
                secondary_types = $9,
                first_release_date = $10,
                genre_tags = $11,
                style_tags = $12,
                status = $13,
                monitored = $14,
                updated_at = $15
            WHERE id = $16
        "#;

        let release_date = entity
            .release_date
            .map(|d| d.format("%Y-%m-%d").to_string());

        sqlx::query(q)
            .bind(entity.artist_id.to_string())
            .bind(entity.foreign_album_id.clone())
            .bind(entity.musicbrainz_release_group_id.clone())
            .bind(entity.musicbrainz_release_id.clone())
            .bind(entity.title.clone())
            .bind(release_date)
            .bind(entity.album_type.clone())
            .bind(entity.primary_type.clone())
            .bind(entity.secondary_types.clone())
            .bind(entity.first_release_date.clone())
            .bind(entity.genre_tags.clone())
            .bind(entity.style_tags.clone())
            .bind(entity.status.to_string())
            .bind(entity.monitored)
            .bind(entity.updated_at.naive_utc())
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting album (postgres)");

        let result = sqlx::query("DELETE FROM albums WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("album not found: {}", id));
        }

        Ok(())
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
        debug!(target: "repository", %artist_id, limit, offset, "fetching albums by artist (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM albums WHERE artist_id = $1 ORDER BY title LIMIT $2 OFFSET $3",
        )
        .bind(artist_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_album(&row)?);
        }
        Ok(out)
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Album>> {
        debug!(target: "repository", foreign_id, "fetching album by foreign_id (postgres)");

        let row = sqlx::query("SELECT * FROM albums WHERE foreign_album_id = $1 LIMIT 1")
            .bind(foreign_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_album(&r)).transpose()?)
    }

    async fn get_by_artist_and_title(
        &self,
        artist_id: ArtistId,
        title: &str,
    ) -> Result<Option<Album>> {
        debug!(target: "repository", %artist_id, title, "fetching album by artist and title (postgres)");

        let row =
            sqlx::query("SELECT * FROM albums WHERE artist_id = $1 AND title ILIKE $2 LIMIT 1")
                .bind(artist_id.to_string())
                .bind(title)
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
        debug!(target: "repository", ?status, limit, offset, "fetching albums by status (postgres)");

        let rows =
            sqlx::query("SELECT * FROM albums WHERE status = $1 ORDER BY title LIMIT $2 OFFSET $3")
                .bind(status.to_string())
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_album(&row)?);
        }
        Ok(out)
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing monitored albums (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM albums WHERE monitored = true ORDER BY title LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_album(&row)?);
        }
        Ok(out)
    }

    async fn get_by_album_type(
        &self,
        album_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Album>> {
        debug!(target: "repository", album_type, limit, offset, "fetching albums by type (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM albums WHERE album_type = $1 ORDER BY title LIMIT $2 OFFSET $3",
        )
        .bind(album_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_album(&row)?);
        }
        Ok(out)
    }

    async fn list_wanted_without_tracks(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing wanted albums without tracks (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM albums \
             WHERE status = $1 \
             AND NOT EXISTS (SELECT 1 FROM tracks WHERE tracks.album_id = albums.id) \
             ORDER BY title LIMIT $2 OFFSET $3",
        )
        .bind(AlbumStatus::Wanted.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_album(&row)?);
        }
        Ok(out)
    }

    async fn list_cutoff_unmet_albums(&self, limit: i64, offset: i64) -> Result<Vec<Album>> {
        debug!(target: "repository", limit, offset, "listing cutoff-unmet albums (postgres)");

        // Postgres equivalent: use jsonb_array_elements_text + index via lateral join.
        // Albums are included when monitored, artist has upgrade_allowed profile with a cutoff,
        // and at least one monitored track file has a codec below or absent from the cutoff.
        let rows = sqlx::query(
            "SELECT a.* \
             FROM albums a \
             JOIN artists ar ON ar.id = a.artist_id \
             JOIN quality_profiles qp ON qp.id = ar.quality_profile_id \
             WHERE a.monitored = true \
               AND qp.upgrade_allowed = true \
               AND qp.cutoff_quality IS NOT NULL \
               AND EXISTS ( \
                 SELECT 1 FROM tracks t \
                 JOIN track_files tf ON tf.track_id = t.id \
                 WHERE t.album_id = a.id \
                   AND t.monitored = true \
                   AND ( \
                     tf.codec IS NULL \
                     OR NOT EXISTS ( \
                       SELECT 1 FROM jsonb_array_elements_text(qp.allowed_qualities::jsonb) WITH ORDINALITY AS q(val, ord) \
                       WHERE LOWER(q.val) = LOWER(tf.codec) \
                         AND q.ord <= ( \
                           SELECT ord FROM jsonb_array_elements_text(qp.allowed_qualities::jsonb) WITH ORDINALITY AS q2(val2, ord2) \
                           WHERE LOWER(q2.val2) = LOWER(qp.cutoff_quality) LIMIT 1 \
                         ) \
                     ) \
                   ) \
               ) \
             ORDER BY a.title LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_album(&row)?);
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
        debug!(target: "repository", %start, %end, limit, offset, "listing upcoming releases (postgres)");

        let start_str = start.format("%Y-%m-%d").to_string();
        let end_str = end.format("%Y-%m-%d").to_string();

        let rows = sqlx::query(
            "SELECT * FROM albums \
             WHERE monitored = true \
               AND release_date IS NOT NULL \
               AND release_date >= $1 \
               AND release_date <= $2 \
             ORDER BY release_date ASC, title ASC \
             LIMIT $3 OFFSET $4",
        )
        .bind(start_str)
        .bind(end_str)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_album(&row)?);
        }
        Ok(out)
    }
}

fn parse_album_status(value: &str) -> Result<AlbumStatus> {
    match value {
        "wanted" => Ok(AlbumStatus::Wanted),
        "released" => Ok(AlbumStatus::Released),
        "announced" => Ok(AlbumStatus::Announced),
        other => Err(anyhow!("unknown album status: {}", other)),
    }
}

fn row_to_album(row: &PgRow) -> Result<Album> {
    let id: String = row.try_get("id")?;
    let artist_id: String = row.try_get("artist_id")?;
    let foreign_album_id: Option<String> = row.try_get("foreign_album_id")?;
    let musicbrainz_release_group_id: Option<String> =
        row.try_get("musicbrainz_release_group_id")?;
    let musicbrainz_release_id: Option<String> = row.try_get("musicbrainz_release_id")?;
    let title: String = row.try_get("title")?;
    let release_date: Option<String> = row.try_get("release_date")?;
    let album_type: Option<String> = row.try_get("album_type")?;
    let primary_type: Option<String> = row.try_get("primary_type")?;
    let secondary_types: Option<String> = row.try_get("secondary_types")?;
    let first_release_date: Option<String> = row.try_get("first_release_date")?;
    let genre_tags: Option<String> = row.try_get("genre_tags")?;
    let style_tags: Option<String> = row.try_get("style_tags")?;
    let status: String = row.try_get("status")?;
    let monitored: bool = row.try_get("monitored")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    Ok(Album {
        id: AlbumId::from_uuid(Uuid::parse_str(&id)?),
        artist_id: ArtistId::from_uuid(Uuid::parse_str(&artist_id)?),
        foreign_album_id,
        musicbrainz_release_group_id,
        musicbrainz_release_id,
        title,
        release_date: release_date.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        album_type,
        primary_type,
        secondary_types,
        first_release_date,
        genre_tags,
        style_tags,
        status: parse_album_status(&status)?,
        monitored,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}

// ============================================================================
// PostgresTrackRepository
// ============================================================================

#[async_trait::async_trait]
impl Repository<Track> for PostgresTrackRepository {
    async fn create(&self, entity: Track) -> Result<Track> {
        debug!(target: "repository", track_id = %entity.id, "creating track (postgres)");

        let q = r#"
            INSERT INTO tracks (
                id, album_id, artist_id, foreign_track_id, title, track_number,
                duration_ms, has_file, monitored, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#;

        sqlx::query(q)
            .bind(entity.id.to_string())
            .bind(entity.album_id.to_string())
            .bind(entity.artist_id.to_string())
            .bind(entity.foreign_track_id.clone())
            .bind(entity.title.clone())
            .bind(entity.track_number.map(|n| n as i32))
            .bind(entity.duration_ms.map(|n| n as i32))
            .bind(entity.has_file)
            .bind(entity.monitored)
            .bind(entity.created_at.naive_utc())
            .bind(entity.updated_at.naive_utc())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Track>> {
        debug!(target: "repository", %id, "fetching track by id (postgres)");

        let row = sqlx::query("SELECT * FROM tracks WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_track(&r)).transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing tracks (postgres)");

        let rows =
            sqlx::query("SELECT * FROM tracks ORDER BY track_number, title LIMIT $1 OFFSET $2")
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_track(&row)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: Track) -> Result<Track> {
        debug!(target: "repository", track_id = %entity.id, "updating track (postgres)");

        let q = r#"
            UPDATE tracks SET
                album_id = $1,
                artist_id = $2,
                foreign_track_id = $3,
                title = $4,
                track_number = $5,
                duration_ms = $6,
                has_file = $7,
                monitored = $8,
                updated_at = $9
            WHERE id = $10
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
            .bind(entity.updated_at.naive_utc())
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting track (postgres)");

        let result = sqlx::query("DELETE FROM tracks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("track not found: {}", id));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl TrackRepository for PostgresTrackRepository {
    async fn get_by_album(&self, album_id: AlbumId, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", %album_id, limit, offset, "fetching tracks by album (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE album_id = $1 ORDER BY track_number, title LIMIT $2 OFFSET $3",
        )
        .bind(album_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_track(&row)?);
        }
        Ok(out)
    }

    async fn get_by_artist(
        &self,
        artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Track>> {
        debug!(target: "repository", %artist_id, limit, offset, "fetching tracks by artist (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE artist_id = $1 ORDER BY track_number, title LIMIT $2 OFFSET $3",
        )
        .bind(artist_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_track(&row)?);
        }
        Ok(out)
    }

    async fn get_by_foreign_id(&self, foreign_id: &str) -> Result<Option<Track>> {
        debug!(target: "repository", foreign_id, "fetching track by foreign_id (postgres)");

        let row = sqlx::query("SELECT * FROM tracks WHERE foreign_track_id = $1 LIMIT 1")
            .bind(foreign_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_track(&r)).transpose()?)
    }

    async fn list_monitored(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing monitored tracks (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE monitored = true ORDER BY track_number, title LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_track(&row)?);
        }
        Ok(out)
    }

    async fn list_without_files(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        debug!(target: "repository", limit, offset, "listing tracks without files (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE has_file = false ORDER BY track_number, title LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_track(&row)?);
        }
        Ok(out)
    }
}

fn row_to_track(row: &PgRow) -> Result<Track> {
    let id: String = row.try_get("id")?;
    let album_id: String = row.try_get("album_id")?;
    let artist_id: String = row.try_get("artist_id")?;
    let foreign_track_id: Option<String> = row.try_get("foreign_track_id")?;
    let title: String = row.try_get("title")?;
    let track_number: Option<i32> = row.try_get("track_number")?;
    let duration_ms: Option<i32> = row.try_get("duration_ms")?;
    let has_file: bool = row.try_get("has_file")?;
    let monitored: bool = row.try_get("monitored")?;
    let musicbrainz_recording_id: Option<String> = row.try_get("musicbrainz_recording_id")?;
    let match_confidence: Option<f64> = row.try_get("match_confidence")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    Ok(Track {
        id: TrackId::from_uuid(Uuid::parse_str(&id)?),
        album_id: AlbumId::from_uuid(Uuid::parse_str(&album_id)?),
        artist_id: ArtistId::from_uuid(Uuid::parse_str(&artist_id)?),
        foreign_track_id,
        title,
        track_number: track_number.map(|n| n as u32),
        duration_ms: duration_ms.map(|n| n as u32),
        has_file,
        monitored,
        musicbrainz_recording_id,
        match_confidence: match_confidence.map(|v| v as f32),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}

// ============================================================================
// PostgresQualityProfileRepository
// ============================================================================

#[async_trait::async_trait]
impl Repository<QualityProfile> for PostgresQualityProfileRepository {
    async fn create(&self, entity: QualityProfile) -> Result<QualityProfile> {
        debug!(target: "repository", profile_id = %entity.id, "creating quality profile (postgres)");

        let qualities_json = serde_json::to_string(&entity.allowed_qualities)?;

        sqlx::query(
            r#"
            INSERT INTO quality_profiles (
                id, name, allowed_qualities, upgrade_allowed, cutoff_quality, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(entity.id.to_string())
        .bind(entity.name.clone())
        .bind(qualities_json)
        .bind(entity.upgrade_allowed)
        .bind(entity.cutoff_quality.clone())
        .bind(entity.created_at.naive_utc())
        .bind(entity.updated_at.naive_utc())
        .execute(&self.pool)
        .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<QualityProfile>> {
        debug!(target: "repository", %id, "fetching quality profile by id (postgres)");

        let row = sqlx::query("SELECT * FROM quality_profiles WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_quality_profile(&r)).transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<QualityProfile>> {
        debug!(target: "repository", limit, offset, "listing quality profiles (postgres)");

        let rows = sqlx::query("SELECT * FROM quality_profiles ORDER BY name LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_quality_profile(&row)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: QualityProfile) -> Result<QualityProfile> {
        debug!(target: "repository", profile_id = %entity.id, "updating quality profile (postgres)");

        let qualities_json = serde_json::to_string(&entity.allowed_qualities)?;

        sqlx::query(
            r#"
            UPDATE quality_profiles SET
                name = $1,
                allowed_qualities = $2,
                upgrade_allowed = $3,
                cutoff_quality = $4,
                updated_at = $5
            WHERE id = $6
            "#,
        )
        .bind(entity.name.clone())
        .bind(qualities_json)
        .bind(entity.upgrade_allowed)
        .bind(entity.cutoff_quality.clone())
        .bind(entity.updated_at.naive_utc())
        .bind(entity.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting quality profile (postgres)");

        let result = sqlx::query("DELETE FROM quality_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("quality profile not found: {}", id));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl QualityProfileRepository for PostgresQualityProfileRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<QualityProfile>> {
        debug!(target: "repository", name, "fetching quality profile by name (postgres)");

        let row = sqlx::query("SELECT * FROM quality_profiles WHERE name = $1 LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_quality_profile(&r)).transpose()?)
    }
}

fn row_to_quality_profile(row: &PgRow) -> Result<QualityProfile> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let allowed_qualities_json: String = row.try_get("allowed_qualities")?;
    let upgrade_allowed: bool = row.try_get("upgrade_allowed")?;
    let cutoff_quality: Option<String> = row.try_get("cutoff_quality")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    let allowed_qualities: Vec<String> =
        serde_json::from_str(&allowed_qualities_json).unwrap_or_default();

    Ok(QualityProfile {
        id: ProfileId::from_uuid(Uuid::parse_str(&id)?),
        name,
        allowed_qualities,
        upgrade_allowed,
        cutoff_quality,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}

// ============================================================================
// PostgresMetadataProfileRepository
// ============================================================================

#[async_trait::async_trait]
impl Repository<MetadataProfile> for PostgresMetadataProfileRepository {
    async fn create(&self, entity: MetadataProfile) -> Result<MetadataProfile> {
        debug!(target: "repository", profile_id = %entity.id, "creating metadata profile (postgres)");

        let primary_json = serde_json::to_string(&entity.primary_album_types)?;
        let secondary_json = serde_json::to_string(&entity.secondary_album_types)?;
        let statuses_json = serde_json::to_string(&entity.release_statuses)?;

        sqlx::query(
            r#"
            INSERT INTO metadata_profiles (
                id, name, primary_album_types, secondary_album_types, release_statuses, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(entity.id.to_string())
        .bind(entity.name.clone())
        .bind(primary_json)
        .bind(secondary_json)
        .bind(statuses_json)
        .bind(entity.created_at.naive_utc())
        .bind(entity.updated_at.naive_utc())
        .execute(&self.pool)
        .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<MetadataProfile>> {
        debug!(target: "repository", %id, "fetching metadata profile by id (postgres)");

        let row = sqlx::query("SELECT * FROM metadata_profiles WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_metadata_profile(&r)).transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<MetadataProfile>> {
        debug!(target: "repository", limit, offset, "listing metadata profiles (postgres)");

        let rows = sqlx::query("SELECT * FROM metadata_profiles ORDER BY name LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_metadata_profile(&row)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: MetadataProfile) -> Result<MetadataProfile> {
        debug!(target: "repository", profile_id = %entity.id, "updating metadata profile (postgres)");

        let primary_json = serde_json::to_string(&entity.primary_album_types)?;
        let secondary_json = serde_json::to_string(&entity.secondary_album_types)?;
        let statuses_json = serde_json::to_string(&entity.release_statuses)?;

        sqlx::query(
            r#"
            UPDATE metadata_profiles SET
                name = $1,
                primary_album_types = $2,
                secondary_album_types = $3,
                release_statuses = $4,
                updated_at = $5
            WHERE id = $6
            "#,
        )
        .bind(entity.name.clone())
        .bind(primary_json)
        .bind(secondary_json)
        .bind(statuses_json)
        .bind(entity.updated_at.naive_utc())
        .bind(entity.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting metadata profile (postgres)");

        let result = sqlx::query("DELETE FROM metadata_profiles WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("metadata profile not found: {}", id));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl MetadataProfileRepository for PostgresMetadataProfileRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<MetadataProfile>> {
        debug!(target: "repository", name, "fetching metadata profile by name (postgres)");

        let row = sqlx::query("SELECT * FROM metadata_profiles WHERE name = $1 LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_metadata_profile(&r)).transpose()?)
    }
}

fn row_to_metadata_profile(row: &PgRow) -> Result<MetadataProfile> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let primary_json: Option<String> = row.try_get("primary_album_types")?;
    let secondary_json: Option<String> = row.try_get("secondary_album_types")?;
    let statuses_json: Option<String> = row.try_get("release_statuses")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    let primary_album_types = primary_json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();
    let secondary_album_types = secondary_json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();
    let release_statuses = statuses_json
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default();

    Ok(MetadataProfile {
        id: ProfileId::from_uuid(Uuid::parse_str(&id)?),
        name,
        primary_album_types,
        secondary_album_types,
        release_statuses,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}

// ============================================================================
// PostgresIndexerDefinitionRepository
// ============================================================================

#[async_trait::async_trait]
impl Repository<IndexerDefinition> for PostgresIndexerDefinitionRepository {
    async fn create(&self, entity: IndexerDefinition) -> Result<IndexerDefinition> {
        debug!(target: "repository", indexer_id = %entity.id, "creating indexer definition (postgres)");

        sqlx::query(
            r#"
            INSERT INTO indexer_definitions (
                id, name, base_url, protocol, api_key, enabled, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(entity.id.to_string())
        .bind(entity.name.clone())
        .bind(entity.base_url.clone())
        .bind(entity.protocol.clone())
        .bind(entity.api_key.clone())
        .bind(entity.enabled)
        .bind(entity.created_at.naive_utc())
        .bind(entity.updated_at.naive_utc())
        .execute(&self.pool)
        .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<IndexerDefinition>> {
        debug!(target: "repository", %id, "fetching indexer definition by id (postgres)");

        let row = sqlx::query("SELECT * FROM indexer_definitions WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_indexer_definition(&r)).transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<IndexerDefinition>> {
        debug!(target: "repository", limit, offset, "listing indexer definitions (postgres)");

        let rows =
            sqlx::query("SELECT * FROM indexer_definitions ORDER BY name LIMIT $1 OFFSET $2")
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_indexer_definition(&row)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: IndexerDefinition) -> Result<IndexerDefinition> {
        debug!(target: "repository", indexer_id = %entity.id, "updating indexer definition (postgres)");

        sqlx::query(
            r#"
            UPDATE indexer_definitions SET
                name = $1,
                base_url = $2,
                protocol = $3,
                api_key = $4,
                enabled = $5,
                updated_at = $6
            WHERE id = $7
            "#,
        )
        .bind(entity.name.clone())
        .bind(entity.base_url.clone())
        .bind(entity.protocol.clone())
        .bind(entity.api_key.clone())
        .bind(entity.enabled)
        .bind(entity.updated_at.naive_utc())
        .bind(entity.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting indexer definition (postgres)");

        let result = sqlx::query("DELETE FROM indexer_definitions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("indexer definition not found: {}", id));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl IndexerDefinitionRepository for PostgresIndexerDefinitionRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<IndexerDefinition>> {
        debug!(target: "repository", name, "fetching indexer definition by name (postgres)");

        let row = sqlx::query("SELECT * FROM indexer_definitions WHERE name = $1 LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_indexer_definition(&r)).transpose()?)
    }
}

fn row_to_indexer_definition(row: &PgRow) -> Result<IndexerDefinition> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let base_url: String = row.try_get("base_url")?;
    let protocol: String = row.try_get("protocol")?;
    let api_key: Option<String> = row.try_get("api_key")?;
    let enabled: bool = row.try_get("enabled")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    Ok(IndexerDefinition {
        id: IndexerDefinitionId::from_uuid(Uuid::parse_str(&id)?),
        name,
        base_url,
        protocol,
        api_key,
        enabled,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}

// ============================================================================
// PostgresDownloadClientDefinitionRepository
// ============================================================================

#[async_trait::async_trait]
impl Repository<DownloadClientDefinition> for PostgresDownloadClientDefinitionRepository {
    async fn create(&self, entity: DownloadClientDefinition) -> Result<DownloadClientDefinition> {
        debug!(target: "repository", client_id = %entity.id, "creating download client definition (postgres)");

        sqlx::query(
            r#"
            INSERT INTO download_client_definitions (
                id, name, client_type, base_url, username, password_encrypted, category, enabled, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(entity.id.to_string())
        .bind(entity.name.clone())
        .bind(entity.client_type.clone())
        .bind(entity.base_url.clone())
        .bind(entity.username.clone())
        .bind(entity.password_encrypted.clone())
        .bind(entity.category.clone())
        .bind(entity.enabled)
        .bind(entity.created_at.naive_utc())
        .bind(entity.updated_at.naive_utc())
        .execute(&self.pool)
        .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<DownloadClientDefinition>> {
        debug!(target: "repository", %id, "fetching download client definition by id (postgres)");

        let row = sqlx::query("SELECT * FROM download_client_definitions WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row
            .map(|r| row_to_download_client_definition(&r))
            .transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<DownloadClientDefinition>> {
        debug!(target: "repository", limit, offset, "listing download client definitions (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM download_client_definitions ORDER BY name LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_download_client_definition(&row)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: DownloadClientDefinition) -> Result<DownloadClientDefinition> {
        debug!(target: "repository", client_id = %entity.id, "updating download client definition (postgres)");

        sqlx::query(
            r#"
            UPDATE download_client_definitions SET
                name = $1,
                client_type = $2,
                base_url = $3,
                username = $4,
                password_encrypted = $5,
                category = $6,
                enabled = $7,
                updated_at = $8
            WHERE id = $9
            "#,
        )
        .bind(entity.name.clone())
        .bind(entity.client_type.clone())
        .bind(entity.base_url.clone())
        .bind(entity.username.clone())
        .bind(entity.password_encrypted.clone())
        .bind(entity.category.clone())
        .bind(entity.enabled)
        .bind(entity.updated_at.naive_utc())
        .bind(entity.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting download client definition (postgres)");

        let result = sqlx::query("DELETE FROM download_client_definitions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("download client definition not found: {}", id));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl DownloadClientDefinitionRepository for PostgresDownloadClientDefinitionRepository {
    async fn get_by_name(&self, name: &str) -> Result<Option<DownloadClientDefinition>> {
        debug!(target: "repository", name, "fetching download client definition by name (postgres)");

        let row = sqlx::query("SELECT * FROM download_client_definitions WHERE name = $1 LIMIT 1")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row
            .map(|r| row_to_download_client_definition(&r))
            .transpose()?)
    }
}

fn row_to_download_client_definition(row: &PgRow) -> Result<DownloadClientDefinition> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let client_type: String = row.try_get("client_type")?;
    let base_url: String = row.try_get("base_url")?;
    let username: Option<String> = row.try_get("username")?;
    let password_encrypted: Option<String> = row.try_get("password_encrypted")?;
    let category: Option<String> = row.try_get("category")?;
    let enabled: bool = row.try_get("enabled")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    Ok(DownloadClientDefinition {
        id: DownloadClientDefinitionId::from_uuid(Uuid::parse_str(&id)?),
        name,
        client_type,
        base_url,
        username,
        password_encrypted,
        category,
        enabled,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}

// ============================================================================
// PostgresTrackFileRepository
// ============================================================================

#[async_trait::async_trait]
impl Repository<TrackFile> for PostgresTrackFileRepository {
    async fn create(&self, entity: TrackFile) -> Result<TrackFile> {
        debug!(target: "repository", track_file_id = %entity.id, "creating track file (postgres)");

        let q = r#"
            INSERT INTO track_files (
                id, track_id, path, size_bytes, duration_ms, bitrate_kbps,
                channels, codec, quality, hash, fingerprint_hash, fingerprint_duration,
                fingerprint_computed_at, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#;

        let fingerprint_computed_at = entity.fingerprint_computed_at.map(|dt| dt.naive_utc());

        sqlx::query(q)
            .bind(entity.id.to_string())
            .bind(entity.track_id.to_string())
            .bind(entity.path.clone())
            .bind(entity.size_bytes as i64)
            .bind(entity.duration_ms.map(|d| d as i32))
            .bind(entity.bitrate_kbps.map(|b| b as i32))
            .bind(entity.channels.map(|c| c as i16))
            .bind(entity.codec.clone())
            .bind(entity.quality.clone())
            .bind(entity.hash.clone())
            .bind(entity.fingerprint_hash.clone())
            .bind(entity.fingerprint_duration.map(|d| d as i32))
            .bind(fingerprint_computed_at)
            .bind(entity.created_at.naive_utc())
            .bind(entity.updated_at.naive_utc())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<TrackFile>> {
        debug!(target: "repository", %id, "fetching track file by id (postgres)");

        let row = sqlx::query("SELECT * FROM track_files WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_track_file(&r)).transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<TrackFile>> {
        debug!(target: "repository", limit, offset, "listing track files (postgres)");

        let rows =
            sqlx::query("SELECT * FROM track_files ORDER BY created_at DESC LIMIT $1 OFFSET $2")
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;

        rows.iter().map(row_to_track_file).collect()
    }

    async fn update(&self, entity: TrackFile) -> Result<TrackFile> {
        debug!(target: "repository", track_file_id = %entity.id, "updating track file (postgres)");

        let q = r#"
            UPDATE track_files SET
                path = $1, size_bytes = $2, duration_ms = $3, bitrate_kbps = $4,
                channels = $5, codec = $6, quality = $7, hash = $8, fingerprint_hash = $9,
                fingerprint_duration = $10, fingerprint_computed_at = $11, updated_at = $12
            WHERE id = $13
        "#;

        let fingerprint_computed_at = entity.fingerprint_computed_at.map(|dt| dt.naive_utc());

        sqlx::query(q)
            .bind(entity.path.clone())
            .bind(entity.size_bytes as i64)
            .bind(entity.duration_ms.map(|d| d as i32))
            .bind(entity.bitrate_kbps.map(|b| b as i32))
            .bind(entity.channels.map(|c| c as i16))
            .bind(entity.codec.clone())
            .bind(entity.quality.clone())
            .bind(entity.hash.clone())
            .bind(entity.fingerprint_hash.clone())
            .bind(entity.fingerprint_duration.map(|d| d as i32))
            .bind(fingerprint_computed_at)
            .bind(entity.updated_at.naive_utc())
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting track file (postgres)");

        sqlx::query("DELETE FROM track_files WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl TrackFileRepository for PostgresTrackFileRepository {
    async fn get_by_track(
        &self,
        track_id: TrackId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TrackFile>> {
        debug!(target: "repository", %track_id, limit, offset, "fetching track files by track (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM track_files WHERE track_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(track_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_track_file).collect()
    }

    async fn get_by_path(&self, path: &str) -> Result<Option<TrackFile>> {
        debug!(target: "repository", path, "fetching track file by path (postgres)");

        let row = sqlx::query("SELECT * FROM track_files WHERE path = $1 LIMIT 1")
            .bind(path)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_track_file(&r)).transpose()?)
    }

    async fn list_with_fingerprints(&self, limit: i64, offset: i64) -> Result<Vec<TrackFile>> {
        debug!(target: "repository", limit, offset, "listing track files with fingerprints (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM track_files WHERE fingerprint_hash IS NOT NULL ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_track_file).collect()
    }

    async fn list_without_fingerprints(&self, limit: i64, offset: i64) -> Result<Vec<TrackFile>> {
        debug!(target: "repository", limit, offset, "listing track files without fingerprints (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM track_files WHERE fingerprint_hash IS NULL ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_track_file).collect()
    }
}

fn row_to_track_file(row: &PgRow) -> Result<TrackFile> {
    let id: String = row.try_get("id")?;
    let track_id: String = row.try_get("track_id")?;
    let path: String = row.try_get("path")?;
    let size_bytes: i64 = row.try_get("size_bytes")?;
    let duration_ms: Option<i32> = row.try_get("duration_ms")?;
    let bitrate_kbps: Option<i32> = row.try_get("bitrate_kbps")?;
    let channels: Option<i16> = row.try_get("channels")?;
    let codec: Option<String> = row.try_get("codec")?;
    let quality: Option<String> = row.try_get("quality")?;
    let hash: Option<String> = row.try_get("hash")?;
    let fingerprint_hash: Option<String> = row.try_get("fingerprint_hash")?;
    let fingerprint_duration: Option<i32> = row.try_get("fingerprint_duration")?;
    let fingerprint_computed_at: Option<NaiveDateTime> = row.try_get("fingerprint_computed_at")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    Ok(TrackFile {
        id: TrackFileId(Uuid::parse_str(&id)?),
        track_id: TrackId(Uuid::parse_str(&track_id)?),
        path,
        size_bytes: size_bytes as u64,
        duration_ms: duration_ms.map(|d| d as u32),
        bitrate_kbps: bitrate_kbps.map(|b| b as u32),
        channels: channels.map(|c| c as u8),
        codec,
        quality,
        hash,
        fingerprint_hash,
        fingerprint_duration: fingerprint_duration.map(|d| d as u32),
        fingerprint_computed_at: fingerprint_computed_at
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}

// ============================================================================
// PostgresArtistRelationshipRepository
// ============================================================================

#[async_trait::async_trait]
impl Repository<ArtistRelationship> for PostgresArtistRelationshipRepository {
    async fn create(&self, entity: ArtistRelationship) -> Result<ArtistRelationship> {
        debug!(target: "repository", relationship_id = %entity.id, "creating artist relationship (postgres)");

        let q = r#"
            INSERT INTO artist_relationships (
                id, source_artist_id, related_artist_id, relationship_type, description,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#;

        sqlx::query(q)
            .bind(entity.id.to_string())
            .bind(entity.source_artist_id.to_string())
            .bind(entity.related_artist_id.to_string())
            .bind(entity.relationship_type.clone())
            .bind(entity.description.clone())
            .bind(entity.created_at.naive_utc())
            .bind(entity.updated_at.naive_utc())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<ArtistRelationship>> {
        debug!(target: "repository", %id, "fetching artist relationship by id (postgres)");

        let row = sqlx::query("SELECT * FROM artist_relationships WHERE id = $1 LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| row_to_artist_relationship(&r)).transpose()?)
    }

    async fn list(&self, limit: i64, offset: i64) -> Result<Vec<ArtistRelationship>> {
        debug!(target: "repository", limit, offset, "listing artist relationships (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM artist_relationships ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_artist_relationship(&row)?);
        }
        Ok(out)
    }

    async fn update(&self, entity: ArtistRelationship) -> Result<ArtistRelationship> {
        debug!(target: "repository", relationship_id = %entity.id, "updating artist relationship (postgres)");

        let q = r#"
            UPDATE artist_relationships SET
                source_artist_id = $1,
                related_artist_id = $2,
                relationship_type = $3,
                description = $4,
                updated_at = $5
            WHERE id = $6
        "#;

        sqlx::query(q)
            .bind(entity.source_artist_id.to_string())
            .bind(entity.related_artist_id.to_string())
            .bind(entity.relationship_type.clone())
            .bind(entity.description.clone())
            .bind(entity.updated_at.naive_utc())
            .bind(entity.id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(entity)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        debug!(target: "repository", %id, "deleting artist relationship (postgres)");

        sqlx::query("DELETE FROM artist_relationships WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ArtistRelationshipRepository for PostgresArtistRelationshipRepository {
    async fn get_by_source_artist(
        &self,
        source_artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArtistRelationship>> {
        debug!(target: "repository", %source_artist_id, limit, offset, "fetching relationships by source artist (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM artist_relationships WHERE source_artist_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(source_artist_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_artist_relationship(&row)?);
        }
        Ok(out)
    }

    async fn get_by_related_artist(
        &self,
        related_artist_id: ArtistId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArtistRelationship>> {
        debug!(target: "repository", %related_artist_id, limit, offset, "fetching relationships by related artist (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM artist_relationships WHERE related_artist_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(related_artist_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_artist_relationship(&row)?);
        }
        Ok(out)
    }

    async fn get_by_type_and_source(
        &self,
        source_artist_id: ArtistId,
        relationship_type: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArtistRelationship>> {
        debug!(target: "repository", %source_artist_id, relationship_type, limit, offset, "fetching relationships by type and source (postgres)");

        let rows = sqlx::query(
            "SELECT * FROM artist_relationships WHERE source_artist_id = $1 AND relationship_type = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4",
        )
        .bind(source_artist_id.to_string())
        .bind(relationship_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(row_to_artist_relationship(&row)?);
        }
        Ok(out)
    }

    async fn relationship_exists(
        &self,
        source_artist_id: ArtistId,
        related_artist_id: ArtistId,
        relationship_type: &str,
    ) -> Result<bool> {
        debug!(target: "repository", %source_artist_id, %related_artist_id, relationship_type, "checking relationship existence (postgres)");

        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM artist_relationships WHERE source_artist_id = $1 AND related_artist_id = $2 AND relationship_type = $3",
        )
        .bind(source_artist_id.to_string())
        .bind(related_artist_id.to_string())
        .bind(relationship_type)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = row.try_get("count")?;
        Ok(count > 0)
    }
}

fn row_to_artist_relationship(row: &PgRow) -> Result<ArtistRelationship> {
    let id: String = row.try_get("id")?;
    let source_artist_id: String = row.try_get("source_artist_id")?;
    let related_artist_id: String = row.try_get("related_artist_id")?;
    let relationship_type: String = row.try_get("relationship_type")?;
    let description: Option<String> = row.try_get("description")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    let updated_at: NaiveDateTime = row.try_get("updated_at")?;

    Ok(ArtistRelationship {
        id: ArtistRelationshipId::from_uuid(Uuid::parse_str(&id)?),
        source_artist_id: ArtistId::from_uuid(Uuid::parse_str(&source_artist_id)?),
        related_artist_id: ArtistId::from_uuid(Uuid::parse_str(&related_artist_id)?),
        relationship_type,
        description,
        created_at: DateTime::<Utc>::from_naive_utc_and_offset(created_at, Utc),
        updated_at: DateTime::<Utc>::from_naive_utc_and_offset(updated_at, Utc),
    })
}
