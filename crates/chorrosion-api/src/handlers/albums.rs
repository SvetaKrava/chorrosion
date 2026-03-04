// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::{Album, AlbumStatus};
use chorrosion_infrastructure::repositories::Repository;
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListAlbumsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AlbumResponse {
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
pub struct ListAlbumsResponse {
    pub items: Vec<AlbumResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl From<Album> for AlbumResponse {
    fn from(album: Album) -> Self {
        Self {
            id: album.id.to_string(),
            artist_id: album.artist_id.to_string(),
            foreign_album_id: album.foreign_album_id,
            title: album.title,
            release_date: album
                .release_date
                .map(|date| date.format("%Y-%m-%d").to_string()),
            album_type: album.album_type,
            status: album.status.to_string(),
            monitored: album.monitored,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAlbumRequest {
    pub artist_id: String,
    pub title: String,
    pub foreign_album_id: Option<String>,
    pub release_date: Option<String>,
    pub album_type: Option<String>,
    pub status: Option<String>,
    pub monitored: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAlbumRequest {
    pub artist_id: Option<String>,
    pub title: Option<String>,
    pub foreign_album_id: Option<String>,
    pub release_date: Option<String>,
    pub album_type: Option<String>,
    pub status: Option<String>,
    pub monitored: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

fn parse_album_status(status_str: &str) -> Result<AlbumStatus, (StatusCode, Json<ErrorResponse>)> {
    match status_str.to_ascii_lowercase().as_str() {
        "wanted" => Ok(AlbumStatus::Wanted),
        "released" => Ok(AlbumStatus::Released),
        "announced" => Ok(AlbumStatus::Announced),
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("invalid status value: {status_str}"),
            }),
        )),
    }
}

fn parse_release_date(
    date_str: &str,
) -> Result<chrono::NaiveDate, (StatusCode, Json<ErrorResponse>)> {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "release_date must use YYYY-MM-DD format".to_string(),
            }),
        )
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/albums",
    params(ListAlbumsQuery),
    responses(
        (status = 200, description = "List of albums", body = ListAlbumsResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "albums"
)]
pub async fn list_albums(
    State(state): State<AppState>,
    Query(query): Query<ListAlbumsQuery>,
) -> Result<Json<ListAlbumsResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", ?query, "listing albums");

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

    let albums = state
        .album_repository
        .list(query.limit, query.offset)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list albums: {error}"),
                }),
            )
        })?;

    let total = albums.len() as i64;
    let items = albums.into_iter().map(AlbumResponse::from).collect();

    Ok(Json(ListAlbumsResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/albums/{id}",
    params(
        ("id" = String, Path, description = "Album ID")
    ),
    responses(
        (status = 200, description = "Album found", body = AlbumResponse),
        (status = 404, description = "Album not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "albums"
)]
pub async fn get_album(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    debug!(target: "api", %id, "fetching album");

    match state.album_repository.get_by_id(id.clone()).await {
        Ok(Some(album)) => (StatusCode::OK, Json(AlbumResponse::from(album))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Album {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch album: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/albums",
    request_body = CreateAlbumRequest,
    responses(
        (status = 201, description = "Album created", body = AlbumResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Artist not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "albums"
)]
pub async fn create_album(
    State(state): State<AppState>,
    Json(request): Json<CreateAlbumRequest>,
) -> impl IntoResponse {
    debug!(target: "api", ?request, "creating album");

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

    let mut album = Album::new(artist.id, request.title);
    album.foreign_album_id = request.foreign_album_id;
    album.album_type = request.album_type;
    album.monitored = request.monitored.unwrap_or(true);

    if let Some(status_str) = request.status {
        match parse_album_status(&status_str) {
            Ok(status) => album.status = status,
            Err(err_response) => return err_response.into_response(),
        }
    }

    if let Some(date_str) = request.release_date {
        match parse_release_date(&date_str) {
            Ok(date) => album.release_date = Some(date),
            Err(err_response) => return err_response.into_response(),
        }
    }

    match state.album_repository.create(album).await {
        Ok(created) => (StatusCode::CREATED, Json(AlbumResponse::from(created))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to create album: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/albums/{id}",
    params(
        ("id" = String, Path, description = "Album ID")
    ),
    request_body = UpdateAlbumRequest,
    responses(
        (status = 200, description = "Album updated", body = AlbumResponse),
        (status = 404, description = "Album or artist not found", body = ErrorResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "albums"
)]
pub async fn update_album(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateAlbumRequest>,
) -> impl IntoResponse {
    debug!(target: "api", %id, ?request, "updating album");

    let mut album = match state.album_repository.get_by_id(id.clone()).await {
        Ok(Some(album)) => album,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Album {} not found", id),
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

    if let Some(artist_id) = request.artist_id {
        match state.artist_repository.get_by_id(artist_id.clone()).await {
            Ok(Some(artist)) => {
                album.artist_id = artist.id;
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

    if let Some(title) = request.title {
        album.title = title;
    }
    if let Some(foreign_album_id) = request.foreign_album_id {
        album.foreign_album_id = Some(foreign_album_id);
    }
    if let Some(album_type) = request.album_type {
        album.album_type = Some(album_type);
    }
    if let Some(monitored) = request.monitored {
        album.monitored = monitored;
    }
    if let Some(status_str) = request.status {
        match parse_album_status(&status_str) {
            Ok(status) => album.status = status,
            Err(err_response) => return err_response.into_response(),
        }
    }
    if let Some(date_str) = request.release_date {
        match parse_release_date(&date_str) {
            Ok(date) => album.release_date = Some(date),
            Err(err_response) => return err_response.into_response(),
        }
    }

    match state.album_repository.update(album).await {
        Ok(updated) => (StatusCode::OK, Json(AlbumResponse::from(updated))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to update album: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/albums/{id}",
    params(
        ("id" = String, Path, description = "Album ID")
    ),
    responses(
        (status = 204, description = "Album deleted"),
        (status = 404, description = "Album not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "albums"
)]
pub async fn delete_album(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "deleting album");

    match state.album_repository.delete(id.clone()).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            let err_msg = error.to_string();
            if err_msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("Album {} not found", id),
                    }),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to delete album: {error}"),
                    }),
                )
                    .into_response()
            }
        }
    }
}
