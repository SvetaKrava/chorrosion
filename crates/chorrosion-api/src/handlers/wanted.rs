// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::{Album, AlbumStatus};
use chorrosion_infrastructure::repositories::{AlbumRepository, Repository};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct WantedQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WantedAlbumResponse {
    pub id: String,
    pub artist_id: String,
    pub foreign_album_id: Option<String>,
    pub title: String,
    pub release_date: Option<String>,
    pub album_type: Option<String>,
    pub status: String,
    pub monitored: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WantedAlbumsResponse {
    pub items: Vec<WantedAlbumResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WantedErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WantedManualSearchResponse {
    pub album_id: String,
    pub query: String,
    pub status: String,
    pub message: String,
}

impl From<Album> for WantedAlbumResponse {
    fn from(album: Album) -> Self {
        Self {
            id: album.id.to_string(),
            artist_id: album.artist_id.to_string(),
            foreign_album_id: album.foreign_album_id,
            title: album.title,
            release_date: album.release_date.map(|d| d.format("%Y-%m-%d").to_string()),
            album_type: album.album_type,
            status: album.status.to_string(),
            monitored: album.monitored,
        }
    }
}

fn validate_query(query: &WantedQuery) -> Result<(), (StatusCode, Json<WantedErrorResponse>)> {
    if !(1..=500).contains(&query.limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(WantedErrorResponse {
                error: "limit must be between 1 and 500".to_string(),
            }),
        ));
    }
    if query.offset < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(WantedErrorResponse {
                error: "offset must be greater than or equal to 0".to_string(),
            }),
        ));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v1/wanted",
    params(WantedQuery),
    responses(
        (status = 200, description = "Paginated list of wanted albums", body = WantedAlbumsResponse),
        (status = 400, description = "Invalid query parameters", body = WantedErrorResponse),
        (status = 500, description = "Internal server error", body = WantedErrorResponse),
    ),
    tag = "wanted"
)]
pub async fn list_wanted_albums(
    State(state): State<AppState>,
    Query(query): Query<WantedQuery>,
) -> Result<Json<WantedAlbumsResponse>, (StatusCode, Json<WantedErrorResponse>)> {
    debug!(target: "api", ?query, "listing wanted albums");
    validate_query(&query)?;

    // Fetch a large window to compute an accurate total, consistent with other
    // list endpoints in the codebase.
    let all = state
        .album_repository
        .get_by_status(AlbumStatus::Wanted, 5000, 0)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WantedErrorResponse {
                    error: format!("failed to list wanted albums: {e}"),
                }),
            )
        })?;

    let total = all.len() as i64;
    let offset = usize::try_from(query.offset).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(WantedErrorResponse {
                error: "offset is out of range".to_string(),
            }),
        )
    })?;
    let limit = query.limit as usize;
    let items = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(WantedAlbumResponse::from)
        .collect();

    Ok(Json(WantedAlbumsResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/wanted/missing",
    params(WantedQuery),
    responses(
        (status = 200, description = "Paginated list of wanted albums with no tracks", body = WantedAlbumsResponse),
        (status = 400, description = "Invalid query parameters", body = WantedErrorResponse),
        (status = 500, description = "Internal server error", body = WantedErrorResponse),
    ),
    tag = "wanted"
)]
pub async fn list_missing_albums(
    State(state): State<AppState>,
    Query(query): Query<WantedQuery>,
) -> Result<Json<WantedAlbumsResponse>, (StatusCode, Json<WantedErrorResponse>)> {
    debug!(target: "api", ?query, "listing missing (wanted, no tracks) albums");
    validate_query(&query)?;

    let all = state
        .album_repository
        .list_wanted_without_tracks(5000, 0)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WantedErrorResponse {
                    error: format!("failed to list missing albums: {e}"),
                }),
            )
        })?;

    let total = all.len() as i64;
    let offset = usize::try_from(query.offset).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(WantedErrorResponse {
                error: "offset is out of range".to_string(),
            }),
        )
    })?;
    let limit = query.limit as usize;
    let items = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(WantedAlbumResponse::from)
        .collect();

    Ok(Json(WantedAlbumsResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/wanted/cutoff",
    params(WantedQuery),
    responses(
        (status = 200, description = "Paginated list of albums with files below quality cutoff", body = WantedAlbumsResponse),
        (status = 400, description = "Invalid query parameters", body = WantedErrorResponse),
        (status = 500, description = "Internal server error", body = WantedErrorResponse),
    ),
    tag = "wanted"
)]
pub async fn list_cutoff_unmet_albums(
    State(state): State<AppState>,
    Query(query): Query<WantedQuery>,
) -> Result<Json<WantedAlbumsResponse>, (StatusCode, Json<WantedErrorResponse>)> {
    debug!(target: "api", ?query, "listing cutoff-unmet albums");
    validate_query(&query)?;

    let all = state
        .album_repository
        .list_cutoff_unmet_albums(5000, 0)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WantedErrorResponse {
                    error: format!("failed to list cutoff-unmet albums: {e}"),
                }),
            )
        })?;

    let total = all.len() as i64;
    let offset = usize::try_from(query.offset).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(WantedErrorResponse {
                error: "offset is out of range".to_string(),
            }),
        )
    })?;
    let limit = query.limit as usize;
    let items = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(WantedAlbumResponse::from)
        .collect();

    Ok(Json(WantedAlbumsResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/wanted/{id}/search",
    params(
        ("id" = String, Path, description = "Wanted album ID")
    ),
    responses(
        (status = 202, description = "Wanted album search triggered", body = WantedManualSearchResponse),
        (status = 404, description = "Album not found", body = WantedErrorResponse),
        (status = 409, description = "Album is not wanted", body = WantedErrorResponse),
        (status = 500, description = "Internal server error", body = WantedErrorResponse),
    ),
    tag = "wanted"
)]
pub async fn trigger_wanted_album_search(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "triggering wanted album search");

    let album = match state.album_repository.get_by_id(id.clone()).await {
        Ok(Some(album)) => album,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(WantedErrorResponse {
                    error: format!("Album {} not found", id),
                }),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WantedErrorResponse {
                    error: format!("failed to fetch album: {error}"),
                }),
            )
                .into_response();
        }
    };

    if album.status != AlbumStatus::Wanted {
        return (
            StatusCode::CONFLICT,
            Json(WantedErrorResponse {
                error: format!(
                    "Album {} has status '{}' and is not eligible for wanted search",
                    album.id, album.status
                ),
            }),
        )
            .into_response();
    }

    let artist_name = match state
        .artist_repository
        .get_by_id(album.artist_id.to_string())
        .await
    {
        Ok(Some(artist)) => artist.name,
        Ok(None) => {
            warn!(
                target: "api",
                album_id = %album.id,
                artist_id = %album.artist_id,
                "artist not found for wanted album; searching by album title only"
            );
            String::new()
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WantedErrorResponse {
                    error: format!("failed to fetch artist: {error}"),
                }),
            )
                .into_response();
        }
    };

    let query = format!("{} {}", artist_name, album.title)
        .trim()
        .to_string();

    (
        StatusCode::ACCEPTED,
        Json(WantedManualSearchResponse {
            album_id: album.id.to_string(),
            query,
            status: "queued".to_string(),
            message: "wanted album search request accepted".to_string(),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Path;
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::repositories::Repository;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackFileRepository, SqliteTrackRepository,
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;

    async fn make_test_pool_and_state() -> (SqlitePool, AppState) {
        use sqlx::sqlite::SqlitePoolOptions;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations");
        let state = AppState::new(
            AppConfig::default(),
            Arc::new(SqliteArtistRepository::new(pool.clone())),
            Arc::new(SqliteAlbumRepository::new(pool.clone())),
            Arc::new(SqliteTrackRepository::new(pool.clone())),
            Arc::new(SqliteQualityProfileRepository::new(pool.clone())),
            Arc::new(SqliteMetadataProfileRepository::new(pool.clone())),
            Arc::new(SqliteIndexerDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool.clone())),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        );
        (pool, state)
    }

    async fn make_test_state() -> AppState {
        make_test_pool_and_state().await.1
    }

    async fn create_test_artist(state: &AppState) -> chorrosion_domain::Artist {
        state
            .artist_repository
            .create(chorrosion_domain::Artist::new("Test Artist"))
            .await
            .expect("create artist")
    }

    async fn create_test_album(
        state: &AppState,
        artist: &chorrosion_domain::Artist,
        status: AlbumStatus,
    ) -> chorrosion_domain::Album {
        let mut album = chorrosion_domain::Album::new(artist.id, "Test Album");
        album.status = status;
        state
            .album_repository
            .create(album)
            .await
            .expect("create album")
    }

    async fn create_test_track(
        state: &AppState,
        artist: &chorrosion_domain::Artist,
        album: &chorrosion_domain::Album,
    ) -> chorrosion_domain::Track {
        state
            .track_repository
            .create(chorrosion_domain::Track::new(
                album.id,
                artist.id,
                "Test Track",
            ))
            .await
            .expect("create track")
    }

    #[tokio::test]
    async fn list_wanted_albums_returns_empty_when_none() {
        let state = make_test_state().await;
        let result = list_wanted_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");
        assert_eq!(result.0.total, 0);
        assert!(result.0.items.is_empty());
    }

    #[tokio::test]
    async fn list_wanted_albums_returns_only_wanted() {
        let state = make_test_state().await;
        let artist = create_test_artist(&state).await;
        create_test_album(&state, &artist, AlbumStatus::Wanted).await;
        create_test_album(&state, &artist, AlbumStatus::Released).await;

        let result = list_wanted_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");
        assert_eq!(result.0.total, 1);
        assert_eq!(result.0.items[0].status, "wanted");
    }

    #[tokio::test]
    async fn list_missing_albums_returns_empty_when_none() {
        let state = make_test_state().await;
        let result = list_missing_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");
        assert_eq!(result.0.total, 0);
        assert!(result.0.items.is_empty());
    }

    #[tokio::test]
    async fn list_missing_albums_returns_wanted_with_no_tracks() {
        let state = make_test_state().await;
        let artist = create_test_artist(&state).await;
        // This album has no tracks → it's "missing"
        create_test_album(&state, &artist, AlbumStatus::Wanted).await;
        // A released album should NOT appear
        create_test_album(&state, &artist, AlbumStatus::Released).await;

        let result = list_missing_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");
        assert_eq!(result.0.total, 1);
        assert_eq!(result.0.items[0].status, "wanted");
    }

    #[tokio::test]
    async fn list_missing_albums_excludes_wanted_with_tracks() {
        let state = make_test_state().await;
        let artist = create_test_artist(&state).await;
        // Create a wanted album that WILL have at least one track.
        let album = create_test_album(&state, &artist, AlbumStatus::Wanted).await;
        // Attach a track to the wanted album so it should NOT be considered "missing".
        create_test_track(&state, &artist, &album).await;

        let result = list_missing_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");
        // The wanted album has at least one track, so it must not be returned as "missing".
        assert_eq!(result.0.total, 0);
        assert!(result.0.items.is_empty());
    }

    #[tokio::test]
    async fn list_wanted_albums_rejects_invalid_limit() {
        let state = make_test_state().await;
        let err = list_wanted_albums(
            State(state),
            Query(WantedQuery {
                limit: 0,
                offset: 0,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_wanted_albums_rejects_negative_offset() {
        let state = make_test_state().await;
        let err = list_wanted_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: -1,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_cutoff_unmet_albums_returns_empty_when_none() {
        let state = make_test_state().await;
        let result = list_cutoff_unmet_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");
        assert_eq!(result.0.total, 0);
        assert!(result.0.items.is_empty());
    }

    #[tokio::test]
    async fn list_cutoff_unmet_albums_rejects_invalid_limit() {
        let state = make_test_state().await;
        let err = list_cutoff_unmet_albums(
            State(state),
            Query(WantedQuery {
                limit: 0,
                offset: 0,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_cutoff_unmet_albums_detects_below_cutoff() {
        let (pool, state) = make_test_pool_and_state().await;
        let track_file_repo = SqliteTrackFileRepository::new(pool);

        // Set up a quality profile: FLAC is best, MP3 is worse; cutoff is FLAC
        let mut profile = chorrosion_domain::QualityProfile::new(
            "HD Audio",
            vec!["FLAC".to_string(), "MP3".to_string()],
        );
        profile.upgrade_allowed = true;
        profile.cutoff_quality = Some("FLAC".to_string());
        let profile = state
            .quality_profile_repository
            .create(profile)
            .await
            .expect("create quality profile");

        // Create an artist assigned to this quality profile
        let mut artist = chorrosion_domain::Artist::new("Cutoff Artist");
        artist.quality_profile_id = Some(profile.id);
        let artist = state
            .artist_repository
            .create(artist)
            .await
            .expect("create artist");

        // Create a monitored album (monitored = true by default in Album::new)
        let album = state
            .album_repository
            .create(chorrosion_domain::Album::new(
                artist.id,
                "Below Cutoff Album",
            ))
            .await
            .expect("create album");

        // Create a monitored track (monitored = true by default in Track::new)
        let track = state
            .track_repository
            .create(chorrosion_domain::Track::new(
                album.id,
                artist.id,
                "Track One",
            ))
            .await
            .expect("create track");

        // Attach a track file with codec MP3 (below FLAC cutoff)
        let mut track_file = chorrosion_domain::TrackFile::new(track.id, "/music/track.mp3", 1024);
        track_file.codec = Some("MP3".to_string());
        track_file_repo
            .create(track_file)
            .await
            .expect("create track file");

        let result = list_cutoff_unmet_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");

        assert_eq!(
            result.0.total, 1,
            "album with MP3 file below FLAC cutoff should be listed"
        );
        assert_eq!(result.0.items[0].title, "Below Cutoff Album");
    }

    #[tokio::test]
    async fn list_cutoff_unmet_albums_excludes_when_cutoff_met() {
        let (pool, state) = make_test_pool_and_state().await;
        let track_file_repo = SqliteTrackFileRepository::new(pool);

        // Set up a quality profile: FLAC is best; cutoff is FLAC
        let mut profile = chorrosion_domain::QualityProfile::new(
            "Lossless Only",
            vec!["FLAC".to_string(), "MP3".to_string()],
        );
        profile.upgrade_allowed = true;
        profile.cutoff_quality = Some("FLAC".to_string());
        let profile = state
            .quality_profile_repository
            .create(profile)
            .await
            .expect("create quality profile");

        let mut artist = chorrosion_domain::Artist::new("Lossless Artist");
        artist.quality_profile_id = Some(profile.id);
        let artist = state
            .artist_repository
            .create(artist)
            .await
            .expect("create artist");

        let album = state
            .album_repository
            .create(chorrosion_domain::Album::new(
                artist.id,
                "Meets Cutoff Album",
            ))
            .await
            .expect("create album");

        let track = state
            .track_repository
            .create(chorrosion_domain::Track::new(
                album.id,
                artist.id,
                "FLAC Track",
            ))
            .await
            .expect("create track");

        // Attach a track file with codec matching the cutoff (case-insensitive: lowercase "flac")
        let mut track_file = chorrosion_domain::TrackFile::new(track.id, "/music/track.flac", 2048);
        track_file.codec = Some("flac".to_string());
        track_file_repo
            .create(track_file)
            .await
            .expect("create track file");

        let result = list_cutoff_unmet_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");

        assert_eq!(
            result.0.total, 0,
            "album with FLAC file meeting the cutoff should not be listed"
        );
    }

    #[tokio::test]
    async fn list_cutoff_unmet_albums_detects_when_cutoff_not_in_allowed() {
        // When cutoff_quality is not present in allowed_qualities (inconsistent profile),
        // the album should be treated as cutoff-unmet rather than silently excluded.
        let (pool, state) = make_test_pool_and_state().await;
        let track_file_repo = SqliteTrackFileRepository::new(pool);

        // Profile with cutoff_quality ("DSD") that is NOT in allowed_qualities
        let mut profile = chorrosion_domain::QualityProfile::new(
            "Broken Profile",
            vec!["FLAC".to_string(), "MP3".to_string()],
        );
        profile.upgrade_allowed = true;
        // Deliberately set a cutoff_quality not present in allowed_qualities
        profile.cutoff_quality = Some("DSD".to_string());
        let profile = state
            .quality_profile_repository
            .create(profile)
            .await
            .expect("create quality profile");

        let mut artist = chorrosion_domain::Artist::new("Inconsistent Artist");
        artist.quality_profile_id = Some(profile.id);
        let artist = state
            .artist_repository
            .create(artist)
            .await
            .expect("create artist");

        let album = state
            .album_repository
            .create(chorrosion_domain::Album::new(
                artist.id,
                "Inconsistent Cutoff Album",
            ))
            .await
            .expect("create album");

        let track = state
            .track_repository
            .create(chorrosion_domain::Track::new(
                album.id,
                artist.id,
                "Some Track",
            ))
            .await
            .expect("create track");

        // Track file with a valid codec (FLAC) — would be fine if cutoff were valid
        let mut track_file = chorrosion_domain::TrackFile::new(track.id, "/music/track.flac", 4096);
        track_file.codec = Some("FLAC".to_string());
        track_file_repo
            .create(track_file)
            .await
            .expect("create track file");

        let result = list_cutoff_unmet_albums(
            State(state),
            Query(WantedQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("should succeed");

        assert_eq!(
            result.0.total, 1,
            "album should be listed when cutoff_quality is absent from allowed_qualities"
        );
        assert_eq!(result.0.items[0].title, "Inconsistent Cutoff Album");
    }

    #[tokio::test]
    async fn trigger_wanted_album_search_returns_202_for_wanted_album() {
        let state = make_test_state().await;
        let artist = create_test_artist(&state).await;
        let album = create_test_album(&state, &artist, AlbumStatus::Wanted).await;

        let response = trigger_wanted_album_search(State(state), Path(album.id.to_string()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn trigger_wanted_album_search_returns_404_for_unknown_album() {
        let state = make_test_state().await;
        let response = trigger_wanted_album_search(
            State(state),
            Path("00000000-0000-0000-0000-000000000000".to_string()),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn trigger_wanted_album_search_returns_409_for_non_wanted_album() {
        let state = make_test_state().await;
        let artist = create_test_artist(&state).await;
        let album = create_test_album(&state, &artist, AlbumStatus::Released).await;

        let response = trigger_wanted_album_search(State(state), Path(album.id.to_string()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn trigger_wanted_album_search_falls_back_to_title_when_artist_missing() {
        use axum::body::to_bytes;
        let (pool, state) = make_test_pool_and_state().await;
        // Insert a wanted album whose artist_id points to a non-existent artist.
        // Acquire a single connection so FK-related PRAGMAs apply to the same connection
        // as the INSERT (the pool has max_connections=1, but be explicit for clarity).
        let fake_artist_id = chorrosion_domain::ArtistId::new().to_string();
        let album_id = chorrosion_domain::AlbumId::new().to_string();
        let title = "Orphaned Album";
        let mut conn = pool.acquire().await.expect("acquire connection");
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .expect("disable FK");
        sqlx::query(
            "INSERT INTO albums (id, artist_id, title, status, monitored) VALUES (?, ?, ?, 'wanted', 1)",
        )
        .bind(&album_id)
        .bind(&fake_artist_id)
        .bind(title)
        .execute(&mut *conn)
        .await
        .expect("insert orphaned album");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await
            .expect("re-enable FK");
        drop(conn);

        let response = trigger_wanted_album_search(State(state), Path(album_id))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body_bytes = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read body");
        let body: WantedManualSearchResponse =
            serde_json::from_slice(&body_bytes).expect("deserialize");
        assert_eq!(
            body.query, title,
            "query should be just the album title when artist is missing"
        );
    }
}
