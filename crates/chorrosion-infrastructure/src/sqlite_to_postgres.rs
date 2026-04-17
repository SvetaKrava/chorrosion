// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "postgres")]

use anyhow::{anyhow, Result};
use chrono::{NaiveDate, NaiveDateTime};
use sqlx::{FromRow, PgPool, SqlitePool};
use std::collections::{HashSet, VecDeque};

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
const MAX_ID_SAMPLES_PER_TABLE: usize = 25;

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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SchemaComparisonReport {
    pub missing_tables_in_postgres: Vec<&'static str>,
    pub missing_tables_in_sqlite: Vec<&'static str>,
    pub missing_columns_in_postgres: Vec<TableColumnDifference>,
    pub missing_columns_in_sqlite: Vec<TableColumnDifference>,
}

impl SchemaComparisonReport {
    pub fn has_differences(&self) -> bool {
        !self.missing_tables_in_postgres.is_empty()
            || !self.missing_tables_in_sqlite.is_empty()
            || !self.missing_columns_in_postgres.is_empty()
            || !self.missing_columns_in_sqlite.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnDifference {
    pub table: &'static str,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DataValidationReport {
    pub table_id_differences: Vec<TableIdDifference>,
    pub referential_integrity: Vec<ReferentialIntegrityCheck>,
}

impl DataValidationReport {
    pub fn has_issues(&self) -> bool {
        !self.table_id_differences.is_empty()
            || self
                .referential_integrity
                .iter()
                .any(|check| check.orphan_count > 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableIdDifference {
    pub table: &'static str,
    pub missing_ids_in_postgres: Vec<String>,
    pub unexpected_ids_in_postgres: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferentialIntegrityCheck {
    pub name: &'static str,
    pub orphan_count: i64,
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

pub async fn compare_sqlite_postgres_schema(
    sqlite_pool: &SqlitePool,
    postgres_pool: &PgPool,
) -> Result<SchemaComparisonReport> {
    let mut report = SchemaComparisonReport::default();
    let postgres_schema = postgres_current_schema(postgres_pool).await?;

    for table in MIGRATED_TABLES {
        let sqlite_exists = sqlite_table_exists(sqlite_pool, table).await?;
        let postgres_exists = postgres_table_exists(postgres_pool, &postgres_schema, table).await?;

        if sqlite_exists && !postgres_exists {
            report.missing_tables_in_postgres.push(table);
            continue;
        }

        if !sqlite_exists && postgres_exists {
            report.missing_tables_in_sqlite.push(table);
            continue;
        }

        if !sqlite_exists && !postgres_exists {
            report.missing_tables_in_postgres.push(table);
            report.missing_tables_in_sqlite.push(table);
            continue;
        }

        let sqlite_columns = sqlite_columns_for_table(sqlite_pool, table).await?;
        let postgres_columns =
            postgres_columns_for_table(postgres_pool, &postgres_schema, table).await?;

        let missing_in_postgres = sqlite_columns
            .difference(&postgres_columns)
            .cloned()
            .collect::<Vec<_>>();
        if !missing_in_postgres.is_empty() {
            report
                .missing_columns_in_postgres
                .push(TableColumnDifference {
                    table,
                    columns: sorted_columns(missing_in_postgres),
                });
        }

        let missing_in_sqlite = postgres_columns
            .difference(&sqlite_columns)
            .cloned()
            .collect::<Vec<_>>();
        if !missing_in_sqlite.is_empty() {
            report
                .missing_columns_in_sqlite
                .push(TableColumnDifference {
                    table,
                    columns: sorted_columns(missing_in_sqlite),
                });
        }
    }

    report.missing_tables_in_postgres.sort_unstable();
    report.missing_tables_in_sqlite.sort_unstable();
    report
        .missing_columns_in_postgres
        .sort_by(|a, b| a.table.cmp(b.table));
    report
        .missing_columns_in_sqlite
        .sort_by(|a, b| a.table.cmp(b.table));

    Ok(report)
}

pub async fn validate_sqlite_postgres_data(
    sqlite_pool: &SqlitePool,
    postgres_pool: &PgPool,
) -> Result<DataValidationReport> {
    let mut report = DataValidationReport::default();

    for table in MIGRATED_TABLES {
        let (missing_ids_in_postgres, unexpected_ids_in_postgres) =
            id_difference_samples_for_table(sqlite_pool, postgres_pool, table).await?;

        if !missing_ids_in_postgres.is_empty() || !unexpected_ids_in_postgres.is_empty() {
            report.table_id_differences.push(TableIdDifference {
                table,
                missing_ids_in_postgres,
                unexpected_ids_in_postgres,
            });
        }
    }

    let fk_checks: [(&str, &str); 6] = [
        (
            "albums_artist_fk",
            "SELECT COUNT(*) FROM albums a LEFT JOIN artists ar ON ar.id = a.artist_id WHERE ar.id IS NULL",
        ),
        (
            "tracks_album_fk",
            "SELECT COUNT(*) FROM tracks t LEFT JOIN albums a ON a.id = t.album_id WHERE a.id IS NULL",
        ),
        (
            "tracks_artist_fk",
            "SELECT COUNT(*) FROM tracks t LEFT JOIN artists a ON a.id = t.artist_id WHERE a.id IS NULL",
        ),
        (
            "track_files_track_fk",
            "SELECT COUNT(*) FROM track_files tf LEFT JOIN tracks t ON t.id = tf.track_id WHERE t.id IS NULL",
        ),
        (
            "artist_relationships_source_fk",
            "SELECT COUNT(*) FROM artist_relationships r LEFT JOIN artists a ON a.id = r.source_artist_id WHERE a.id IS NULL",
        ),
        (
            "artist_relationships_related_fk",
            "SELECT COUNT(*) FROM artist_relationships r LEFT JOIN artists a ON a.id = r.related_artist_id WHERE a.id IS NULL",
        ),
    ];

    for (name, query) in fk_checks {
        let orphan_count: i64 = sqlx::query_scalar(query).fetch_one(postgres_pool).await?;
        report
            .referential_integrity
            .push(ReferentialIntegrityCheck { name, orphan_count });
    }

    report
        .table_id_differences
        .sort_by(|a, b| a.table.cmp(b.table));
    report
        .referential_integrity
        .sort_by(|a, b| a.name.cmp(b.name));

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

fn sorted_columns(mut columns: Vec<String>) -> Vec<String> {
    columns.sort_unstable();
    columns
}

async fn sqlite_table_exists(pool: &SqlitePool, table: &str) -> Result<bool> {
    ensure_migrated_table(table)?;
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1")
            .bind(table)
            .fetch_optional(pool)
            .await?;
    Ok(exists.is_some())
}

#[derive(Debug, Default)]
struct IdBatchCursor {
    rows: VecDeque<String>,
    last_seen: Option<String>,
    exhausted: bool,
}

fn push_id_sample(samples: &mut Vec<String>, id: &str) {
    if samples.len() < MAX_ID_SAMPLES_PER_TABLE {
        samples.push(id.to_string());
    }
}

async fn id_difference_samples_for_table(
    sqlite_pool: &SqlitePool,
    postgres_pool: &PgPool,
    table: &str,
) -> Result<(Vec<String>, Vec<String>)> {
    ensure_migrated_table(table)?;

    let mut sqlite_cursor = IdBatchCursor::default();
    let mut postgres_cursor = IdBatchCursor::default();
    let mut missing_ids_in_postgres = Vec::new();
    let mut unexpected_ids_in_postgres = Vec::new();

    let mut sqlite_id = next_sqlite_id(sqlite_pool, table, &mut sqlite_cursor).await?;
    let mut postgres_id = next_postgres_id(postgres_pool, table, &mut postgres_cursor).await?;

    while sqlite_id.is_some() || postgres_id.is_some() {
        match (&sqlite_id, &postgres_id) {
            (Some(sqlite), Some(postgres)) => match sqlite.cmp(postgres) {
                std::cmp::Ordering::Less => {
                    push_id_sample(&mut missing_ids_in_postgres, sqlite);
                    sqlite_id = next_sqlite_id(sqlite_pool, table, &mut sqlite_cursor).await?;
                }
                std::cmp::Ordering::Equal => {
                    sqlite_id = next_sqlite_id(sqlite_pool, table, &mut sqlite_cursor).await?;
                    postgres_id =
                        next_postgres_id(postgres_pool, table, &mut postgres_cursor).await?;
                }
                std::cmp::Ordering::Greater => {
                    push_id_sample(&mut unexpected_ids_in_postgres, postgres);
                    postgres_id =
                        next_postgres_id(postgres_pool, table, &mut postgres_cursor).await?;
                }
            },
            (Some(sqlite), None) => {
                push_id_sample(&mut missing_ids_in_postgres, sqlite);
                sqlite_id = next_sqlite_id(sqlite_pool, table, &mut sqlite_cursor).await?;
            }
            (None, Some(postgres)) => {
                push_id_sample(&mut unexpected_ids_in_postgres, postgres);
                postgres_id = next_postgres_id(postgres_pool, table, &mut postgres_cursor).await?;
            }
            (None, None) => break,
        }
    }

    Ok((missing_ids_in_postgres, unexpected_ids_in_postgres))
}

async fn next_sqlite_id(
    pool: &SqlitePool,
    table: &str,
    cursor: &mut IdBatchCursor,
) -> Result<Option<String>> {
    loop {
        if let Some(id) = cursor.rows.pop_front() {
            return Ok(Some(id));
        }

        if cursor.exhausted {
            return Ok(None);
        }

        let rows = sqlite_id_batch(pool, table, cursor.last_seen.as_deref()).await?;

        if let Some(last_id) = rows.last() {
            cursor.last_seen = Some(last_id.clone());
            cursor.rows = VecDeque::from(rows);
        } else {
            cursor.exhausted = true;
        }
    }
}

async fn next_postgres_id(
    pool: &PgPool,
    table: &str,
    cursor: &mut IdBatchCursor,
) -> Result<Option<String>> {
    loop {
        if let Some(id) = cursor.rows.pop_front() {
            return Ok(Some(id));
        }

        if cursor.exhausted {
            return Ok(None);
        }

        let rows = postgres_id_batch(pool, table, cursor.last_seen.as_deref()).await?;

        if let Some(last_id) = rows.last() {
            cursor.last_seen = Some(last_id.clone());
            cursor.rows = VecDeque::from(rows);
        } else {
            cursor.exhausted = true;
        }
    }
}

async fn sqlite_id_batch(
    pool: &SqlitePool,
    table: &str,
    after_id: Option<&str>,
) -> Result<Vec<String>> {
    let query = sqlite_id_batch_query(table, after_id.is_some())?;
    match after_id {
        Some(last_id) => Ok(sqlx::query_scalar(query)
            .bind(last_id)
            .bind(DEFAULT_BATCH_SIZE)
            .fetch_all(pool)
            .await?),
        None => Ok(sqlx::query_scalar(query)
            .bind(DEFAULT_BATCH_SIZE)
            .fetch_all(pool)
            .await?),
    }
}

async fn postgres_current_schema(pool: &PgPool) -> Result<String> {
    Ok(sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(pool)
        .await?)
}

async fn postgres_table_exists(pool: &PgPool, schema: &str, table: &str) -> Result<bool> {
    ensure_migrated_table(table)?;
    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM information_schema.tables WHERE table_schema = $1 AND table_name = $2 LIMIT 1",
    )
    .bind(schema)
    .bind(table)
    .fetch_optional(pool)
    .await?;
    Ok(exists.is_some())
}

async fn postgres_id_batch(
    pool: &PgPool,
    table: &str,
    after_id: Option<&str>,
) -> Result<Vec<String>> {
    let query = postgres_id_batch_query(table, after_id.is_some())?;
    match after_id {
        Some(last_id) => Ok(sqlx::query_scalar(query)
            .bind(last_id)
            .bind(DEFAULT_BATCH_SIZE)
            .fetch_all(pool)
            .await?),
        None => Ok(sqlx::query_scalar(query)
            .bind(DEFAULT_BATCH_SIZE)
            .fetch_all(pool)
            .await?),
    }
}

fn sqlite_id_batch_query(table: &str, with_after_id: bool) -> Result<&'static str> {
    Ok(match (table, with_after_id) {
        ("quality_profiles", true) => {
            "SELECT id FROM quality_profiles WHERE id > ? ORDER BY id LIMIT ?"
        }
        ("metadata_profiles", true) => {
            "SELECT id FROM metadata_profiles WHERE id > ? ORDER BY id LIMIT ?"
        }
        ("artists", true) => "SELECT id FROM artists WHERE id > ? ORDER BY id LIMIT ?",
        ("albums", true) => "SELECT id FROM albums WHERE id > ? ORDER BY id LIMIT ?",
        ("tracks", true) => "SELECT id FROM tracks WHERE id > ? ORDER BY id LIMIT ?",
        ("track_files", true) => "SELECT id FROM track_files WHERE id > ? ORDER BY id LIMIT ?",
        ("artist_relationships", true) => {
            "SELECT id FROM artist_relationships WHERE id > ? ORDER BY id LIMIT ?"
        }
        ("indexer_definitions", true) => {
            "SELECT id FROM indexer_definitions WHERE id > ? ORDER BY id LIMIT ?"
        }
        ("download_client_definitions", true) => {
            "SELECT id FROM download_client_definitions WHERE id > ? ORDER BY id LIMIT ?"
        }
        ("quality_profiles", false) => "SELECT id FROM quality_profiles ORDER BY id LIMIT ?",
        ("metadata_profiles", false) => "SELECT id FROM metadata_profiles ORDER BY id LIMIT ?",
        ("artists", false) => "SELECT id FROM artists ORDER BY id LIMIT ?",
        ("albums", false) => "SELECT id FROM albums ORDER BY id LIMIT ?",
        ("tracks", false) => "SELECT id FROM tracks ORDER BY id LIMIT ?",
        ("track_files", false) => "SELECT id FROM track_files ORDER BY id LIMIT ?",
        ("artist_relationships", false) => {
            "SELECT id FROM artist_relationships ORDER BY id LIMIT ?"
        }
        ("indexer_definitions", false) => "SELECT id FROM indexer_definitions ORDER BY id LIMIT ?",
        ("download_client_definitions", false) => {
            "SELECT id FROM download_client_definitions ORDER BY id LIMIT ?"
        }
        _ => return Err(anyhow!("unsupported migration table: {table}")),
    })
}

fn postgres_id_batch_query(table: &str, with_after_id: bool) -> Result<&'static str> {
    Ok(match (table, with_after_id) {
        ("quality_profiles", true) => {
            "SELECT id FROM quality_profiles WHERE id > $1 ORDER BY id LIMIT $2"
        }
        ("metadata_profiles", true) => {
            "SELECT id FROM metadata_profiles WHERE id > $1 ORDER BY id LIMIT $2"
        }
        ("artists", true) => "SELECT id FROM artists WHERE id > $1 ORDER BY id LIMIT $2",
        ("albums", true) => "SELECT id FROM albums WHERE id > $1 ORDER BY id LIMIT $2",
        ("tracks", true) => "SELECT id FROM tracks WHERE id > $1 ORDER BY id LIMIT $2",
        ("track_files", true) => "SELECT id FROM track_files WHERE id > $1 ORDER BY id LIMIT $2",
        ("artist_relationships", true) => {
            "SELECT id FROM artist_relationships WHERE id > $1 ORDER BY id LIMIT $2"
        }
        ("indexer_definitions", true) => {
            "SELECT id FROM indexer_definitions WHERE id > $1 ORDER BY id LIMIT $2"
        }
        ("download_client_definitions", true) => {
            "SELECT id FROM download_client_definitions WHERE id > $1 ORDER BY id LIMIT $2"
        }
        ("quality_profiles", false) => "SELECT id FROM quality_profiles ORDER BY id LIMIT $1",
        ("metadata_profiles", false) => "SELECT id FROM metadata_profiles ORDER BY id LIMIT $1",
        ("artists", false) => "SELECT id FROM artists ORDER BY id LIMIT $1",
        ("albums", false) => "SELECT id FROM albums ORDER BY id LIMIT $1",
        ("tracks", false) => "SELECT id FROM tracks ORDER BY id LIMIT $1",
        ("track_files", false) => "SELECT id FROM track_files ORDER BY id LIMIT $1",
        ("artist_relationships", false) => {
            "SELECT id FROM artist_relationships ORDER BY id LIMIT $1"
        }
        ("indexer_definitions", false) => "SELECT id FROM indexer_definitions ORDER BY id LIMIT $1",
        ("download_client_definitions", false) => {
            "SELECT id FROM download_client_definitions ORDER BY id LIMIT $1"
        }
        _ => return Err(anyhow!("unsupported migration table: {table}")),
    })
}

async fn sqlite_columns_for_table(pool: &SqlitePool, table: &str) -> Result<HashSet<String>> {
    ensure_migrated_table(table)?;
    let pragma = format!("SELECT name FROM pragma_table_info('{table}')");
    let rows: Vec<String> = sqlx::query_scalar(&pragma).fetch_all(pool).await?;
    Ok(rows.into_iter().collect())
}

async fn postgres_columns_for_table(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> Result<HashSet<String>> {
    ensure_migrated_table(table)?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
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

    #[test]
    fn schema_report_has_differences_detects_presence() {
        let report = SchemaComparisonReport::default();
        assert!(!report.has_differences());

        let report = SchemaComparisonReport {
            missing_tables_in_postgres: vec!["artists"],
            ..SchemaComparisonReport::default()
        };
        assert!(report.has_differences());
    }

    #[test]
    fn data_validation_report_has_issues_detects_failures() {
        let clean = DataValidationReport::default();
        assert!(!clean.has_issues());

        let with_id_diff = DataValidationReport {
            table_id_differences: vec![TableIdDifference {
                table: "artists",
                missing_ids_in_postgres: vec!["a-1".to_string()],
                unexpected_ids_in_postgres: Vec::new(),
            }],
            ..DataValidationReport::default()
        };
        assert!(with_id_diff.has_issues());

        let with_orphans = DataValidationReport {
            referential_integrity: vec![ReferentialIntegrityCheck {
                name: "tracks_album_fk",
                orphan_count: 1,
            }],
            ..DataValidationReport::default()
        };
        assert!(with_orphans.has_issues());
    }
}
