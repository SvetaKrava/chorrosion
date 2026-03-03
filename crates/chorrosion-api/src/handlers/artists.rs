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
    State(_state): State<AppState>,
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

    // TODO: Use repository to fetch from database
    let artists: Vec<Artist> = vec![];
    let total = artists.len() as i64;

    let page = apply_list_query(artists, &normalized)
        .into_iter()
        .map(ArtistResponse::from)
        .collect();

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
    InvalidLimit,
    InvalidOffset,
    InvalidStatus,
    InvalidSortBy,
    InvalidSortOrder,
}

impl std::fmt::Display for ListArtistsQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLimit => write!(f, "limit must be between 1 and 500"),
            Self::InvalidOffset => write!(f, "offset must be greater than or equal to 0"),
            Self::InvalidStatus => write!(f, "status must be one of: continuing, ended"),
            Self::InvalidSortBy => write!(f, "sort_by must be one of: name, status, monitored"),
            Self::InvalidSortOrder => write!(f, "sort_order must be one of: asc, desc"),
        }
    }
}

fn normalize_list_query(query: &ListArtistsQuery) -> Result<NormalizedListQuery, ListArtistsQueryError> {
    if !(1..=500).contains(&query.limit) {
        return Err(ListArtistsQueryError::InvalidLimit);
    }

    if query.offset < 0 {
        return Err(ListArtistsQueryError::InvalidOffset);
    }

    let status = match query.status.as_deref() {
        None => None,
        Some(value) if value.eq_ignore_ascii_case("continuing") => Some(ArtistStatus::Continuing),
        Some(value) if value.eq_ignore_ascii_case("ended") => Some(ArtistStatus::Ended),
        Some(_) => return Err(ListArtistsQueryError::InvalidStatus),
    };

    let sort_by = match query.sort_by.as_deref() {
        None => ArtistSortField::Name,
        Some(value) if value.eq_ignore_ascii_case("name") => ArtistSortField::Name,
        Some(value) if value.eq_ignore_ascii_case("status") => ArtistSortField::Status,
        Some(value) if value.eq_ignore_ascii_case("monitored") => ArtistSortField::Monitored,
        Some(_) => return Err(ListArtistsQueryError::InvalidSortBy),
    };

    let sort_order = match query.sort_order.as_deref() {
        None => SortOrder::Asc,
        Some(value) if value.eq_ignore_ascii_case("asc") => SortOrder::Asc,
        Some(value) if value.eq_ignore_ascii_case("desc") => SortOrder::Desc,
        Some(_) => return Err(ListArtistsQueryError::InvalidSortOrder),
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

fn apply_list_query(mut artists: Vec<Artist>, query: &NormalizedListQuery) -> Vec<Artist> {
    if let Some(monitored) = query.monitored {
        artists.retain(|artist| artist.monitored == monitored);
    }

    if let Some(status) = query.status {
        artists.retain(|artist| artist.status == status);
    }

    artists.sort_by(|left, right| {
        let ordering = match query.sort_by {
            ArtistSortField::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            ArtistSortField::Status => left.status.to_string().cmp(&right.status.to_string()),
            ArtistSortField::Monitored => left.monitored.cmp(&right.monitored),
        };

        match query.sort_order {
            SortOrder::Asc => ordering,
            SortOrder::Desc => ordering.reverse(),
        }
    });

    let start = query.offset as usize;
    let end = start.saturating_add(query.limit as usize);

    if start >= artists.len() {
        return vec![];
    }

    artists
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
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
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "fetching artist");

    // TODO: Use repository to fetch from database
    // For now, return 404
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: format!("Artist {} not found", id),
        }),
    )
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
    State(_state): State<AppState>,
    Json(request): Json<CreateArtistRequest>,
) -> impl IntoResponse {
    debug!(target: "api", ?request, "creating artist");

    // TODO: Use repository to insert into database
    let mut artist = Artist::new(request.name);
    artist.foreign_artist_id = request.foreign_artist_id;
    artist.monitored = request.monitored.unwrap_or(true);
    artist.path = request.path;

    if let Some(status_str) = request.status {
        artist.status = match status_str.as_str() {
            "ended" => ArtistStatus::Ended,
            _ => ArtistStatus::Continuing,
        };
    }

    (StatusCode::CREATED, Json(ArtistResponse::from(artist)))
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
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateArtistRequest>,
) -> impl IntoResponse {
    debug!(target: "api", %id, ?request, "updating artist");

    // TODO: Use repository to fetch and update in database
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: format!("Artist {} not found", id),
        }),
    )
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
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "deleting artist");

    // TODO: Use repository to delete from database
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: format!("Artist {} not found", id),
        }),
    )
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
        assert!(matches!(result, Err(ListArtistsQueryError::InvalidLimit)));
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

        let filtered = apply_list_query(artists, &query);

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

        let paged = apply_list_query(artists, &query);

        assert_eq!(paged.len(), 2);
        assert_eq!(paged[0].name, "Charlie");
        assert_eq!(paged[1].name, "Bravo");
    }
}
