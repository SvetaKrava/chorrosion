// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::{ArtistId, Track};
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListTracksQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TrackResponse {
    pub id: String,
    pub album_id: String,
    pub artist_id: String,
    pub foreign_track_id: Option<String>,
    pub title: String,
    pub track_number: Option<u32>,
    pub duration_ms: Option<u32>,
    pub has_file: bool,
    pub monitored: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListTracksResponse {
    pub items: Vec<TrackResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl From<Track> for TrackResponse {
    fn from(track: Track) -> Self {
        Self {
            id: track.id.to_string(),
            album_id: track.album_id.to_string(),
            artist_id: track.artist_id.to_string(),
            foreign_track_id: track.foreign_track_id,
            title: track.title,
            track_number: track.track_number,
            duration_ms: track.duration_ms,
            has_file: track.has_file,
            monitored: track.monitored,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTrackRequest {
    pub album_id: String,
    pub artist_id: String,
    pub title: String,
    pub foreign_track_id: Option<String>,
    pub track_number: Option<u32>,
    pub duration_ms: Option<u32>,
    pub has_file: Option<bool>,
    pub monitored: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTrackRequest {
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    pub title: Option<String>,
    pub foreign_track_id: Option<String>,
    pub track_number: Option<u32>,
    pub duration_ms: Option<u32>,
    pub has_file: Option<bool>,
    pub monitored: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = TrackErrorResponse)]
pub struct ErrorResponse {
    pub error: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/tracks",
    params(ListTracksQuery),
    responses(
        (status = 200, description = "List of tracks", body = ListTracksResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "tracks"
)]
pub async fn list_tracks(
    State(state): State<AppState>,
    Query(query): Query<ListTracksQuery>,
) -> Result<Json<ListTracksResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", ?query, "listing tracks");

    if !(1..=500).contains(&query.limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "limit must be between 1 and 500".to_string(),
            }),
        ));
    }

    if query.offset < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "offset must be greater than or equal to 0".to_string(),
            }),
        ));
    }

    // Load a bounded full page for total + stable in-memory pagination.
    let all_tracks = state
        .track_repository
        .list(5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list tracks: {error}"),
                }),
            )
        })?;

    let total = all_tracks.len() as i64;
    // query.offset is validated >= 0 above, so try_from only fails on 32-bit
    // targets when offset exceeds usize::MAX.  In that case the offset value
    // is clearly out of range, so return a 400 rather than silently resetting
    // to 0 or returning a misleadingly empty 200.
    let offset = match usize::try_from(query.offset) {
        Ok(o) => o,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "offset is out of range".to_string(),
                }),
            ));
        }
    };
    // limit is validated to be in [1, 500], so the cast is always safe.
    let limit = query.limit as usize;
    let items = all_tracks
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(TrackResponse::from)
        .collect();

    Ok(Json(ListTracksResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/albums/{album_id}/tracks",
    params(
        ("album_id" = String, Path, description = "Album ID"),
        ListTracksQuery
    ),
    responses(
        (status = 200, description = "List tracks for album", body = ListTracksResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Album not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "tracks"
)]
pub async fn list_tracks_by_album(
    State(state): State<AppState>,
    Path(album_id): Path<String>,
    Query(query): Query<ListTracksQuery>,
) -> Result<Json<ListTracksResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", %album_id, ?query, "listing tracks by album");

    if !(1..=500).contains(&query.limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "limit must be between 1 and 500".to_string(),
            }),
        ));
    }

    if query.offset < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "offset must be greater than or equal to 0".to_string(),
            }),
        ));
    }

    let album = state
        .album_repository
        .get_by_id(&album_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch album: {error}"),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Album {album_id} not found"),
                }),
            )
        })?;

    let all_tracks = state
        .track_repository
        .get_by_album(album.id, 5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list tracks by album: {error}"),
                }),
            )
        })?;

    let total = all_tracks.len() as i64;
    let offset = match usize::try_from(query.offset) {
        Ok(o) => o,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "offset is out of range".to_string(),
                }),
            ));
        }
    };
    let limit = query.limit as usize;
    let items = all_tracks
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(TrackResponse::from)
        .collect();

    Ok(Json(ListTracksResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/artists/{artist_id}/tracks",
    params(
        ("artist_id" = String, Path, description = "Artist ID"),
        ListTracksQuery
    ),
    responses(
        (status = 200, description = "List tracks for artist", body = ListTracksResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Artist not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "tracks"
)]
pub async fn list_tracks_by_artist(
    State(state): State<AppState>,
    Path(artist_id): Path<String>,
    Query(query): Query<ListTracksQuery>,
) -> Result<Json<ListTracksResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", %artist_id, ?query, "listing tracks by artist");

    if !(1..=500).contains(&query.limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "limit must be between 1 and 500".to_string(),
            }),
        ));
    }

    if query.offset < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "offset must be greater than or equal to 0".to_string(),
            }),
        ));
    }

    let artist = state
        .artist_repository
        .get_by_id(&artist_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch artist: {error}"),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Artist {artist_id} not found"),
                }),
            )
        })?;

    let all_tracks = state
        .track_repository
        .get_by_artist(artist.id, 5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list tracks by artist: {error}"),
                }),
            )
        })?;

    let total = all_tracks.len() as i64;
    let offset = match usize::try_from(query.offset) {
        Ok(o) => o,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "offset is out of range".to_string(),
                }),
            ));
        }
    };
    let limit = query.limit as usize;
    let items = all_tracks
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(TrackResponse::from)
        .collect();

    Ok(Json(ListTracksResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/tracks/{id}",
    params(
        ("id" = String, Path, description = "Track ID")
    ),
    responses(
        (status = 200, description = "Track found", body = TrackResponse),
        (status = 404, description = "Track not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "tracks"
)]
pub async fn get_track(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    debug!(target: "api", %id, "fetching track");

    match state.track_repository.get_by_id(&id).await {
        Ok(Some(track)) => (StatusCode::OK, Json(TrackResponse::from(track))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Track {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch track: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/tracks",
    request_body = CreateTrackRequest,
    responses(
        (status = 201, description = "Track created", body = TrackResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Album or artist not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "tracks"
)]
pub async fn create_track(
    State(state): State<AppState>,
    Json(request): Json<CreateTrackRequest>,
) -> impl IntoResponse {
    debug!(target: "api", ?request, "creating track");

    let album = match state.album_repository.get_by_id(&request.album_id).await {
        Ok(Some(album)) => album,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Album {} not found", request.album_id),
                }),
            )
                .into_response()
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch album: {error}"),
                }),
            )
                .into_response()
        }
    };

    let artist = match state.artist_repository.get_by_id(&request.artist_id).await {
        Ok(Some(artist)) => artist,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Artist {} not found", request.artist_id),
                }),
            )
                .into_response()
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch artist: {error}"),
                }),
            )
                .into_response()
        }
    };

    if album.artist_id != artist.id {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "album_id and artist_id must reference the same artist".to_string(),
            }),
        )
            .into_response();
    }

    let mut track = Track::new(album.id, artist.id, request.title);
    track.foreign_track_id = request.foreign_track_id;
    track.track_number = request.track_number;
    track.duration_ms = request.duration_ms;
    track.has_file = request.has_file.unwrap_or(false);
    track.monitored = request.monitored.unwrap_or(true);

    match state.track_repository.create(track).await {
        Ok(created) => (StatusCode::CREATED, Json(TrackResponse::from(created))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to create track: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/tracks/{id}",
    params(
        ("id" = String, Path, description = "Track ID")
    ),
    request_body = UpdateTrackRequest,
    responses(
        (status = 200, description = "Track updated", body = TrackResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Track, album, or artist not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "tracks"
)]
pub async fn update_track(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateTrackRequest>,
) -> impl IntoResponse {
    debug!(target: "api", %id, ?request, "updating track");

    let UpdateTrackRequest {
        album_id,
        artist_id,
        title,
        foreign_track_id,
        track_number,
        duration_ms,
        has_file,
        monitored,
    } = request;

    let mut track = match state.track_repository.get_by_id(&id).await {
        Ok(Some(track)) => track,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Track {} not found", id),
                }),
            )
                .into_response()
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch track: {error}"),
                }),
            )
                .into_response()
        }
    };

    // Whether we need to validate album/artist consistency after applying changes.
    let album_id_provided = album_id.is_some();
    let artist_id_provided = artist_id.is_some();

    // Carry the album's artist_id forward when album_id is in the request so
    // we can skip a second DB round-trip during the consistency check below.
    let mut fetched_album_artist_id: Option<ArtistId> = None;

    if let Some(album_id) = album_id {
        match state.album_repository.get_by_id(&album_id).await {
            Ok(Some(album)) => {
                fetched_album_artist_id = Some(album.artist_id);
                track.album_id = album.id;
                if !artist_id_provided {
                    track.artist_id = album.artist_id;
                }
            }
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("Album {} not found", album_id),
                    }),
                )
                    .into_response()
            }
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to fetch album: {error}"),
                    }),
                )
                    .into_response()
            }
        }
    }

    if let Some(artist_id) = artist_id {
        match state.artist_repository.get_by_id(&artist_id).await {
            Ok(Some(artist)) => {
                track.artist_id = artist.id;
            }
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("Artist {} not found", artist_id),
                    }),
                )
                    .into_response()
            }
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to fetch artist: {error}"),
                    }),
                )
                    .into_response()
            }
        }
    }

    // Validate album/artist consistency when either field changed.
    // Reuse the already-fetched album artist_id to avoid a second DB query
    // when album_id was part of the request.
    if album_id_provided || artist_id_provided {
        let album_artist_id = match fetched_album_artist_id {
            Some(id) => id,
            None => {
                // Only artist_id changed; fetch the album to get its artist_id.
                match state
                    .album_repository
                    .get_by_id(&track.album_id.to_string())
                    .await
                {
                    Ok(Some(album)) => album.artist_id,
                    Ok(None) => {
                        return (
                            StatusCode::NOT_FOUND,
                            Json(ErrorResponse {
                                error: "current album for track was not found".to_string(),
                            }),
                        )
                            .into_response()
                    }
                    Err(error) => {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: format!("failed to re-validate album: {error}"),
                            }),
                        )
                            .into_response()
                    }
                }
            }
        };
        if album_artist_id != track.artist_id {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "album_id and artist_id must reference the same artist".to_string(),
                }),
            )
                .into_response();
        }
    }

    if let Some(title) = title {
        track.title = title;
    }
    if let Some(foreign_track_id) = foreign_track_id {
        track.foreign_track_id = Some(foreign_track_id);
    }
    if let Some(track_number) = track_number {
        track.track_number = Some(track_number);
    }
    if let Some(duration_ms) = duration_ms {
        track.duration_ms = Some(duration_ms);
    }
    if let Some(has_file) = has_file {
        track.has_file = has_file;
    }
    if let Some(monitored) = monitored {
        track.monitored = monitored;
    }

    match state.track_repository.update(track).await {
        Ok(updated) => (StatusCode::OK, Json(TrackResponse::from(updated))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to update track: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/tracks/{id}",
    params(
        ("id" = String, Path, description = "Track ID")
    ),
    responses(
        (status = 204, description = "Track deleted"),
        (status = 404, description = "Track not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "tracks"
)]
pub async fn delete_track(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "deleting track");

    match state.track_repository.get_by_id(&id).await {
        Ok(Some(_)) => {
            match state.track_repository.delete(&id).await {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(delete_error) => {
                    // Check if the track was concurrently deleted before we could.
                    match state.track_repository.get_by_id(&id).await {
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            Json(ErrorResponse {
                                error: format!("Track {} not found", id),
                            }),
                        )
                            .into_response(),
                        Ok(Some(_)) | Err(_) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: format!("failed to delete track: {delete_error}"),
                            }),
                        )
                            .into_response(),
                    }
                }
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Track {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch track before delete: {error}"),
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Write handler tests (async, in-memory SQLite)
    // ============================================================================

    mod write_handlers {
        use super::*;
        use axum::extract::{Path, Query, State};
        use axum::response::IntoResponse;
        use chorrosion_config::AppConfig;
        use chorrosion_domain::{Album, Artist};
        use chorrosion_infrastructure::sqlite_adapters::{
            SqliteAlbumRepository, SqliteArtistRepository,
            SqliteDownloadClientDefinitionRepository, SqliteIndexerDefinitionRepository,
            SqliteMetadataProfileRepository, SqliteQualityProfileRepository, SqliteTagRepository,
            SqliteTaggedEntityRepository, SqliteTrackRepository,
        };
        use std::sync::Arc;

        async fn make_test_state() -> AppState {
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
            AppState::new(
                AppConfig::default(),
                Arc::new(SqliteArtistRepository::new(pool.clone())),
                Arc::new(SqliteAlbumRepository::new(pool.clone())),
                Arc::new(SqliteTrackRepository::new(pool.clone())),
                Arc::new(SqliteQualityProfileRepository::new(pool.clone())),
                Arc::new(SqliteMetadataProfileRepository::new(pool.clone())),
                Arc::new(SqliteIndexerDefinitionRepository::new(pool.clone())),
                Arc::new(SqliteDownloadClientDefinitionRepository::new(pool.clone())),
                Arc::new(SqliteTagRepository::new(pool.clone())),
                Arc::new(SqliteTaggedEntityRepository::new(pool.clone())),
                Arc::new(
                    chorrosion_infrastructure::sqlite_adapters::SqliteSmartPlaylistRepository::new(
                        pool.clone(),
                    ),
                ),
                chorrosion_infrastructure::ResponseCache::new(100, 60),
            )
        }

        async fn create_test_artist(state: &AppState) -> Artist {
            state
                .artist_repository
                .create(Artist::new("Test Artist"))
                .await
                .expect("create test artist")
        }

        async fn create_test_album(state: &AppState, artist: &Artist) -> Album {
            state
                .album_repository
                .create(Album::new(artist.id, "Test Album"))
                .await
                .expect("create test album")
        }

        // --- list_tracks ---

        #[tokio::test]
        async fn list_tracks_returns_empty_when_no_tracks() {
            let state = make_test_state().await;
            let query = ListTracksQuery {
                limit: 10,
                offset: 0,
            };
            let result = list_tracks(State(state), Query(query)).await.unwrap();
            assert_eq!(result.total, 0);
            assert!(result.items.is_empty());
        }

        #[tokio::test]
        async fn list_tracks_rejects_invalid_limit() {
            let state = make_test_state().await;
            let query = ListTracksQuery {
                limit: 0,
                offset: 0,
            };
            let result = list_tracks(State(state), Query(query)).await;
            assert!(result.is_err());
            let (status, _) = result.unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn list_tracks_rejects_negative_offset() {
            let state = make_test_state().await;
            let query = ListTracksQuery {
                limit: 10,
                offset: -1,
            };
            let result = list_tracks(State(state), Query(query)).await;
            assert!(result.is_err());
            let (status, _) = result.unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn list_tracks_returns_accurate_total_with_pagination() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist).await;
            for title in ["Track A", "Track B", "Track C"] {
                state
                    .track_repository
                    .create(Track::new(album.id, artist.id, title))
                    .await
                    .unwrap();
            }
            let query = ListTracksQuery {
                limit: 2,
                offset: 0,
            };
            let result = list_tracks(State(state), Query(query)).await.unwrap();
            assert_eq!(result.total, 3);
            assert_eq!(result.items.len(), 2);
        }

        #[tokio::test]
        async fn list_tracks_by_album_filters_results() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album_one = create_test_album(&state, &artist).await;
            let album_two = state
                .album_repository
                .create(Album::new(artist.id, "Second Album"))
                .await
                .unwrap();
            state
                .track_repository
                .create(Track::new(album_one.id, artist.id, "Track A"))
                .await
                .unwrap();
            state
                .track_repository
                .create(Track::new(album_two.id, artist.id, "Track B"))
                .await
                .unwrap();

            let result = list_tracks_by_album(
                State(state),
                Path(album_one.id.to_string()),
                Query(ListTracksQuery {
                    limit: 10,
                    offset: 0,
                }),
            )
            .await
            .unwrap();

            assert_eq!(result.total, 1);
            assert_eq!(result.items.len(), 1);
            assert_eq!(result.items[0].title, "Track A");
        }

        #[tokio::test]
        async fn list_tracks_by_album_returns_404_for_unknown_album() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let result = list_tracks_by_album(
                State(state),
                Path(unknown_id.clone()),
                Query(ListTracksQuery {
                    limit: 10,
                    offset: 0,
                }),
            )
            .await;
            assert!(result.is_err());
            let (status, Json(body)) = result.unwrap_err();
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(body.error, format!("Album {unknown_id} not found"));
        }

        #[tokio::test]
        async fn list_tracks_by_artist_filters_results() {
            let state = make_test_state().await;
            let artist_one = create_test_artist(&state).await;
            let artist_two = state
                .artist_repository
                .create(Artist::new("Another Artist"))
                .await
                .unwrap();
            let album_one = create_test_album(&state, &artist_one).await;
            let album_two = state
                .album_repository
                .create(Album::new(artist_two.id, "Other Album"))
                .await
                .unwrap();
            state
                .track_repository
                .create(Track::new(album_one.id, artist_one.id, "Track A"))
                .await
                .unwrap();
            state
                .track_repository
                .create(Track::new(album_two.id, artist_two.id, "Track B"))
                .await
                .unwrap();

            let result = list_tracks_by_artist(
                State(state),
                Path(artist_one.id.to_string()),
                Query(ListTracksQuery {
                    limit: 10,
                    offset: 0,
                }),
            )
            .await
            .unwrap();

            assert_eq!(result.total, 1);
            assert_eq!(result.items.len(), 1);
            assert_eq!(result.items[0].title, "Track A");
        }

        #[tokio::test]
        async fn list_tracks_by_artist_returns_404_for_unknown_artist() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let result = list_tracks_by_artist(
                State(state),
                Path(unknown_id.clone()),
                Query(ListTracksQuery {
                    limit: 10,
                    offset: 0,
                }),
            )
            .await;
            assert!(result.is_err());
            let (status, Json(body)) = result.unwrap_err();
            assert_eq!(status, StatusCode::NOT_FOUND);
            assert_eq!(body.error, format!("Artist {unknown_id} not found"));
        }

        // --- get_track ---

        #[tokio::test]
        async fn get_track_returns_200_on_success() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist).await;
            let track = state
                .track_repository
                .create(Track::new(album.id, artist.id, "My Track"))
                .await
                .unwrap();
            let response = get_track(State(state), Path(track.id.to_string()))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn get_track_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = get_track(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // --- create_track ---

        #[tokio::test]
        async fn create_track_returns_201_on_success() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist).await;
            let request = CreateTrackRequest {
                album_id: album.id.to_string(),
                artist_id: artist.id.to_string(),
                title: "New Track".to_string(),
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = create_track(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        #[tokio::test]
        async fn create_track_returns_404_for_unknown_album() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let request = CreateTrackRequest {
                album_id: "00000000-0000-0000-0000-000000000000".to_string(),
                artist_id: artist.id.to_string(),
                title: "New Track".to_string(),
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = create_track(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn create_track_returns_404_for_unknown_artist() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist).await;
            let request = CreateTrackRequest {
                album_id: album.id.to_string(),
                artist_id: "00000000-0000-0000-0000-000000000000".to_string(),
                title: "New Track".to_string(),
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = create_track(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn create_track_returns_400_when_album_artist_mismatch() {
            let state = make_test_state().await;
            let artist1 = create_test_artist(&state).await;
            let artist2 = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist1).await;
            // album belongs to artist1, but request references artist2
            let request = CreateTrackRequest {
                album_id: album.id.to_string(),
                artist_id: artist2.id.to_string(),
                title: "Mismatch Track".to_string(),
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = create_track(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        // --- update_track ---

        #[tokio::test]
        async fn update_track_returns_200_on_success() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist).await;
            let track = state
                .track_repository
                .create(Track::new(album.id, artist.id, "Before"))
                .await
                .unwrap();
            let request = UpdateTrackRequest {
                album_id: None,
                artist_id: None,
                title: Some("After".to_string()),
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = update_track(State(state), Path(track.id.to_string()), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn update_track_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let request = UpdateTrackRequest {
                album_id: None,
                artist_id: None,
                title: Some("Title".to_string()),
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = update_track(State(state), Path(unknown_id), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn update_track_returns_404_for_unknown_album() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist).await;
            let track = state
                .track_repository
                .create(Track::new(album.id, artist.id, "Track"))
                .await
                .unwrap();
            let request = UpdateTrackRequest {
                album_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
                artist_id: None,
                title: None,
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = update_track(State(state), Path(track.id.to_string()), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn update_track_returns_404_for_unknown_artist() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist).await;
            let track = state
                .track_repository
                .create(Track::new(album.id, artist.id, "Track"))
                .await
                .unwrap();
            let request = UpdateTrackRequest {
                album_id: None,
                artist_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
                title: None,
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = update_track(State(state), Path(track.id.to_string()), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn update_track_returns_400_when_album_artist_mismatch() {
            let state = make_test_state().await;
            let artist1 = create_test_artist(&state).await;
            let artist2 = create_test_artist(&state).await;
            let album1 = create_test_album(&state, &artist1).await;
            let track = state
                .track_repository
                .create(Track::new(album1.id, artist1.id, "Track"))
                .await
                .unwrap();
            // Switch only the artist_id to artist2, which doesn't own album1
            let request = UpdateTrackRequest {
                album_id: None,
                artist_id: Some(artist2.id.to_string()),
                title: None,
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = update_track(State(state), Path(track.id.to_string()), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn update_track_returns_200_when_album_id_changed_for_same_artist() {
            // When album_id is changed to another album owned by the same artist,
            // the update should succeed and return 200 OK.
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album1 = create_test_album(&state, &artist).await;
            let album2 = create_test_album(&state, &artist).await;
            let track = state
                .track_repository
                .create(Track::new(album1.id, artist.id, "Track"))
                .await
                .unwrap();
            let request = UpdateTrackRequest {
                album_id: Some(album2.id.to_string()),
                artist_id: None,
                title: None,
                foreign_track_id: None,
                track_number: None,
                duration_ms: None,
                has_file: None,
                monitored: None,
            };
            let response = update_track(State(state), Path(track.id.to_string()), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        // --- delete_track ---

        #[tokio::test]
        async fn delete_track_returns_204_on_success() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = create_test_album(&state, &artist).await;
            let track = state
                .track_repository
                .create(Track::new(album.id, artist.id, "To Delete"))
                .await
                .unwrap();
            let response = delete_track(State(state), Path(track.id.to_string()))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }

        #[tokio::test]
        async fn delete_track_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = delete_track(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }
}
