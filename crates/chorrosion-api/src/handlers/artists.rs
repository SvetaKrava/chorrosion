// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::{Artist, ArtistStatus};
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::{IntoParams, ToSchema};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListArtistsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    pub monitored: Option<bool>,
    pub status: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ArtistResponse {
    pub id: String,
    pub name: String,
    pub foreign_artist_id: Option<String>,
    pub status: String,
    pub monitored: bool,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListArtistsResponse {
    pub items: Vec<ArtistResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ArtistStatisticsResponse {
    pub artist_id: String,
    pub total_albums: i64,
    pub monitored_albums: i64,
    pub total_tracks: i64,
    pub monitored_tracks: i64,
    pub tracks_with_files: i64,
    pub tracks_without_files: i64,
}

impl From<Artist> for ArtistResponse {
    fn from(artist: Artist) -> Self {
        Self {
            id: artist.id.to_string(),
            name: artist.name,
            foreign_artist_id: artist.foreign_artist_id,
            status: artist.status.to_string(),
            monitored: artist.monitored,
            path: artist.path,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateArtistRequest {
    pub name: String,
    pub foreign_artist_id: Option<String>,
    pub status: Option<String>,
    pub monitored: Option<bool>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateArtistRequest {
    pub name: Option<String>,
    pub foreign_artist_id: Option<String>,
    pub status: Option<String>,
    pub monitored: Option<bool>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

// ============================================================================
// Helpers
// ============================================================================

fn parse_artist_status(
    status_str: &str,
) -> Result<ArtistStatus, (StatusCode, Json<ErrorResponse>)> {
    match status_str.to_ascii_lowercase().as_str() {
        "continuing" => Ok(ArtistStatus::Continuing),
        "ended" => Ok(ArtistStatus::Ended),
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("invalid status value: {status_str}"),
            }),
        )),
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// List all artists with optional filtering
#[utoipa::path(
    get,
    path = "/api/v1/artists",
    params(ListArtistsQuery),
    responses(
        (status = 200, description = "List of artists", body = ListArtistsResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "artists"
)]
pub async fn list_artists(
    State(state): State<AppState>,
    Query(query): Query<ListArtistsQuery>,
) -> Result<Json<ListArtistsResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", ?query, "listing artists");

    let normalized = normalize_list_query(&query).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )
    })?;

    let artists = state
        .artist_repository
        .list(5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list artists: {error}"),
                }),
            )
        })?;

    let (page, total) = apply_list_query(artists, &normalized);

    let page = page.into_iter().map(ArtistResponse::from).collect();

    Ok(Json(ListArtistsResponse {
        items: page,
        total,
        limit: normalized.limit,
        offset: normalized.offset,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArtistSortField {
    Name,
    Status,
    Monitored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NormalizedListQuery {
    limit: i64,
    offset: i64,
    monitored: Option<bool>,
    status: Option<ArtistStatus>,
    sort_by: ArtistSortField,
    sort_order: SortOrder,
}

#[derive(Debug)]
enum ListArtistsQueryError {
    Limit,
    Offset,
    Status,
    SortBy,
    SortOrder,
}

impl std::fmt::Display for ListArtistsQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit => write!(f, "limit must be between 1 and 500"),
            Self::Offset => write!(f, "offset must be greater than or equal to 0"),
            Self::Status => write!(f, "status must be one of: continuing, ended"),
            Self::SortBy => write!(f, "sort_by must be one of: name, status, monitored"),
            Self::SortOrder => write!(f, "sort_order must be one of: asc, desc"),
        }
    }
}

fn normalize_list_query(
    query: &ListArtistsQuery,
) -> Result<NormalizedListQuery, ListArtistsQueryError> {
    if !(1..=500).contains(&query.limit) {
        return Err(ListArtistsQueryError::Limit);
    }

    if query.offset < 0 {
        return Err(ListArtistsQueryError::Offset);
    }

    let status = match query.status.as_deref() {
        None => None,
        Some(value) if value.eq_ignore_ascii_case("continuing") => Some(ArtistStatus::Continuing),
        Some(value) if value.eq_ignore_ascii_case("ended") => Some(ArtistStatus::Ended),
        Some(_) => return Err(ListArtistsQueryError::Status),
    };

    let sort_by = match query.sort_by.as_deref() {
        None => ArtistSortField::Name,
        Some(value) if value.eq_ignore_ascii_case("name") => ArtistSortField::Name,
        Some(value) if value.eq_ignore_ascii_case("status") => ArtistSortField::Status,
        Some(value) if value.eq_ignore_ascii_case("monitored") => ArtistSortField::Monitored,
        Some(_) => return Err(ListArtistsQueryError::SortBy),
    };

    let sort_order = match query.sort_order.as_deref() {
        None => SortOrder::Asc,
        Some(value) if value.eq_ignore_ascii_case("asc") => SortOrder::Asc,
        Some(value) if value.eq_ignore_ascii_case("desc") => SortOrder::Desc,
        Some(_) => return Err(ListArtistsQueryError::SortOrder),
    };

    Ok(NormalizedListQuery {
        limit: query.limit,
        offset: query.offset,
        monitored: query.monitored,
        status,
        sort_by,
        sort_order,
    })
}

fn apply_list_query(mut artists: Vec<Artist>, query: &NormalizedListQuery) -> (Vec<Artist>, i64) {
    if let Some(monitored) = query.monitored {
        artists.retain(|artist| artist.monitored == monitored);
    }

    if let Some(status) = query.status {
        artists.retain(|artist| artist.status == status);
    }

    let total = artists.len() as i64;

    let start = match usize::try_from(query.offset) {
        Ok(v) => v,
        Err(_) => return (vec![], total),
    };
    let limit = match usize::try_from(query.limit) {
        Ok(v) => v,
        Err(_) => return (vec![], total),
    };

    if start >= artists.len() {
        return (vec![], total);
    }

    match query.sort_by {
        ArtistSortField::Name => {
            artists.sort_by_cached_key(|a| (a.name.to_lowercase(), a.id.0));
        }
        ArtistSortField::Status => {
            artists.sort_by_cached_key(|a| {
                let rank: u8 = match a.status {
                    ArtistStatus::Continuing => 0,
                    ArtistStatus::Ended => 1,
                };
                (rank, a.id.0)
            });
        }
        ArtistSortField::Monitored => {
            artists.sort_by_cached_key(|a| (a.monitored, a.id.0));
        }
    }

    if query.sort_order == SortOrder::Desc {
        artists.reverse();
    }

    let end = start.saturating_add(limit);
    let page = artists
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();

    (page, total)
}

/// Get a single artist by ID
#[utoipa::path(
    get,
    path = "/api/v1/artists/{id}",
    params(
        ("id" = String, Path, description = "Artist ID")
    ),
    responses(
        (status = 200, description = "Artist found", body = ArtistResponse),
        (status = 404, description = "Artist not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "artists"
)]
pub async fn get_artist(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "fetching artist");

    match state.artist_repository.get_by_id(&id).await {
        Ok(Some(artist)) => (StatusCode::OK, Json(ArtistResponse::from(artist))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Artist {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch artist: {error}"),
            }),
        )
            .into_response(),
    }
}

/// Get aggregate statistics for a single artist.
#[utoipa::path(
    get,
    path = "/api/v1/artists/{id}/statistics",
    params(
        ("id" = String, Path, description = "Artist ID")
    ),
    responses(
        (status = 200, description = "Artist statistics", body = ArtistStatisticsResponse),
        (status = 404, description = "Artist not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "artists"
)]
pub async fn get_artist_statistics(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "fetching artist statistics");

    let artist = match state.artist_repository.get_by_id(&id).await {
        Ok(Some(artist)) => artist,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Artist {} not found", id),
                }),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch artist: {error}"),
                }),
            )
                .into_response();
        }
    };

    const PAGE_SIZE: i64 = 5000;

    let mut total_albums: i64 = 0;
    let mut monitored_albums: i64 = 0;
    let mut album_offset: i64 = 0;

    loop {
        let page = match state
            .album_repository
            .get_by_artist(artist.id, PAGE_SIZE, album_offset)
            .await
        {
            Ok(page) => page,
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to fetch albums for artist: {error}"),
                    }),
                )
                    .into_response();
            }
        };

        if page.is_empty() {
            break;
        }

        total_albums += page.len() as i64;
        monitored_albums += page.iter().filter(|album| album.monitored).count() as i64;

        if page.len() < PAGE_SIZE as usize {
            break;
        }

        album_offset += PAGE_SIZE;
    }

    let mut total_tracks: i64 = 0;
    let mut monitored_tracks: i64 = 0;
    let mut tracks_with_files: i64 = 0;
    let mut track_offset: i64 = 0;

    loop {
        let page = match state
            .track_repository
            .get_by_artist(artist.id, PAGE_SIZE, track_offset)
            .await
        {
            Ok(page) => page,
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to fetch tracks for artist: {error}"),
                    }),
                )
                    .into_response();
            }
        };

        if page.is_empty() {
            break;
        }

        for track in &page {
            total_tracks += 1;
            if track.monitored {
                monitored_tracks += 1;
            }
            if track.has_file {
                tracks_with_files += 1;
            }
        }

        if page.len() < PAGE_SIZE as usize {
            break;
        }

        track_offset += PAGE_SIZE;
    }

    let tracks_without_files = total_tracks - tracks_with_files;

    (
        StatusCode::OK,
        Json(ArtistStatisticsResponse {
            artist_id: artist.id.to_string(),
            total_albums,
            monitored_albums,
            total_tracks,
            monitored_tracks,
            tracks_with_files,
            tracks_without_files,
        }),
    )
        .into_response()
}

/// Create a new artist
#[utoipa::path(
    post,
    path = "/api/v1/artists",
    request_body = CreateArtistRequest,
    responses(
        (status = 201, description = "Artist created", body = ArtistResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "artists"
)]
pub async fn create_artist(
    State(state): State<AppState>,
    Json(request): Json<CreateArtistRequest>,
) -> impl IntoResponse {
    debug!(target: "api", ?request, "creating artist");

    let mut artist = Artist::new(request.name);
    artist.foreign_artist_id = request.foreign_artist_id;
    artist.monitored = request.monitored.unwrap_or(true);
    artist.path = request.path;

    if let Some(status_str) = request.status {
        match parse_artist_status(&status_str) {
            Ok(status) => artist.status = status,
            Err(err_response) => return err_response.into_response(),
        }
    }

    match state.artist_repository.create(artist).await {
        Ok(created) => (StatusCode::CREATED, Json(ArtistResponse::from(created))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to create artist: {error}"),
            }),
        )
            .into_response(),
    }
}

/// Update an existing artist
#[utoipa::path(
    put,
    path = "/api/v1/artists/{id}",
    params(
        ("id" = String, Path, description = "Artist ID")
    ),
    request_body = UpdateArtistRequest,
    responses(
        (status = 200, description = "Artist updated", body = ArtistResponse),
        (status = 404, description = "Artist not found", body = ErrorResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "artists"
)]
pub async fn update_artist(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateArtistRequest>,
) -> impl IntoResponse {
    debug!(target: "api", %id, ?request, "updating artist");

    let mut artist = match state.artist_repository.get_by_id(&id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Artist {} not found", id),
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

    if let Some(name) = request.name {
        artist.name = name;
    }
    if let Some(foreign_id) = request.foreign_artist_id {
        artist.foreign_artist_id = Some(foreign_id);
    }
    if let Some(status_str) = request.status {
        match parse_artist_status(&status_str) {
            Ok(status) => artist.status = status,
            Err(err_response) => return err_response.into_response(),
        }
    }
    if let Some(monitored) = request.monitored {
        artist.monitored = monitored;
    }
    if let Some(path) = request.path {
        artist.path = Some(path);
    }

    match state.artist_repository.update(artist).await {
        Ok(updated) => (StatusCode::OK, Json(ArtistResponse::from(updated))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to update artist: {error}"),
            }),
        )
            .into_response(),
    }
}

/// Delete an artist
#[utoipa::path(
    delete,
    path = "/api/v1/artists/{id}",
    params(
        ("id" = String, Path, description = "Artist ID")
    ),
    responses(
        (status = 204, description = "Artist deleted"),
        (status = 404, description = "Artist not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "artists"
)]
pub async fn delete_artist(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "deleting artist");

    match state.artist_repository.get_by_id(&id).await {
        Ok(Some(_)) => {
            match state.artist_repository.delete(&id).await {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(delete_error) => {
                    // Check if the artist was concurrently deleted before we could.
                    match state.artist_repository.get_by_id(&id).await {
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            Json(ErrorResponse {
                                error: format!("Artist {} not found", id),
                            }),
                        )
                            .into_response(),
                        Ok(Some(_)) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: format!("failed to delete artist: {delete_error}"),
                            }),
                        )
                            .into_response(),
                        Err(_) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: format!("failed to delete artist: {delete_error}"),
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
                error: format!("Artist {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch artist before delete: {error}"),
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artist(name: &str, status: ArtistStatus, monitored: bool) -> Artist {
        let mut artist = Artist::new(name);
        artist.status = status;
        artist.monitored = monitored;
        artist
    }

    #[test]
    fn normalize_query_rejects_invalid_limit() {
        let query = ListArtistsQuery {
            limit: 0,
            offset: 0,
            monitored: None,
            status: None,
            sort_by: None,
            sort_order: None,
        };

        let result = normalize_list_query(&query);
        assert!(matches!(result, Err(ListArtistsQueryError::Limit)));
    }

    #[test]
    fn normalize_query_rejects_negative_offset() {
        let query = ListArtistsQuery {
            limit: 10,
            offset: -1,
            monitored: None,
            status: None,
            sort_by: None,
            sort_order: None,
        };

        let result = normalize_list_query(&query);
        assert!(matches!(result, Err(ListArtistsQueryError::Offset)));
    }

    #[test]
    fn normalize_query_rejects_invalid_status() {
        let query = ListArtistsQuery {
            limit: 10,
            offset: 0,
            monitored: None,
            status: Some("unknown".to_string()),
            sort_by: None,
            sort_order: None,
        };

        let result = normalize_list_query(&query);
        assert!(matches!(result, Err(ListArtistsQueryError::Status)));
    }

    #[test]
    fn normalize_query_rejects_invalid_sort_by() {
        let query = ListArtistsQuery {
            limit: 10,
            offset: 0,
            monitored: None,
            status: None,
            sort_by: Some("invalid_field".to_string()),
            sort_order: None,
        };

        let result = normalize_list_query(&query);
        assert!(matches!(result, Err(ListArtistsQueryError::SortBy)));
    }

    #[test]
    fn normalize_query_rejects_invalid_sort_order() {
        let query = ListArtistsQuery {
            limit: 10,
            offset: 0,
            monitored: None,
            status: None,
            sort_by: None,
            sort_order: Some("random".to_string()),
        };

        let result = normalize_list_query(&query);
        assert!(matches!(result, Err(ListArtistsQueryError::SortOrder)));
    }

    #[test]
    fn apply_query_filters_status_and_monitored() {
        let artists = vec![
            artist("A", ArtistStatus::Continuing, true),
            artist("B", ArtistStatus::Ended, true),
            artist("C", ArtistStatus::Continuing, false),
        ];

        let query = NormalizedListQuery {
            limit: 50,
            offset: 0,
            monitored: Some(true),
            status: Some(ArtistStatus::Continuing),
            sort_by: ArtistSortField::Name,
            sort_order: SortOrder::Asc,
        };

        let (filtered, total) = apply_list_query(artists, &query);

        assert_eq!(total, 1);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "A");
    }

    #[test]
    fn apply_query_sorts_desc_by_name_and_paginates() {
        let artists = vec![
            artist("Alpha", ArtistStatus::Continuing, true),
            artist("Charlie", ArtistStatus::Continuing, true),
            artist("Bravo", ArtistStatus::Continuing, true),
        ];

        let query = NormalizedListQuery {
            limit: 2,
            offset: 0,
            monitored: None,
            status: None,
            sort_by: ArtistSortField::Name,
            sort_order: SortOrder::Desc,
        };

        let (paged, total) = apply_list_query(artists, &query);

        assert_eq!(total, 3);
        assert_eq!(paged.len(), 2);
        assert_eq!(paged[0].name, "Charlie");
        assert_eq!(paged[1].name, "Bravo");
    }

    #[test]
    fn apply_query_sort_is_deterministic_when_primary_keys_equal() {
        use chorrosion_domain::ArtistId;
        use uuid::Uuid;

        let mut a1 = artist("same", ArtistStatus::Continuing, true);
        a1.id = ArtistId::from_uuid(Uuid::from_u128(1));

        let mut a2 = artist("same", ArtistStatus::Continuing, true);
        a2.id = ArtistId::from_uuid(Uuid::from_u128(2));

        let query = NormalizedListQuery {
            limit: 50,
            offset: 0,
            monitored: None,
            status: None,
            sort_by: ArtistSortField::Name,
            sort_order: SortOrder::Asc,
        };

        // Run twice with inputs in different order to verify stable tie-breaking.
        let (ordered1, _) = apply_list_query(vec![a2.clone(), a1.clone()], &query);
        let (ordered2, _) = apply_list_query(vec![a1.clone(), a2.clone()], &query);

        // UUID 1 < UUID 2, so a1 must always come first in ascending order.
        assert_eq!(ordered1[0].id, a1.id);
        assert_eq!(ordered1[1].id, a2.id);
        assert_eq!(ordered2[0].id, a1.id);
        assert_eq!(ordered2[1].id, a2.id);
    }

    // ============================================================================
    // parse_artist_status tests
    // ============================================================================

    #[test]
    fn parse_status_accepts_continuing_case_insensitive() {
        assert!(matches!(
            parse_artist_status("continuing"),
            Ok(ArtistStatus::Continuing)
        ));
        assert!(matches!(
            parse_artist_status("Continuing"),
            Ok(ArtistStatus::Continuing)
        ));
        assert!(matches!(
            parse_artist_status("CONTINUING"),
            Ok(ArtistStatus::Continuing)
        ));
    }

    #[test]
    fn parse_status_accepts_ended_case_insensitive() {
        assert!(matches!(
            parse_artist_status("ended"),
            Ok(ArtistStatus::Ended)
        ));
        assert!(matches!(
            parse_artist_status("Ended"),
            Ok(ArtistStatus::Ended)
        ));
        assert!(matches!(
            parse_artist_status("ENDED"),
            Ok(ArtistStatus::Ended)
        ));
    }

    #[test]
    fn parse_status_rejects_unknown_value() {
        let result = parse_artist_status("unknown");
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn parse_status_rejects_empty_string() {
        let result = parse_artist_status("");
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // ============================================================================
    // Write handler tests (async, in-memory SQLite)
    // ============================================================================

    #[cfg(test)]
    mod write_handlers {
        use super::*;
        use axum::extract::{Path, State};
        use axum::response::IntoResponse;
        use chorrosion_config::AppConfig;
        use chorrosion_domain::{Album, Track};
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

        // --- create_artist ---

        #[tokio::test]
        async fn create_artist_returns_201_on_success() {
            let state = make_test_state().await;
            let request = CreateArtistRequest {
                name: "Test Artist".to_string(),
                foreign_artist_id: None,
                status: None,
                monitored: None,
                path: None,
            };
            let response = create_artist(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        #[tokio::test]
        async fn create_artist_rejects_invalid_status() {
            let state = make_test_state().await;
            let request = CreateArtistRequest {
                name: "Test Artist".to_string(),
                foreign_artist_id: None,
                status: Some("garbage".to_string()),
                monitored: None,
                path: None,
            };
            let response = create_artist(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn create_artist_accepts_uppercase_ended_status() {
            let state = make_test_state().await;
            let request = CreateArtistRequest {
                name: "Test Artist".to_string(),
                foreign_artist_id: None,
                status: Some("ENDED".to_string()),
                monitored: None,
                path: None,
            };
            let response = create_artist(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        // --- update_artist ---

        #[tokio::test]
        async fn update_artist_returns_200_on_success() {
            let state = make_test_state().await;
            // First create an artist.
            let created = state
                .artist_repository
                .create(Artist::new("Before"))
                .await
                .unwrap();
            let id = created.id.to_string();
            let request = UpdateArtistRequest {
                name: Some("After".to_string()),
                foreign_artist_id: None,
                status: None,
                monitored: None,
                path: None,
            };
            let response = update_artist(State(state), Path(id), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn update_artist_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let request = UpdateArtistRequest {
                name: Some("Name".to_string()),
                foreign_artist_id: None,
                status: None,
                monitored: None,
                path: None,
            };
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = update_artist(State(state), Path(unknown_id), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn update_artist_rejects_invalid_status() {
            let state = make_test_state().await;
            let created = state
                .artist_repository
                .create(Artist::new("Artist"))
                .await
                .unwrap();
            let id = created.id.to_string();
            let request = UpdateArtistRequest {
                name: None,
                foreign_artist_id: None,
                status: Some("bad_status".to_string()),
                monitored: None,
                path: None,
            };
            let response = update_artist(State(state), Path(id), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        // --- delete_artist ---

        #[tokio::test]
        async fn delete_artist_returns_204_on_success() {
            let state = make_test_state().await;
            let created = state
                .artist_repository
                .create(Artist::new("To Delete"))
                .await
                .unwrap();
            let id = created.id.to_string();
            let response = delete_artist(State(state), Path(id)).await.into_response();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }

        #[tokio::test]
        async fn delete_artist_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = delete_artist(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // --- get_artist_statistics ---

        #[tokio::test]
        async fn get_artist_statistics_returns_200_for_existing_artist() {
            let state = make_test_state().await;

            let artist = state
                .artist_repository
                .create(Artist::new("Stats Artist"))
                .await
                .unwrap();

            // 1 unmonitored album
            let mut album = Album::new(artist.id, "Stats Album");
            album.monitored = false;
            let album = state.album_repository.create(album).await.unwrap();

            // track_1: monitored=true (default), has_file=true
            let mut track_1 = Track::new(album.id, artist.id, "Song A");
            track_1.has_file = true;
            state.track_repository.create(track_1).await.unwrap();

            // track_2: monitored=false, has_file=false (default)
            let mut track_2 = Track::new(album.id, artist.id, "Song B");
            track_2.monitored = false;
            state.track_repository.create(track_2).await.unwrap();

            let response = get_artist_statistics(State(state), Path(artist.id.to_string()))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);

            let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let stats: ArtistStatisticsResponse = serde_json::from_slice(&body_bytes).unwrap();

            assert_eq!(stats.artist_id, artist.id.to_string());
            assert_eq!(stats.total_albums, 1);
            assert_eq!(stats.monitored_albums, 0); // album is unmonitored
            assert_eq!(stats.total_tracks, 2);
            assert_eq!(stats.monitored_tracks, 1); // only track_1 is monitored
            assert_eq!(stats.tracks_with_files, 1); // only track_1 has a file
            assert_eq!(stats.tracks_without_files, 1); // track_2 has no file
        }

        #[tokio::test]
        async fn get_artist_statistics_returns_404_for_unknown_artist() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();

            let response = get_artist_statistics(State(state), Path(unknown_id))
                .await
                .into_response();

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }
}
