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
#[schema(as = AlbumErrorResponse)]
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

    // Load all albums and paginate in memory to compute an accurate total count.
    let all_albums = state
        .album_repository
        .list(5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list albums: {error}"),
                }),
            )
        })?;

    let total = all_albums.len() as i64;
    let offset = usize::try_from(query.offset).unwrap_or(0);
    let limit = usize::try_from(query.limit).unwrap_or(50);
    let items = all_albums
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(AlbumResponse::from)
        .collect();

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

    match state.album_repository.get_by_id(id.clone()).await {
        Ok(Some(_)) => {
            match state.album_repository.delete(id.clone()).await {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(delete_error) => {
                    // Check if the album was concurrently deleted before we could.
                    match state.album_repository.get_by_id(id.clone()).await {
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            Json(ErrorResponse {
                                error: format!("Album {} not found", id),
                            }),
                        )
                            .into_response(),
                        Ok(Some(_)) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: format!("failed to delete album: {delete_error}"),
                            }),
                        )
                            .into_response(),
                        Err(_) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: format!("failed to delete album: {delete_error}"),
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
                error: format!("Album {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch album before delete: {error}"),
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // parse_album_status tests
    // ============================================================================

    #[test]
    fn parse_status_accepts_wanted_case_insensitive() {
        assert!(matches!(
            parse_album_status("wanted"),
            Ok(AlbumStatus::Wanted)
        ));
        assert!(matches!(
            parse_album_status("WANTED"),
            Ok(AlbumStatus::Wanted)
        ));
        assert!(matches!(
            parse_album_status("Wanted"),
            Ok(AlbumStatus::Wanted)
        ));
    }

    #[test]
    fn parse_status_accepts_released_case_insensitive() {
        assert!(matches!(
            parse_album_status("released"),
            Ok(AlbumStatus::Released)
        ));
        assert!(matches!(
            parse_album_status("RELEASED"),
            Ok(AlbumStatus::Released)
        ));
    }

    #[test]
    fn parse_status_accepts_announced_case_insensitive() {
        assert!(matches!(
            parse_album_status("announced"),
            Ok(AlbumStatus::Announced)
        ));
        assert!(matches!(
            parse_album_status("ANNOUNCED"),
            Ok(AlbumStatus::Announced)
        ));
    }

    #[test]
    fn parse_status_rejects_unknown_value() {
        let result = parse_album_status("unknown");
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn parse_status_rejects_empty_string() {
        let result = parse_album_status("");
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // ============================================================================
    // Write handler tests (async, in-memory SQLite)
    // ============================================================================

    mod write_handlers {
        use super::*;
        use axum::extract::{Path, Query, State};
        use axum::response::IntoResponse;
        use chorrosion_config::AppConfig;
        use chorrosion_domain::Artist;
        use chorrosion_infrastructure::sqlite_adapters::{
            SqliteAlbumRepository, SqliteArtistRepository,
            SqliteDownloadClientDefinitionRepository, SqliteIndexerDefinitionRepository,
            SqliteMetadataProfileRepository, SqliteQualityProfileRepository, SqliteTrackRepository,
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
                Arc::new(SqliteDownloadClientDefinitionRepository::new(pool)),
            )
        }

        async fn create_test_artist(state: &AppState) -> Artist {
            state
                .artist_repository
                .create(Artist::new("Test Artist"))
                .await
                .expect("create test artist")
        }

        // --- create_album ---

        #[tokio::test]
        async fn create_album_returns_201_on_success() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let request = CreateAlbumRequest {
                artist_id: artist.id.to_string(),
                title: "Test Album".to_string(),
                foreign_album_id: None,
                release_date: None,
                album_type: None,
                status: None,
                monitored: None,
            };
            let response = create_album(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        #[tokio::test]
        async fn create_album_returns_404_for_unknown_artist() {
            let state = make_test_state().await;
            let request = CreateAlbumRequest {
                artist_id: "00000000-0000-0000-0000-000000000000".to_string(),
                title: "Test Album".to_string(),
                foreign_album_id: None,
                release_date: None,
                album_type: None,
                status: None,
                monitored: None,
            };
            let response = create_album(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn create_album_rejects_invalid_status() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let request = CreateAlbumRequest {
                artist_id: artist.id.to_string(),
                title: "Test Album".to_string(),
                foreign_album_id: None,
                release_date: None,
                album_type: None,
                status: Some("garbage".to_string()),
                monitored: None,
            };
            let response = create_album(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn create_album_rejects_invalid_release_date() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let request = CreateAlbumRequest {
                artist_id: artist.id.to_string(),
                title: "Test Album".to_string(),
                foreign_album_id: None,
                release_date: Some("not-a-date".to_string()),
                album_type: None,
                status: None,
                monitored: None,
            };
            let response = create_album(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        // --- get_album ---

        #[tokio::test]
        async fn get_album_returns_200_on_success() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = state
                .album_repository
                .create(Album::new(artist.id, "My Album"))
                .await
                .unwrap();
            let response = get_album(State(state), Path(album.id.to_string()))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn get_album_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = get_album(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // --- update_album ---

        #[tokio::test]
        async fn update_album_returns_200_on_success() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = state
                .album_repository
                .create(Album::new(artist.id, "Before"))
                .await
                .unwrap();
            let request = UpdateAlbumRequest {
                artist_id: None,
                title: Some("After".to_string()),
                foreign_album_id: None,
                release_date: None,
                album_type: None,
                status: None,
                monitored: None,
            };
            let response = update_album(State(state), Path(album.id.to_string()), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn update_album_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let request = UpdateAlbumRequest {
                artist_id: None,
                title: Some("Title".to_string()),
                foreign_album_id: None,
                release_date: None,
                album_type: None,
                status: None,
                monitored: None,
            };
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = update_album(State(state), Path(unknown_id), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn update_album_returns_404_for_unknown_artist() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = state
                .album_repository
                .create(Album::new(artist.id, "Album"))
                .await
                .unwrap();
            let request = UpdateAlbumRequest {
                artist_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
                title: None,
                foreign_album_id: None,
                release_date: None,
                album_type: None,
                status: None,
                monitored: None,
            };
            let response = update_album(State(state), Path(album.id.to_string()), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // --- delete_album ---

        #[tokio::test]
        async fn delete_album_returns_204_on_success() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            let album = state
                .album_repository
                .create(Album::new(artist.id, "To Delete"))
                .await
                .unwrap();
            let response = delete_album(State(state), Path(album.id.to_string()))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }

        #[tokio::test]
        async fn delete_album_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = delete_album(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // --- list_albums ---

        #[tokio::test]
        async fn list_albums_returns_accurate_total_with_pagination() {
            let state = make_test_state().await;
            let artist = create_test_artist(&state).await;
            for title in ["Album A", "Album B", "Album C"] {
                state
                    .album_repository
                    .create(Album::new(artist.id, title))
                    .await
                    .unwrap();
            }
            let query = ListAlbumsQuery {
                limit: 2,
                offset: 0,
            };
            let result = list_albums(State(state), Query(query)).await.unwrap();
            assert_eq!(result.total, 3);
            assert_eq!(result.items.len(), 2);
        }
    }
}
