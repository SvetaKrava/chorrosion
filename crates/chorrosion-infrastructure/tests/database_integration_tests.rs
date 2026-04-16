// SPDX-License-Identifier: GPL-3.0-or-later

use chorrosion_config::AppConfig;
use chorrosion_domain::{Album, Artist, ArtistRelationship, Track, TrackFile};
#[cfg(feature = "postgres")]
use chorrosion_domain::{
    DownloadClientDefinition, IndexerDefinition, MetadataProfile, QualityProfile,
};
#[cfg(feature = "postgres")]
use chorrosion_infrastructure::create_postgres_pool;
use chorrosion_infrastructure::init_database;
#[cfg(feature = "postgres")]
use chorrosion_infrastructure::init_postgres_database;
#[cfg(feature = "postgres")]
use chorrosion_infrastructure::postgres_adapters::{
    PostgresAlbumRepository, PostgresArtistRelationshipRepository, PostgresArtistRepository,
    PostgresDownloadClientDefinitionRepository, PostgresIndexerDefinitionRepository,
    PostgresMetadataProfileRepository, PostgresQualityProfileRepository,
    PostgresTrackFileRepository, PostgresTrackRepository,
};
use chorrosion_infrastructure::repositories::{
    AlbumRepository, ArtistRelationshipRepository, Repository, TrackFileRepository, TrackRepository,
};
#[cfg(feature = "postgres")]
use chorrosion_infrastructure::repositories::{
    ArtistRepository, DownloadClientDefinitionRepository, IndexerDefinitionRepository,
    MetadataProfileRepository, QualityProfileRepository,
};
use chorrosion_infrastructure::sqlite_adapters::{
    SqliteAlbumRepository, SqliteArtistRelationshipRepository, SqliteArtistRepository,
    SqliteTrackFileRepository, SqliteTrackRepository,
};
#[cfg(feature = "postgres")]
use chorrosion_infrastructure::sqlite_to_postgres::{
    migrate_sqlite_to_postgres_with_options, MigrationOptions, TargetResetPolicy,
};
#[cfg(feature = "postgres")]
use sqlx::Executor;
#[cfg(feature = "postgres")]
use sqlx::PgPool;
use sqlx::SqlitePool;
#[cfg(feature = "postgres")]
use uuid::Uuid;

async fn setup_pool() -> SqlitePool {
    let mut config = AppConfig::default();
    config.database.url = "sqlite://:memory:".to_string();
    config.database.pool_max_size = 1;

    init_database(&config)
        .await
        .expect("init in-memory sqlite with migrations")
}

#[tokio::test]
async fn artist_album_track_track_file_workflow_round_trip() {
    let pool = setup_pool().await;

    let artist_repo = SqliteArtistRepository::new(pool.clone());
    let album_repo = SqliteAlbumRepository::new(pool.clone());
    let track_repo = SqliteTrackRepository::new(pool.clone());
    let track_file_repo = SqliteTrackFileRepository::new(pool.clone());

    let artist = Artist::new("Integration Artist");
    let artist_id = artist.id;
    artist_repo.create(artist).await.expect("create artist");

    let album = Album::new(artist_id, "Integration Album");
    let album_id = album.id;
    album_repo.create(album).await.expect("create album");

    let track = Track::new(album_id, artist_id, "Integration Track");
    let track_id = track.id;
    track_repo.create(track).await.expect("create track");

    let track_file = TrackFile::new(track_id, "/music/integration-track.flac", 12_345);
    let track_file_id = track_file.id;
    track_file_repo
        .create(track_file)
        .await
        .expect("create track file");

    let tracks = track_repo
        .get_by_album(album_id, 10, 0)
        .await
        .expect("get tracks by album");
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].id, track_id);

    let files = track_file_repo
        .get_by_track(track_id, 10, 0)
        .await
        .expect("get files by track");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, track_file_id);
    assert_eq!(files[0].path, "/music/integration-track.flac");

    let by_path = track_file_repo
        .get_by_path("/music/integration-track.flac")
        .await
        .expect("get file by path")
        .expect("track file should exist");
    assert_eq!(by_path.id, track_file_id);
}

#[tokio::test]
async fn wanted_without_tracks_relationship_and_track_transition_workflow() {
    let pool = setup_pool().await;

    let artist_repo = SqliteArtistRepository::new(pool.clone());
    let album_repo = SqliteAlbumRepository::new(pool.clone());
    let track_repo = SqliteTrackRepository::new(pool.clone());
    let relationship_repo = SqliteArtistRelationshipRepository::new(pool.clone());

    let mut artist_a = Artist::new("Artist A");
    artist_a.genre_tags = Some("rock|indie".to_string());
    let artist_a_id = artist_a.id;

    let mut artist_b = Artist::new("Artist B");
    artist_b.style_tags = Some("atmospheric|melodic".to_string());
    let artist_b_id = artist_b.id;

    artist_repo.create(artist_a).await.expect("create artist A");
    artist_repo.create(artist_b).await.expect("create artist B");

    let mut relationship = ArtistRelationship::new(artist_a_id, artist_b_id, "collaborator");
    relationship.description = Some("Featured collaboration".to_string());
    relationship_repo
        .create(relationship)
        .await
        .expect("create relationship");

    let mut wanted_album = Album::new(artist_a_id, "Wanted But Missing Tracks");
    wanted_album.style_tags = Some("dream-pop".to_string());
    let wanted_album_id = wanted_album.id;
    album_repo
        .create(wanted_album)
        .await
        .expect("create wanted album");

    let wanted_without_tracks_before = album_repo
        .list_wanted_without_tracks(10, 0)
        .await
        .expect("list wanted without tracks before track exists");
    assert!(wanted_without_tracks_before
        .iter()
        .any(|album| album.id == wanted_album_id));

    let track = Track::new(wanted_album_id, artist_a_id, "Now Exists");
    track_repo
        .create(track)
        .await
        .expect("create track for album");

    let wanted_without_tracks_after = album_repo
        .list_wanted_without_tracks(10, 0)
        .await
        .expect("list wanted without tracks after track exists");
    assert!(wanted_without_tracks_after
        .iter()
        .all(|album| album.id != wanted_album_id));

    let relationships = relationship_repo
        .get_by_source_artist(artist_a_id, 10, 0)
        .await
        .expect("get relationships by source");
    assert_eq!(relationships.len(), 1);
    assert_eq!(relationships[0].related_artist_id, artist_b_id);
    assert_eq!(relationships[0].relationship_type, "collaborator");

    let exists = relationship_repo
        .relationship_exists(artist_a_id, artist_b_id, "collaborator")
        .await
        .expect("relationship existence check");
    assert!(exists);
}

#[cfg(feature = "postgres")]
async fn setup_postgres_pool_from_env() -> Option<PgPool> {
    let postgres_url = std::env::var("CHORROSION_TEST_POSTGRES_URL").ok()?;

    let mut config = AppConfig::default();
    config.database.url = postgres_url;
    config.database.pool_max_size = 1;

    Some(
        create_postgres_pool(&config)
            .await
            .expect("create postgres pool"),
    )
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_pool_connectivity_check() {
    let Some(pool) = setup_postgres_pool_from_env().await else {
        return;
    };

    let one: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&pool)
        .await
        .expect("postgres connectivity query should succeed");
    assert_eq!(one, 1);
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_init_database_runs_migrations() {
    let Some(_pool) = setup_postgres_pool_from_env().await else {
        return;
    };

    let postgres_url = std::env::var("CHORROSION_TEST_POSTGRES_URL")
        .expect("CHORROSION_TEST_POSTGRES_URL should be set when running this test");

    let mut config = AppConfig::default();
    config.database.url = postgres_url;
    config.database.pool_max_size = 2;

    let pool = init_postgres_database(&config)
        .await
        .expect("postgres init should run migrations successfully");

    let artists_table: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.artists')::text")
            .fetch_one(&pool)
            .await
            .expect("postgres should be able to resolve migrated artists table");
    assert_eq!(artists_table, Some("artists".to_string()));
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_artist_repository_crud_and_filters() {
    let Some(pool) = setup_postgres_pool_from_env().await else {
        return;
    };

    sqlx::query(
        r#"
        CREATE TEMP TABLE IF NOT EXISTS artists (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            foreign_artist_id TEXT,
            musicbrainz_artist_id TEXT,
            metadata_profile_id TEXT,
            quality_profile_id TEXT,
            status TEXT NOT NULL DEFAULT 'continuing',
            path TEXT,
            monitored BOOLEAN NOT NULL DEFAULT TRUE,
            artist_type TEXT,
            sort_name TEXT,
            country TEXT,
            disambiguation TEXT,
            genre_tags TEXT,
            style_tags TEXT,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("create temporary artists table for postgres test");

    let repo = PostgresArtistRepository::new(pool.clone());

    let mut artist = Artist::new("Postgres Artist");
    artist.foreign_artist_id = Some("foreign-artist-1".to_string());
    artist.monitored = true;
    artist.genre_tags = Some("rock|alt".to_string());

    let artist_id = artist.id.to_string();
    repo.create(artist).await.expect("create postgres artist");

    let by_id = repo
        .get_by_id(&artist_id)
        .await
        .expect("get artist by id")
        .expect("artist should exist");
    assert_eq!(by_id.name, "Postgres Artist");

    let by_name = repo
        .get_by_name("postgres artist")
        .await
        .expect("get artist by name")
        .expect("artist should be found case-insensitively");
    assert_eq!(by_name.id.to_string(), artist_id);

    let by_foreign_id = repo
        .get_by_foreign_id("foreign-artist-1")
        .await
        .expect("get artist by foreign id")
        .expect("artist should be found by foreign id");
    assert_eq!(by_foreign_id.id.to_string(), artist_id);

    let monitored = repo
        .list_monitored(10, 0)
        .await
        .expect("list monitored artists");
    assert_eq!(monitored.len(), 1);

    let mut updated = by_id.clone();
    updated.status = chorrosion_domain::ArtistStatus::Ended;
    updated.monitored = false;
    updated.name = "Postgres Artist Updated".to_string();
    repo.update(updated).await.expect("update postgres artist");

    let ended = repo
        .get_by_status(chorrosion_domain::ArtistStatus::Ended, 10, 0)
        .await
        .expect("list artists by status");
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].name, "Postgres Artist Updated");

    let monitored_after_update = repo
        .list_monitored(10, 0)
        .await
        .expect("list monitored artists after update");
    assert!(monitored_after_update.is_empty());

    repo.delete(&artist_id)
        .await
        .expect("delete postgres artist");

    let gone = repo
        .get_by_id(&artist_id)
        .await
        .expect("get artist after delete");
    assert!(gone.is_none());
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn sqlite_to_postgres_migration_copies_core_rows() {
    let mut sqlite_config = AppConfig::default();
    sqlite_config.database.url = "sqlite://:memory:".to_string();
    sqlite_config.database.pool_max_size = 1;
    let sqlite_pool = init_database(&sqlite_config)
        .await
        .expect("initialize sqlite source pool");

    let artist = Artist::new("Migration Artist");
    let artist_id = artist.id;
    SqliteArtistRepository::new(sqlite_pool.clone())
        .create(artist)
        .await
        .expect("create source artist");

    let album = Album::new(artist_id, "Migration Album");
    let album_id = album.id;
    SqliteAlbumRepository::new(sqlite_pool.clone())
        .create(album)
        .await
        .expect("create source album");

    let track = Track::new(album_id, artist_id, "Migration Track");
    let track_id = track.id;
    SqliteTrackRepository::new(sqlite_pool.clone())
        .create(track)
        .await
        .expect("create source track");

    let file = TrackFile::new(track_id, "/music/migration-track.flac", 42_000);
    SqliteTrackFileRepository::new(sqlite_pool.clone())
        .create(file)
        .await
        .expect("create source track file");

    let Some(_pool) = setup_postgres_pool_from_env().await else {
        return;
    };

    let postgres_url = std::env::var("CHORROSION_TEST_POSTGRES_URL")
        .expect("CHORROSION_TEST_POSTGRES_URL should be set when running this test");
    let isolated_schema = format!("it_sqlite_to_postgres_{}", Uuid::new_v4().simple());
    let escaped_schema = isolated_schema.replace('"', "\"\"");
    sqlx::query(&format!("CREATE SCHEMA \"{escaped_schema}\""))
        .execute(&_pool)
        .await
        .expect("create isolated schema for sqlite->postgres migration test");

    let mut postgres_config = AppConfig::default();
    postgres_config.database.url = postgres_url_with_search_path(&postgres_url, &isolated_schema);
    postgres_config.database.pool_max_size = 2;
    let postgres_pool = init_postgres_database(&postgres_config)
        .await
        .expect("initialize postgres target pool");

    let report = migrate_sqlite_to_postgres_with_options(
        &sqlite_pool,
        &postgres_pool,
        MigrationOptions {
            target_reset_policy: TargetResetPolicy::TruncateAll,
            ..MigrationOptions::default()
        },
    )
    .await
    .expect("migrate sqlite data into postgres");
    assert!(
        report.mismatched_tables().is_empty(),
        "all migrated tables should have matching row counts"
    );

    let pg_artist_repo = PostgresArtistRepository::new(postgres_pool.clone());
    let migrated_artist = pg_artist_repo
        .get_by_id(&artist_id.to_string())
        .await
        .expect("query migrated artist")
        .expect("artist should exist in postgres after migration");
    assert_eq!(migrated_artist.name, "Migration Artist");

    sqlx::query(&format!(
        "DROP SCHEMA IF EXISTS \"{escaped_schema}\" CASCADE"
    ))
    .execute(&_pool)
    .await
    .expect("drop isolated schema for sqlite->postgres migration test");
}

#[cfg(feature = "postgres")]
fn postgres_url_with_search_path(base_url: &str, schema: &str) -> String {
    let separator = if base_url.contains('?') { '&' } else { '?' };
    format!("{base_url}{separator}options=-csearch_path%3D{schema}")
}

#[cfg(feature = "postgres")]
async fn create_postgres_repository_temp_tables(pool: &PgPool) {
    let statements = [
        r#"
        CREATE TEMP TABLE IF NOT EXISTS artists (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            foreign_artist_id TEXT,
            musicbrainz_artist_id TEXT,
            metadata_profile_id TEXT,
            quality_profile_id TEXT,
            status TEXT NOT NULL DEFAULT 'continuing',
            path TEXT,
            monitored BOOLEAN NOT NULL DEFAULT TRUE,
            artist_type TEXT,
            sort_name TEXT,
            country TEXT,
            disambiguation TEXT,
            genre_tags TEXT,
            style_tags TEXT,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
        r#"
        CREATE TEMP TABLE IF NOT EXISTS albums (
            id TEXT PRIMARY KEY,
            artist_id TEXT NOT NULL,
            foreign_album_id TEXT,
            musicbrainz_release_group_id TEXT,
            musicbrainz_release_id TEXT,
            title TEXT NOT NULL,
            release_date TEXT,
            album_type TEXT,
            primary_type TEXT,
            secondary_types TEXT,
            first_release_date TEXT,
            genre_tags TEXT,
            style_tags TEXT,
            status TEXT NOT NULL DEFAULT 'wanted',
            monitored BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
        r#"
        CREATE TEMP TABLE IF NOT EXISTS tracks (
            id TEXT PRIMARY KEY,
            album_id TEXT NOT NULL,
            artist_id TEXT NOT NULL,
            foreign_track_id TEXT,
            title TEXT NOT NULL,
            track_number INTEGER,
            duration_ms INTEGER,
            has_file BOOLEAN NOT NULL DEFAULT FALSE,
            monitored BOOLEAN NOT NULL DEFAULT TRUE,
            musicbrainz_recording_id TEXT,
            match_confidence DOUBLE PRECISION,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
        r#"
        CREATE TEMP TABLE IF NOT EXISTS quality_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            allowed_qualities TEXT NOT NULL,
            upgrade_allowed BOOLEAN NOT NULL DEFAULT FALSE,
            cutoff_quality TEXT,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
        r#"
        CREATE TEMP TABLE IF NOT EXISTS metadata_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            primary_album_types TEXT NOT NULL DEFAULT '[]',
            secondary_album_types TEXT NOT NULL DEFAULT '[]',
            release_statuses TEXT NOT NULL DEFAULT '[]',
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
        r#"
        CREATE TEMP TABLE IF NOT EXISTS indexer_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            base_url TEXT NOT NULL,
            protocol TEXT NOT NULL,
            api_key TEXT,
            enabled BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
        r#"
        CREATE TEMP TABLE IF NOT EXISTS download_client_definitions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            client_type TEXT NOT NULL,
            base_url TEXT NOT NULL,
            username TEXT,
            password_encrypted TEXT,
            category TEXT,
            enabled BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
        r#"
        CREATE TEMP TABLE IF NOT EXISTS track_files (
            id TEXT PRIMARY KEY,
            track_id TEXT NOT NULL,
            path TEXT NOT NULL,
            size_bytes BIGINT NOT NULL,
            duration_ms INTEGER,
            bitrate_kbps INTEGER,
            channels SMALLINT,
            codec TEXT,
            quality TEXT,
            hash TEXT,
            fingerprint_hash TEXT,
            fingerprint_duration INTEGER,
            fingerprint_computed_at TIMESTAMP,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
        r#"
        CREATE TEMP TABLE IF NOT EXISTS artist_relationships (
            id TEXT PRIMARY KEY,
            source_artist_id TEXT NOT NULL,
            related_artist_id TEXT NOT NULL,
            relationship_type TEXT NOT NULL,
            description TEXT,
            created_at TIMESTAMP NOT NULL,
            updated_at TIMESTAMP NOT NULL
        )
        "#,
    ];

    for statement in statements {
        pool.execute(statement)
            .await
            .expect("create postgres temp table");
    }
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_remaining_adapters_crud_and_specialized_queries() {
    let Some(pool) = setup_postgres_pool_from_env().await else {
        return;
    };

    create_postgres_repository_temp_tables(&pool).await;

    let artist_repo = PostgresArtistRepository::new(pool.clone());
    let album_repo = PostgresAlbumRepository::new(pool.clone());
    let track_repo = PostgresTrackRepository::new(pool.clone());
    let quality_profile_repo = PostgresQualityProfileRepository::new(pool.clone());
    let metadata_profile_repo = PostgresMetadataProfileRepository::new(pool.clone());
    let indexer_repo = PostgresIndexerDefinitionRepository::new(pool.clone());
    let download_client_repo = PostgresDownloadClientDefinitionRepository::new(pool.clone());
    let track_file_repo = PostgresTrackFileRepository::new(pool.clone());
    let relationship_repo = PostgresArtistRelationshipRepository::new(pool.clone());

    let mut quality_profile = QualityProfile::new(
        "Postgres Quality Profile",
        vec!["FLAC".to_string(), "MP3".to_string()],
    );
    quality_profile.upgrade_allowed = true;
    quality_profile.cutoff_quality = Some("FLAC".to_string());
    let quality_profile_id = quality_profile.id;
    let quality_profile_id_str = quality_profile_id.to_string();
    quality_profile_repo
        .create(quality_profile)
        .await
        .expect("create postgres quality profile");
    assert!(quality_profile_repo
        .get_by_name("Postgres Quality Profile")
        .await
        .expect("get quality profile by name")
        .is_some());

    let mut quality_profile_updated = quality_profile_repo
        .get_by_id(&quality_profile_id_str)
        .await
        .expect("get quality profile by id")
        .expect("quality profile exists");
    quality_profile_updated.name = "Postgres Quality Profile Updated".to_string();
    quality_profile_repo
        .update(quality_profile_updated)
        .await
        .expect("update quality profile");

    let mut metadata_profile = MetadataProfile::new("Postgres Metadata Profile");
    metadata_profile.primary_album_types = vec!["Album".to_string()];
    let metadata_profile_id = metadata_profile.id;
    let metadata_profile_id_str = metadata_profile_id.to_string();
    metadata_profile_repo
        .create(metadata_profile)
        .await
        .expect("create postgres metadata profile");
    assert!(metadata_profile_repo
        .get_by_name("Postgres Metadata Profile")
        .await
        .expect("get metadata profile by name")
        .is_some());

    let mut metadata_profile_updated = metadata_profile_repo
        .get_by_id(&metadata_profile_id_str)
        .await
        .expect("get metadata profile by id")
        .expect("metadata profile exists");
    metadata_profile_updated.name = "Postgres Metadata Profile Updated".to_string();
    metadata_profile_repo
        .update(metadata_profile_updated)
        .await
        .expect("update metadata profile");

    let mut artist = Artist::new("Postgres Repo Artist");
    artist.quality_profile_id = Some(quality_profile_id);
    artist.metadata_profile_id = Some(metadata_profile_id);
    let artist_id = artist.id;
    artist_repo
        .create(artist)
        .await
        .expect("create postgres artist for album/track tests");

    let mut album = Album::new(artist_id, "100% Hits");
    album.foreign_album_id = Some("album-foreign-id".to_string());
    let album_id = album.id;
    let album_id_str = album_id.to_string();
    album_repo
        .create(album)
        .await
        .expect("create postgres album");

    let wildcard_lookup = album_repo
        .get_by_artist_and_title(artist_id, "100% H_ts")
        .await
        .expect("query album by title with wildcard chars");
    assert!(wildcard_lookup.is_none());

    let exact_lookup = album_repo
        .get_by_artist_and_title(artist_id, "100% hits")
        .await
        .expect("query album by exact case-insensitive title")
        .expect("album should match exactly");
    assert_eq!(exact_lookup.id, album_id);

    let mut track = Track::new(album_id, artist_id, "Cutoff Track");
    track.has_file = false;
    let track_id = track.id;
    let track_id_str = track.id.to_string();
    track_repo
        .create(track)
        .await
        .expect("create postgres track");

    let without_files = track_repo
        .list_without_files(10, 0)
        .await
        .expect("list tracks without files");
    assert!(without_files.iter().any(|item| item.id == track_id));

    let mut updated_track = track_repo
        .get_by_id(&track_id_str)
        .await
        .expect("get track by id")
        .expect("track exists");
    updated_track.title = "Cutoff Track Updated".to_string();
    track_repo
        .update(updated_track)
        .await
        .expect("update postgres track");

    let mut track_file = TrackFile::new(track_id, "/music/postgres-cutoff.mp3", 1024);
    track_file.codec = Some("MP3".to_string());
    track_file.quality = Some("MP3".to_string());
    track_file.fingerprint_hash = Some("fp-hash-1".to_string());
    let track_file_id = track_file.id.to_string();
    track_file_repo
        .create(track_file)
        .await
        .expect("create postgres track file");

    let with_fingerprint = track_file_repo
        .list_with_fingerprints(10, 0)
        .await
        .expect("list track files with fingerprints");
    assert!(with_fingerprint
        .iter()
        .any(|item| item.path == "/music/postgres-cutoff.mp3"));

    let mut updated_track_file = track_file_repo
        .get_by_id(&track_file_id)
        .await
        .expect("get track file by id")
        .expect("track file exists");
    updated_track_file.path = "/music/postgres-cutoff-updated.mp3".to_string();
    track_file_repo
        .update(updated_track_file)
        .await
        .expect("update postgres track file");
    assert!(track_file_repo
        .get_by_path("/music/postgres-cutoff-updated.mp3")
        .await
        .expect("get track file by path")
        .is_some());

    let cutoff_unmet = album_repo
        .list_cutoff_unmet_albums(10, 0)
        .await
        .expect("list cutoff-unmet albums");
    assert!(cutoff_unmet.iter().any(|item| item.id == album_id));

    let mut updated_album = album_repo
        .get_by_id(&album_id_str)
        .await
        .expect("get album by id")
        .expect("album exists");
    updated_album.title = "100% Hits Updated".to_string();
    album_repo
        .update(updated_album)
        .await
        .expect("update postgres album");

    let mut indexer = IndexerDefinition::new("Indexer A", "https://idx.example", "torznab");
    indexer.api_key = Some("idx-key".to_string());
    let indexer_id = indexer.id.to_string();
    indexer_repo
        .create(indexer)
        .await
        .expect("create indexer definition");
    assert!(indexer_repo
        .get_by_name("Indexer A")
        .await
        .expect("get indexer by name")
        .is_some());

    let mut updated_indexer = indexer_repo
        .get_by_id(&indexer_id)
        .await
        .expect("get indexer by id")
        .expect("indexer exists");
    updated_indexer.enabled = false;
    indexer_repo
        .update(updated_indexer)
        .await
        .expect("update indexer definition");

    let mut download_client =
        DownloadClientDefinition::new("Client A", "qbittorrent", "http://localhost:8080");
    download_client.username = Some("admin".to_string());
    let download_client_id = download_client.id.to_string();
    download_client_repo
        .create(download_client)
        .await
        .expect("create download client definition");
    assert!(download_client_repo
        .get_by_name("Client A")
        .await
        .expect("get download client by name")
        .is_some());

    let mut updated_download_client = download_client_repo
        .get_by_id(&download_client_id)
        .await
        .expect("get download client by id")
        .expect("download client exists");
    updated_download_client.enabled = false;
    download_client_repo
        .update(updated_download_client)
        .await
        .expect("update download client definition");

    let mut related_artist = Artist::new("Related Artist");
    related_artist.quality_profile_id = Some(quality_profile_id);
    related_artist.metadata_profile_id = Some(metadata_profile_id);
    let related_artist_id = related_artist.id;
    artist_repo
        .create(related_artist)
        .await
        .expect("create related artist");

    let relationship = ArtistRelationship::new(artist_id, related_artist_id, "collaborator");
    let relationship_id = relationship.id.to_string();
    relationship_repo
        .create(relationship)
        .await
        .expect("create artist relationship");
    assert!(relationship_repo
        .relationship_exists(artist_id, related_artist_id, "collaborator")
        .await
        .expect("relationship exists check"));

    let mut updated_relationship = relationship_repo
        .get_by_id(&relationship_id)
        .await
        .expect("get artist relationship by id")
        .expect("relationship exists");
    updated_relationship.description = Some("updated".to_string());
    relationship_repo
        .update(updated_relationship)
        .await
        .expect("update artist relationship");

    relationship_repo
        .delete(&relationship_id)
        .await
        .expect("delete artist relationship");
    track_file_repo
        .delete(&track_file_id)
        .await
        .expect("delete track file");
    track_repo
        .delete(&track_id_str)
        .await
        .expect("delete track");
    album_repo
        .delete(&album_id_str)
        .await
        .expect("delete album");
    indexer_repo
        .delete(&indexer_id)
        .await
        .expect("delete indexer");
    download_client_repo
        .delete(&download_client_id)
        .await
        .expect("delete download client");
    metadata_profile_repo
        .delete(&metadata_profile_id_str)
        .await
        .expect("delete metadata profile");
    quality_profile_repo
        .delete(&quality_profile_id_str)
        .await
        .expect("delete quality profile");
}
