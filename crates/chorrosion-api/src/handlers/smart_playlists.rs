// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chorrosion_application::{AppState, SmartPlaylist, SmartPlaylistCriteria};
use chorrosion_domain::Validate;
use chrono::{Datelike, Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};
use utoipa::{IntoParams, ToSchema};

use crate::handlers::albums::AlbumResponse;

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListSmartPlaylistsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SmartPlaylistCriteriaRequest {
    pub recently_added_days: Option<i64>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    #[serde(default = "default_monitored_only")]
    pub monitored_only: bool,
}

fn default_monitored_only() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SmartPlaylistResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub criteria: SmartPlaylistCriteriaRequest,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateSmartPlaylistRequest {
    pub name: String,
    pub description: Option<String>,
    pub criteria: SmartPlaylistCriteriaRequest,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateSmartPlaylistRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub criteria: Option<SmartPlaylistCriteriaRequest>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListSmartPlaylistsResponse {
    pub items: Vec<SmartPlaylistResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SmartPlaylistItemsResponse {
    pub playlist: SmartPlaylistResponse,
    pub items: Vec<AlbumResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(as = SmartPlaylistErrorResponse)]
pub struct ErrorResponse {
    pub error: String,
}

impl From<SmartPlaylistCriteriaRequest> for SmartPlaylistCriteria {
    fn from(value: SmartPlaylistCriteriaRequest) -> Self {
        SmartPlaylistCriteria {
            recently_added_days: value.recently_added_days,
            genre: value.genre,
            year: value.year,
            monitored_only: value.monitored_only,
        }
    }
}

impl From<SmartPlaylistCriteria> for SmartPlaylistCriteriaRequest {
    fn from(value: SmartPlaylistCriteria) -> Self {
        Self {
            recently_added_days: value.recently_added_days,
            genre: value.genre,
            year: value.year,
            monitored_only: value.monitored_only,
        }
    }
}

impl From<SmartPlaylist> for SmartPlaylistResponse {
    fn from(value: SmartPlaylist) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            description: value.description,
            criteria: SmartPlaylistCriteriaRequest::from(value.criteria),
            created_at: value.created_at.to_rfc3339(),
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

fn error_response(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn album_matches_criteria(
    album: &chorrosion_domain::Album,
    criteria: &SmartPlaylistCriteria,
    now: chrono::DateTime<Utc>,
) -> bool {
    if criteria.monitored_only && !album.monitored {
        return false;
    }

    if let Some(days) = criteria.recently_added_days {
        let threshold = now - Duration::days(days);
        if album.created_at < threshold {
            return false;
        }
    }

    if let Some(expected_year) = criteria.year {
        let Some(release_date) = album.release_date else {
            return false;
        };
        if release_date.year() != expected_year {
            return false;
        }
    }

    if let Some(expected_genre) = &criteria.genre {
        let Some(album_genres) = &album.genre_tags else {
            return false;
        };
        let expected = expected_genre.trim();
        if expected.is_empty() {
            return false;
        }
        let genre_match = album_genres
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .any(|value| value.eq_ignore_ascii_case(expected));
        if !genre_match {
            return false;
        }
    }

    true
}

#[utoipa::path(
    post,
    path = "/api/v1/smart-playlists",
    request_body = CreateSmartPlaylistRequest,
    responses(
        (status = 201, description = "Smart playlist created", body = SmartPlaylistResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 409, description = "Playlist name already exists", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "smart_playlists"
)]
pub async fn create_smart_playlist(
    State(state): State<AppState>,
    Json(payload): Json<CreateSmartPlaylistRequest>,
) -> Result<(StatusCode, Json<SmartPlaylistResponse>), (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", name = %payload.name, "creating smart playlist");

    let playlist = SmartPlaylist::new(payload.name, payload.description, payload.criteria.into());

    if let Err(validation_errors) = playlist.validate() {
        let message = validation_errors
            .iter()
            .map(|item| format!("{} {}", item.field, item.message))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(error_response(StatusCode::BAD_REQUEST, message));
    }

    match state.smart_playlist_repository.create(playlist).await {
        Ok(created) => Ok((
            StatusCode::CREATED,
            Json(SmartPlaylistResponse::from(created)),
        )),
        Err(err) => {
            error!(target: "api", error = %err, "failed to create smart playlist");
            if err.to_string().contains("UNIQUE") {
                Err(error_response(
                    StatusCode::CONFLICT,
                    "smart playlist with this name already exists",
                ))
            } else {
                Err(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to create smart playlist",
                ))
            }
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/smart-playlists",
    params(ListSmartPlaylistsQuery),
    responses(
        (status = 200, description = "List smart playlists", body = ListSmartPlaylistsResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "smart_playlists"
)]
pub async fn list_smart_playlists(
    State(state): State<AppState>,
    Query(query): Query<ListSmartPlaylistsQuery>,
) -> Result<Json<ListSmartPlaylistsResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !(1..=500).contains(&query.limit) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "limit must be between 1 and 500",
        ));
    }
    if query.offset < 0 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "offset must be greater than or equal to 0",
        ));
    }

    let total = state
        .smart_playlist_repository
        .count()
        .await
        .map_err(|err| {
            error!(target: "api", error = %err, "failed to count smart playlists");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list smart playlists",
            )
        })?;

    let items = state
        .smart_playlist_repository
        .list(query.limit, query.offset)
        .await
        .map_err(|err| {
            error!(target: "api", error = %err, "failed to list smart playlists");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list smart playlists",
            )
        })?
        .into_iter()
        .map(SmartPlaylistResponse::from)
        .collect();

    Ok(Json(ListSmartPlaylistsResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/smart-playlists/{playlist_id}",
    params(
        ("playlist_id" = String, Path, description = "Smart playlist ID")
    ),
    responses(
        (status = 200, description = "Smart playlist", body = SmartPlaylistResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "smart_playlists"
)]
pub async fn get_smart_playlist(
    State(state): State<AppState>,
    Path(playlist_id): Path<String>,
) -> Result<Json<SmartPlaylistResponse>, (StatusCode, Json<ErrorResponse>)> {
    match state
        .smart_playlist_repository
        .get_by_id(&playlist_id)
        .await
    {
        Ok(Some(playlist)) => Ok(Json(SmartPlaylistResponse::from(playlist))),
        Ok(None) => Err(error_response(
            StatusCode::NOT_FOUND,
            "smart playlist not found",
        )),
        Err(err) => {
            error!(target: "api", error = %err, "failed to fetch smart playlist");
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to fetch smart playlist",
            ))
        }
    }
}

#[utoipa::path(
    patch,
    path = "/api/v1/smart-playlists/{playlist_id}",
    request_body = UpdateSmartPlaylistRequest,
    params(
        ("playlist_id" = String, Path, description = "Smart playlist ID")
    ),
    responses(
        (status = 200, description = "Smart playlist updated", body = SmartPlaylistResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 409, description = "Playlist name already exists", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "smart_playlists"
)]
pub async fn update_smart_playlist(
    State(state): State<AppState>,
    Path(playlist_id): Path<String>,
    Json(payload): Json<UpdateSmartPlaylistRequest>,
) -> Result<Json<SmartPlaylistResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut playlist = state
        .smart_playlist_repository
        .get_by_id(&playlist_id)
        .await
        .map_err(|err| {
            error!(target: "api", error = %err, "failed to fetch smart playlist for update");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to update smart playlist",
            )
        })?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "smart playlist not found"))?;

    if let Some(name) = payload.name {
        playlist.name = name;
    }
    if payload.description.is_some() {
        playlist.description = payload.description;
    }
    if let Some(criteria) = payload.criteria {
        playlist.criteria = criteria.into();
    }
    playlist.updated_at = Utc::now();

    if let Err(validation_errors) = playlist.validate() {
        let message = validation_errors
            .iter()
            .map(|item| format!("{} {}", item.field, item.message))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(error_response(StatusCode::BAD_REQUEST, message));
    }

    match state.smart_playlist_repository.update(playlist).await {
        Ok(updated) => Ok(Json(SmartPlaylistResponse::from(updated))),
        Err(err) => {
            error!(target: "api", error = %err, "failed to update smart playlist");
            if err.to_string().contains("UNIQUE") {
                Err(error_response(
                    StatusCode::CONFLICT,
                    "smart playlist with this name already exists",
                ))
            } else {
                Err(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to update smart playlist",
                ))
            }
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/smart-playlists/{playlist_id}",
    params(
        ("playlist_id" = String, Path, description = "Smart playlist ID")
    ),
    responses(
        (status = 204, description = "Smart playlist deleted"),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "smart_playlists"
)]
pub async fn delete_smart_playlist(
    State(state): State<AppState>,
    Path(playlist_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.smart_playlist_repository.delete(&playlist_id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(err) => {
            error!(target: "api", error = %err, "failed to delete smart playlist");
            if err.to_string().contains("not found") {
                Err(error_response(
                    StatusCode::NOT_FOUND,
                    "smart playlist not found",
                ))
            } else {
                Err(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to delete smart playlist",
                ))
            }
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/smart-playlists/{playlist_id}/items",
    params(
        ("playlist_id" = String, Path, description = "Smart playlist ID"),
        ListSmartPlaylistsQuery
    ),
    responses(
        (status = 200, description = "Evaluated smart playlist items", body = SmartPlaylistItemsResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "smart_playlists"
)]
pub async fn get_smart_playlist_items(
    State(state): State<AppState>,
    Path(playlist_id): Path<String>,
    Query(query): Query<ListSmartPlaylistsQuery>,
) -> Result<Json<SmartPlaylistItemsResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !(1..=500).contains(&query.limit) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "limit must be between 1 and 500",
        ));
    }
    if query.offset < 0 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "offset must be greater than or equal to 0",
        ));
    }

    let playlist = state
        .smart_playlist_repository
        .get_by_id(&playlist_id)
        .await
        .map_err(|err| {
            error!(target: "api", error = %err, "failed to fetch smart playlist");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to evaluate smart playlist",
            )
        })?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "smart playlist not found"))?;

    let now = Utc::now();
    let mut total = 0_i64;
    let mut repository_offset = 0_i64;
    let mut items = Vec::new();
    const ALBUM_EVALUATION_PAGE_SIZE: i64 = 500;

    loop {
        let albums = state
            .album_repository
            .list(ALBUM_EVALUATION_PAGE_SIZE, repository_offset)
            .await
            .map_err(|err| {
                error!(target: "api", error = %err, "failed to list albums for smart playlist evaluation");
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to evaluate smart playlist",
                )
            })?;

        let fetched_count = albums.len() as i64;

        for album in albums {
            if album_matches_criteria(&album, &playlist.criteria, now) {
                if total >= query.offset && (items.len() as i64) < query.limit {
                    items.push(AlbumResponse::from(album));
                }
                total += 1;
            }
        }

        if fetched_count < ALBUM_EVALUATION_PAGE_SIZE {
            break;
        }

        repository_offset += ALBUM_EVALUATION_PAGE_SIZE;
    }

    Ok(Json(SmartPlaylistItemsResponse {
        playlist: SmartPlaylistResponse::from(playlist),
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, Query, State};
    use chorrosion_config::AppConfig;
    use chorrosion_domain::{Album, Artist};
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteSmartPlaylistRepository, SqliteTagRepository,
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
            Arc::new(SqliteSmartPlaylistRepository::new(pool.clone())),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        )
    }

    #[tokio::test]
    async fn create_and_list_smart_playlists_round_trip() {
        let state = make_test_state().await;

        let created = create_smart_playlist(
            State(state.clone()),
            Json(CreateSmartPlaylistRequest {
                name: "Recent Metal".to_string(),
                description: Some("All recently added metal releases".to_string()),
                criteria: SmartPlaylistCriteriaRequest {
                    recently_added_days: Some(30),
                    genre: Some("metal".to_string()),
                    year: None,
                    monitored_only: true,
                },
            }),
        )
        .await
        .expect("create smart playlist should succeed");

        assert_eq!(created.0, StatusCode::CREATED);
        assert_eq!(created.1.name, "Recent Metal");

        let listed = list_smart_playlists(
            State(state),
            Query(ListSmartPlaylistsQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("list smart playlists should succeed");

        assert_eq!(listed.total, 1);
        assert_eq!(listed.items[0].name, "Recent Metal");
    }

    #[tokio::test]
    async fn get_smart_playlist_items_filters_by_genre_and_year() {
        let state = make_test_state().await;

        let artist = Artist::new("Test Artist");
        let artist_id = artist.id;
        state
            .artist_repository
            .create(artist)
            .await
            .expect("create artist");

        let mut matching_album = Album::new(artist_id, "Matching Album");
        matching_album.genre_tags = Some("metal,rock".to_string());
        matching_album.release_date = chrono::NaiveDate::from_ymd_opt(2024, 6, 1);
        state
            .album_repository
            .create(matching_album)
            .await
            .expect("create matching album");

        let mut non_matching_album = Album::new(artist_id, "Non Matching Album");
        non_matching_album.genre_tags = Some("jazz".to_string());
        non_matching_album.release_date = chrono::NaiveDate::from_ymd_opt(2023, 1, 1);
        state
            .album_repository
            .create(non_matching_album)
            .await
            .expect("create non matching album");

        let (status, created) = create_smart_playlist(
            State(state.clone()),
            Json(CreateSmartPlaylistRequest {
                name: "Metal 2024".to_string(),
                description: None,
                criteria: SmartPlaylistCriteriaRequest {
                    recently_added_days: None,
                    genre: Some("metal".to_string()),
                    year: Some(2024),
                    monitored_only: true,
                },
            }),
        )
        .await
        .expect("create smart playlist should succeed");
        assert_eq!(status, StatusCode::CREATED);

        let items = get_smart_playlist_items(
            State(state),
            Path(created.id.clone()),
            Query(ListSmartPlaylistsQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("evaluating smart playlist should succeed");

        assert_eq!(items.total, 1);
        assert_eq!(items.items[0].title, "Matching Album");
    }
}
