// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "postgres")]

use anyhow::{anyhow, Result};
use chorrosion_domain::{Artist, ArtistId, ArtistStatus};
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::PgPool;
use sqlx::Row;
use tracing::debug;
use uuid::Uuid;

use crate::repositories::{ArtistRepository, Repository};

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
