// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "postgres")]

use anyhow::{anyhow, Result};
use chrono::{NaiveDate, NaiveDateTime};
use sqlx::{FromRow, PgPool, SqlitePool};

const MIGRATED_TABLES: [&str; 9] = [
    "quality_profiles",
    "metadata_profiles",
    "artists",
    "albums",
    "tracks",
    "track_files",
    "artist_relationships",
    "indexer_definitions",
    "download_client_definitions",
];
const DEFAULT_BATCH_SIZE: i64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMigrationSummary {
    pub table: &'static str,
    pub sqlite_rows: i64,
    pub postgres_rows: i64,
}

#[derive(Debug, Clone, Default)]
pub struct MigrationReport {
    pub tables: Vec<TableMigrationSummary>,
}

impl MigrationReport {
    pub fn mismatched_tables(&self) -> Vec<&TableMigrationSummary> {
        self.tables
            .iter()
            .filter(|summary| summary.sqlite_rows != summary.postgres_rows)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetResetPolicy {
    #[default]
    RejectNonEmpty,
    TruncateAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationOptions {
    pub target_reset_policy: TargetResetPolicy,
    pub sqlite_batch_size: i64,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            target_reset_policy: TargetResetPolicy::RejectNonEmpty,
            sqlite_batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

pub async fn plan_sqlite_to_postgres_migration(
    sqlite_pool: &SqlitePool,
    postgres_pool: &PgPool,
) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();

    for table in MIGRATED_TABLES {
        report.tables.push(TableMigrationSummary {
            table,
            sqlite_rows: count_rows_sqlite(sqlite_pool, table).await?,
            postgres_rows: count_rows_postgres(postgres_pool, table).await?,
        });
    }

    Ok(report)
}

/// Migrates core tables from SQLite to PostgreSQL.
///
/// By default, this migration is non-destructive and returns an error when
/// the target contains rows in migrated tables. To explicitly wipe target
/// tables before migration, call `migrate_sqlite_to_postgres_with_options`
/// and set `TargetResetPolicy::TruncateAll`.
pub async fn migrate_sqlite_to_postgres(
    sqlite_pool: &SqlitePool,
    postgres_pool: &PgPool,
) -> Result<MigrationReport> {
    migrate_sqlite_to_postgres_with_options(sqlite_pool, postgres_pool, MigrationOptions::default())
        .await
}

pub async fn migrate_sqlite_to_postgres_with_options(
    sqlite_pool: &SqlitePool,
    postgres_pool: &PgPool,
    options: MigrationOptions,
) -> Result<MigrationReport> {
    if options.sqlite_batch_size <= 0 {
        return Err(anyhow!("sqlite_batch_size must be greater than zero"));
    }

    let sqlite_report = report_sqlite_counts(sqlite_pool).await?;
    let mut tx = postgres_pool.begin().await?;

    match options.target_reset_policy {
        TargetResetPolicy::RejectNonEmpty => {
            let existing = report_postgres_counts_in_tx(&mut tx).await?;
            let non_empty = existing
                .tables
                .iter()
                .filter(|summary| summary.postgres_rows > 0)
                .map(|summary| summary.table)
                .collect::<Vec<_>>();
            if !non_empty.is_empty() {
                return Err(anyhow!(
                    "target PostgreSQL tables are not empty ({}); rerun with TargetResetPolicy::TruncateAll to replace existing data",
                    non_empty.join(", ")
                ));
            }
        }
        TargetResetPolicy::TruncateAll => {
            sqlx::query(
                "TRUNCATE TABLE track_files, tracks, artist_relationships, albums, artists, quality_profiles, metadata_profiles, indexer_definitions, download_client_definitions RESTART IDENTITY CASCADE",
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let quality_profiles = sqlx::query_as::<_, QualityProfileRow>(
            "SELECT id, name, allowed_qualities, upgrade_allowed, cutoff_quality, created_at, updated_at FROM quality_profiles ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if quality_profiles.is_empty() {
            break;
        }
        offset += quality_profiles.len() as i64;

        for row in &quality_profiles {
            sqlx::query(
                "INSERT INTO quality_profiles (id, name, allowed_qualities, upgrade_allowed, cutoff_quality, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&row.id)
            .bind(&row.name)
            .bind(&row.allowed_qualities)
            .bind(row.upgrade_allowed)
            .bind(&row.cutoff_quality)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let metadata_profiles = sqlx::query_as::<_, MetadataProfileRow>(
            "SELECT id, name, primary_album_types, secondary_album_types, release_statuses, created_at, updated_at FROM metadata_profiles ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if metadata_profiles.is_empty() {
            break;
        }
        offset += metadata_profiles.len() as i64;

        for row in &metadata_profiles {
            sqlx::query(
                "INSERT INTO metadata_profiles (id, name, primary_album_types, secondary_album_types, release_statuses, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&row.id)
            .bind(&row.name)
            .bind(&row.primary_album_types)
            .bind(&row.secondary_album_types)
            .bind(&row.release_statuses)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let artists = sqlx::query_as::<_, ArtistRow>(
            "SELECT id, name, foreign_artist_id, musicbrainz_artist_id, metadata_profile_id, quality_profile_id, status, path, monitored, artist_type, sort_name, country, disambiguation, genre_tags, style_tags, created_at, updated_at FROM artists ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if artists.is_empty() {
            break;
        }
        offset += artists.len() as i64;

        for row in &artists {
            sqlx::query(
                "INSERT INTO artists (id, name, foreign_artist_id, musicbrainz_artist_id, metadata_profile_id, quality_profile_id, status, path, monitored, artist_type, sort_name, country, disambiguation, genre_tags, style_tags, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
            )
            .bind(&row.id)
            .bind(&row.name)
            .bind(&row.foreign_artist_id)
            .bind(&row.musicbrainz_artist_id)
            .bind(&row.metadata_profile_id)
            .bind(&row.quality_profile_id)
            .bind(&row.status)
            .bind(&row.path)
            .bind(row.monitored)
            .bind(&row.artist_type)
            .bind(&row.sort_name)
            .bind(&row.country)
            .bind(&row.disambiguation)
            .bind(&row.genre_tags)
            .bind(&row.style_tags)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let albums = sqlx::query_as::<_, AlbumRow>(
            "SELECT id, artist_id, foreign_album_id, title, release_date, album_type, status, monitored, musicbrainz_release_group_id, musicbrainz_release_id, primary_type, secondary_types, first_release_date, genre_tags, style_tags, created_at, updated_at FROM albums ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if albums.is_empty() {
            break;
        }
        offset += albums.len() as i64;

        for row in &albums {
            sqlx::query(
                "INSERT INTO albums (id, artist_id, foreign_album_id, title, release_date, album_type, status, monitored, musicbrainz_release_group_id, musicbrainz_release_id, primary_type, secondary_types, first_release_date, genre_tags, style_tags, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
            )
            .bind(&row.id)
            .bind(&row.artist_id)
            .bind(&row.foreign_album_id)
            .bind(&row.title)
            .bind(row.release_date)
            .bind(&row.album_type)
            .bind(&row.status)
            .bind(row.monitored)
            .bind(&row.musicbrainz_release_group_id)
            .bind(&row.musicbrainz_release_id)
            .bind(&row.primary_type)
            .bind(&row.secondary_types)
            .bind(&row.first_release_date)
            .bind(&row.genre_tags)
            .bind(&row.style_tags)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let tracks = sqlx::query_as::<_, TrackRow>(
            "SELECT id, album_id, artist_id, foreign_track_id, title, track_number, duration_ms, has_file, monitored, musicbrainz_recording_id, match_confidence, created_at, updated_at FROM tracks ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if tracks.is_empty() {
            break;
        }
        offset += tracks.len() as i64;

        for row in &tracks {
            sqlx::query(
                "INSERT INTO tracks (id, album_id, artist_id, foreign_track_id, title, track_number, duration_ms, has_file, monitored, musicbrainz_recording_id, match_confidence, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            )
            .bind(&row.id)
            .bind(&row.album_id)
            .bind(&row.artist_id)
            .bind(&row.foreign_track_id)
            .bind(&row.title)
            .bind(row.track_number)
            .bind(row.duration_ms)
            .bind(row.has_file)
            .bind(row.monitored)
            .bind(&row.musicbrainz_recording_id)
            .bind(row.match_confidence)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let track_files = sqlx::query_as::<_, TrackFileRow>(
            "SELECT id, track_id, path, size_bytes, duration_ms, bitrate_kbps, channels, codec, hash, fingerprint_hash, fingerprint_duration, fingerprint_computed_at, quality, created_at, updated_at FROM track_files ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if track_files.is_empty() {
            break;
        }
        offset += track_files.len() as i64;

        for row in &track_files {
            sqlx::query(
                "INSERT INTO track_files (id, track_id, path, size_bytes, duration_ms, bitrate_kbps, channels, codec, hash, fingerprint_hash, fingerprint_duration, fingerprint_computed_at, quality, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
            )
            .bind(&row.id)
            .bind(&row.track_id)
            .bind(&row.path)
            .bind(row.size_bytes)
            .bind(row.duration_ms)
            .bind(row.bitrate_kbps)
            .bind(row.channels)
            .bind(&row.codec)
            .bind(&row.hash)
            .bind(&row.fingerprint_hash)
            .bind(row.fingerprint_duration)
            .bind(row.fingerprint_computed_at)
            .bind(&row.quality)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let artist_relationships = sqlx::query_as::<_, ArtistRelationshipRow>(
            "SELECT id, source_artist_id, related_artist_id, relationship_type, description, created_at, updated_at FROM artist_relationships ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if artist_relationships.is_empty() {
            break;
        }
        offset += artist_relationships.len() as i64;

        for row in &artist_relationships {
            sqlx::query(
                "INSERT INTO artist_relationships (id, source_artist_id, related_artist_id, relationship_type, description, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&row.id)
            .bind(&row.source_artist_id)
            .bind(&row.related_artist_id)
            .bind(&row.relationship_type)
            .bind(&row.description)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let indexers = sqlx::query_as::<_, IndexerDefinitionRow>(
            "SELECT id, name, base_url, protocol, api_key, enabled, created_at, updated_at FROM indexer_definitions ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if indexers.is_empty() {
            break;
        }
        offset += indexers.len() as i64;

        for row in &indexers {
            sqlx::query(
                "INSERT INTO indexer_definitions (id, name, base_url, protocol, api_key, enabled, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(&row.id)
            .bind(&row.name)
            .bind(&row.base_url)
            .bind(&row.protocol)
            .bind(&row.api_key)
            .bind(row.enabled)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let mut offset = 0;
    loop {
        let download_clients = sqlx::query_as::<_, DownloadClientDefinitionRow>(
            "SELECT id, name, client_type, base_url, username, password_encrypted, category, enabled, created_at, updated_at FROM download_client_definitions ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(options.sqlite_batch_size)
        .bind(offset)
        .fetch_all(sqlite_pool)
        .await?;
        if download_clients.is_empty() {
            break;
        }
        offset += download_clients.len() as i64;

        for row in &download_clients {
            sqlx::query(
                "INSERT INTO download_client_definitions (id, name, client_type, base_url, username, password_encrypted, category, enabled, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(&row.id)
            .bind(&row.name)
            .bind(&row.client_type)
            .bind(&row.base_url)
            .bind(&row.username)
            .bind(&row.password_encrypted)
            .bind(&row.category)
            .bind(row.enabled)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&mut *tx)
            .await?;
        }
    }

    let report = report_postgres_counts_with_sqlite_baseline_in_tx(&mut tx, &sqlite_report).await?;
    let mismatches = report.mismatched_tables();
    if !mismatches.is_empty() {
        let details = mismatches
            .iter()
            .map(|item| {
                format!(
                    "{} (sqlite={}, postgres={})",
                    item.table, item.sqlite_rows, item.postgres_rows
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(anyhow!(
            "sqlite->postgres row-count validation failed: {}",
            details
        ));
    }

    tx.commit().await?;

    Ok(report)
}

async fn report_sqlite_counts(sqlite_pool: &SqlitePool) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();
    for table in MIGRATED_TABLES {
        report.tables.push(TableMigrationSummary {
            table,
            sqlite_rows: count_rows_sqlite(sqlite_pool, table).await?,
            postgres_rows: 0,
        });
    }
    Ok(report)
}

async fn report_postgres_counts_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();
    for table in MIGRATED_TABLES {
        report.tables.push(TableMigrationSummary {
            table,
            sqlite_rows: 0,
            postgres_rows: count_rows_postgres_in_tx(tx, table).await?,
        });
    }
    Ok(report)
}

async fn report_postgres_counts_with_sqlite_baseline_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    sqlite_report: &MigrationReport,
) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();
    for summary in &sqlite_report.tables {
        report.tables.push(TableMigrationSummary {
            table: summary.table,
            sqlite_rows: summary.sqlite_rows,
            postgres_rows: count_rows_postgres_in_tx(tx, summary.table).await?,
        });
    }
    Ok(report)
}

async fn count_rows_sqlite(pool: &SqlitePool, table: &str) -> Result<i64> {
    ensure_migrated_table(table)?;
    let query = format!("SELECT COUNT(*) FROM {table}");
    Ok(sqlx::query_scalar::<_, i64>(&query).fetch_one(pool).await?)
}

async fn count_rows_postgres(pool: &PgPool, table: &str) -> Result<i64> {
    ensure_migrated_table(table)?;
    let query = format!("SELECT COUNT(*) FROM {table}");
    Ok(sqlx::query_scalar::<_, i64>(&query).fetch_one(pool).await?)
}

async fn count_rows_postgres_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &str,
) -> Result<i64> {
    ensure_migrated_table(table)?;
    let query = format!("SELECT COUNT(*) FROM {table}");
    Ok(sqlx::query_scalar::<_, i64>(&query)
        .fetch_one(&mut **tx)
        .await?)
}

fn ensure_migrated_table(table: &str) -> Result<()> {
    if MIGRATED_TABLES.contains(&table) {
        Ok(())
    } else {
        Err(anyhow!("unsupported migration table: {table}"))
    }
}

#[derive(Debug, Clone, FromRow)]
struct QualityProfileRow {
    id: String,
    name: String,
    allowed_qualities: String,
    upgrade_allowed: bool,
    cutoff_quality: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct MetadataProfileRow {
    id: String,
    name: String,
    primary_album_types: Option<String>,
    secondary_album_types: Option<String>,
    release_statuses: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct ArtistRow {
    id: String,
    name: String,
    foreign_artist_id: Option<String>,
    musicbrainz_artist_id: Option<String>,
    metadata_profile_id: Option<String>,
    quality_profile_id: Option<String>,
    status: String,
    path: Option<String>,
    monitored: bool,
    artist_type: Option<String>,
    sort_name: Option<String>,
    country: Option<String>,
    disambiguation: Option<String>,
    genre_tags: Option<String>,
    style_tags: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct AlbumRow {
    id: String,
    artist_id: String,
    foreign_album_id: Option<String>,
    title: String,
    release_date: Option<NaiveDate>,
    album_type: Option<String>,
    status: String,
    monitored: bool,
    musicbrainz_release_group_id: Option<String>,
    musicbrainz_release_id: Option<String>,
    primary_type: Option<String>,
    secondary_types: Option<String>,
    first_release_date: Option<String>,
    genre_tags: Option<String>,
    style_tags: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct TrackRow {
    id: String,
    album_id: String,
    artist_id: String,
    foreign_track_id: Option<String>,
    title: String,
    track_number: Option<i64>,
    duration_ms: Option<i64>,
    has_file: bool,
    monitored: bool,
    musicbrainz_recording_id: Option<String>,
    match_confidence: Option<f64>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct TrackFileRow {
    id: String,
    track_id: String,
    path: String,
    size_bytes: i64,
    duration_ms: Option<i64>,
    bitrate_kbps: Option<i64>,
    channels: Option<i64>,
    codec: Option<String>,
    hash: Option<String>,
    fingerprint_hash: Option<String>,
    fingerprint_duration: Option<i64>,
    fingerprint_computed_at: Option<NaiveDateTime>,
    quality: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct ArtistRelationshipRow {
    id: String,
    source_artist_id: String,
    related_artist_id: String,
    relationship_type: String,
    description: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct IndexerDefinitionRow {
    id: String,
    name: String,
    base_url: String,
    protocol: String,
    api_key: Option<String>,
    enabled: bool,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct DownloadClientDefinitionRow {
    id: String,
    name: String,
    client_type: String,
    base_url: String,
    username: Option<String>,
    password_encrypted: Option<String>,
    category: Option<String>,
    enabled: bool,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_mismatched_tables_filters_equal_rows() {
        let report = MigrationReport {
            tables: vec![
                TableMigrationSummary {
                    table: "artists",
                    sqlite_rows: 10,
                    postgres_rows: 10,
                },
                TableMigrationSummary {
                    table: "albums",
                    sqlite_rows: 8,
                    postgres_rows: 7,
                },
            ],
        };

        let mismatches = report.mismatched_tables();
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].table, "albums");
    }
}
