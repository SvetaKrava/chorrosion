// SPDX-License-Identifier: GPL-3.0-or-later

use chorrosion_domain::{Album, Artist, ArtistRelationship, Track, TrackFile};
use chorrosion_infrastructure::repositories::{
    AlbumRepository, ArtistRelationshipRepository, Repository, TrackFileRepository, TrackRepository,
};
use chorrosion_infrastructure::sqlite_adapters::{
    SqliteAlbumRepository, SqliteArtistRelationshipRepository, SqliteArtistRepository,
    SqliteTrackFileRepository, SqliteTrackRepository,
};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn setup_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrate");

    pool
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
