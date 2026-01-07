use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use lidarr_application::AppState;
use lidarr_domain::{Artist, ArtistStatus};
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
        (status = 200, description = "List of artists", body = Vec<ArtistResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "artists"
)]
pub async fn list_artists(
    State(_state): State<AppState>,
    Query(query): Query<ListArtistsQuery>,
) -> impl IntoResponse {
    debug!(target: "api", ?query, "listing artists");
    
    // TODO: Use repository to fetch from database
    let artists: Vec<ArtistResponse> = vec![];
    
    Json(artists)
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
