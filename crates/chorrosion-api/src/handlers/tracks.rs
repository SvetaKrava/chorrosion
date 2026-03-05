// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::Track;
use chorrosion_infrastructure::repositories::Repository;
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
    let offset = usize::try_from(query.offset).unwrap_or(0);
    let limit = usize::try_from(query.limit).unwrap_or(50);
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

    match state.track_repository.get_by_id(id.clone()).await {
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

    let album = match state
        .album_repository
        .get_by_id(request.album_id.clone())
        .await
    {
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

    let artist = match state
        .artist_repository
        .get_by_id(request.artist_id.clone())
        .await
    {
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

    let mut track = match state.track_repository.get_by_id(id.clone()).await {
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

    if let Some(album_id) = album_id {
        match state.album_repository.get_by_id(album_id.clone()).await {
            Ok(Some(album)) => {
                track.album_id = album.id;
                if artist_id.is_none() {
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
        match state.artist_repository.get_by_id(artist_id.clone()).await {
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

    match state
        .album_repository
        .get_by_id(track.album_id.to_string())
        .await
    {
        Ok(Some(album)) => {
            if album.artist_id != track.artist_id {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "album_id and artist_id must reference the same artist".to_string(),
                    }),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "album for track was not found".to_string(),
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

    match state.track_repository.delete(id.clone()).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            let err_msg = error.to_string();
            if err_msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("Track {} not found", id),
                    }),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to delete track: {error}"),
                    }),
                )
                    .into_response()
            }
        }
    }
}
