// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::MetadataProfile;
use chorrosion_infrastructure::repositories::Repository;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListMetadataProfilesQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MetadataProfileResponse {
    pub id: String,
    pub name: String,
    pub primary_album_types: Vec<String>,
    pub secondary_album_types: Vec<String>,
    pub release_statuses: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListMetadataProfilesResponse {
    pub items: Vec<MetadataProfileResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl From<MetadataProfile> for MetadataProfileResponse {
    fn from(profile: MetadataProfile) -> Self {
        Self {
            id: profile.id.to_string(),
            name: profile.name,
            primary_album_types: profile.primary_album_types,
            secondary_album_types: profile.secondary_album_types,
            release_statuses: profile.release_statuses,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMetadataProfileRequest {
    pub name: String,
    pub primary_album_types: Option<Vec<String>>,
    pub secondary_album_types: Option<Vec<String>>,
    pub release_statuses: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMetadataProfileRequest {
    pub name: Option<String>,
    pub primary_album_types: Option<Vec<String>>,
    pub secondary_album_types: Option<Vec<String>>,
    pub release_statuses: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = MetadataProfileErrorResponse)]
pub struct ErrorResponse {
    pub error: String,
}

fn validate_name(name: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if name.trim().is_empty() {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "name cannot be empty".to_string(),
            }),
        ))
    } else {
        Ok(())
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/metadata-profiles",
    params(ListMetadataProfilesQuery),
    responses(
        (status = 200, description = "List metadata profiles", body = ListMetadataProfilesResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn list_metadata_profiles(
    State(state): State<AppState>,
    Query(query): Query<ListMetadataProfilesQuery>,
) -> Result<Json<ListMetadataProfilesResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", ?query, "listing metadata profiles");

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

    let all_profiles = state
        .metadata_profile_repository
        .list(5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list metadata profiles: {error}"),
                }),
            )
        })?;

    let total = all_profiles.len() as i64;
    let offset = match usize::try_from(query.offset) {
        Ok(offset) => offset,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "offset out of valid range".to_string(),
                }),
            ));
        }
    };
    let limit = match usize::try_from(query.limit) {
        Ok(limit) => limit,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "limit out of valid range".to_string(),
                }),
            ));
        }
    };

    let items = all_profiles
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(MetadataProfileResponse::from)
        .collect();

    Ok(Json(ListMetadataProfilesResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/metadata-profiles/{id}",
    params(("id" = String, Path, description = "Metadata profile ID")),
    responses(
        (status = 200, description = "Metadata profile found", body = MetadataProfileResponse),
        (status = 404, description = "Metadata profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn get_metadata_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "fetching metadata profile");

    match state
        .metadata_profile_repository
        .get_by_id(id.clone())
        .await
    {
        Ok(Some(profile)) => {
            (StatusCode::OK, Json(MetadataProfileResponse::from(profile))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Metadata profile {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch metadata profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/metadata-profiles",
    request_body = CreateMetadataProfileRequest,
    responses(
        (status = 201, description = "Metadata profile created", body = MetadataProfileResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn create_metadata_profile(
    State(state): State<AppState>,
    Json(request): Json<CreateMetadataProfileRequest>,
) -> impl IntoResponse {
    debug!(target: "api", ?request, "creating metadata profile");

    if let Err(err_response) = validate_name(&request.name) {
        return err_response.into_response();
    }

    let mut profile = MetadataProfile::new(request.name);
    if let Some(primary) = request.primary_album_types {
        profile.primary_album_types = primary;
    }
    if let Some(secondary) = request.secondary_album_types {
        profile.secondary_album_types = secondary;
    }
    if let Some(statuses) = request.release_statuses {
        profile.release_statuses = statuses;
    }

    match state.metadata_profile_repository.create(profile).await {
        Ok(created) => (
            StatusCode::CREATED,
            Json(MetadataProfileResponse::from(created)),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to create metadata profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/metadata-profiles/{id}",
    params(("id" = String, Path, description = "Metadata profile ID")),
    request_body = UpdateMetadataProfileRequest,
    responses(
        (status = 200, description = "Metadata profile updated", body = MetadataProfileResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Metadata profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn update_metadata_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateMetadataProfileRequest>,
) -> impl IntoResponse {
    debug!(target: "api", %id, ?request, "updating metadata profile");

    let mut profile = match state
        .metadata_profile_repository
        .get_by_id(id.clone())
        .await
    {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Metadata profile {} not found", id),
                }),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch metadata profile: {error}"),
                }),
            )
                .into_response();
        }
    };

    if let Some(name) = request.name {
        if let Err(err_response) = validate_name(&name) {
            return err_response.into_response();
        }
        profile.name = name;
    }
    if let Some(primary) = request.primary_album_types {
        profile.primary_album_types = primary;
    }
    if let Some(secondary) = request.secondary_album_types {
        profile.secondary_album_types = secondary;
    }
    if let Some(statuses) = request.release_statuses {
        profile.release_statuses = statuses;
    }
    profile.updated_at = Utc::now();

    match state.metadata_profile_repository.update(profile).await {
        Ok(updated) => {
            (StatusCode::OK, Json(MetadataProfileResponse::from(updated))).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to update metadata profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/settings/metadata-profiles/{id}",
    params(("id" = String, Path, description = "Metadata profile ID")),
    responses(
        (status = 204, description = "Metadata profile deleted"),
        (status = 404, description = "Metadata profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn delete_metadata_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "deleting metadata profile");

    match state
        .metadata_profile_repository
        .get_by_id(id.clone())
        .await
    {
        Ok(Some(_)) => {
            match state.metadata_profile_repository.delete(id.clone()).await {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(delete_error) => {
                    // Recheck existence to distinguish concurrent deletion (404)
                    // from a transient delete failure (500).
                    match state
                        .metadata_profile_repository
                        .get_by_id(id.clone())
                        .await
                    {
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            Json(ErrorResponse {
                                error: format!("Metadata profile {} not found", id),
                            }),
                        )
                            .into_response(),
                        Ok(Some(_)) | Err(_) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: format!("failed to delete metadata profile: {delete_error}"),
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
                error: format!("Metadata profile {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch metadata profile before delete: {error}"),
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod write_handlers {
        use super::*;
        use axum::extract::{Path, Query, State};
        use axum::response::IntoResponse;
        use chorrosion_config::AppConfig;
        use chorrosion_infrastructure::sqlite_adapters::{
            SqliteAlbumRepository, SqliteArtistRepository, SqliteMetadataProfileRepository,
            SqliteQualityProfileRepository, SqliteTrackRepository,
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
                Arc::new(SqliteMetadataProfileRepository::new(pool)),
            )
        }

        async fn create_test_profile(state: &AppState) -> chorrosion_domain::MetadataProfile {
            state
                .metadata_profile_repository
                .create(chorrosion_domain::MetadataProfile::new("Test Profile"))
                .await
                .expect("create test metadata profile")
        }

        // --- list_metadata_profiles ---

        #[tokio::test]
        async fn list_metadata_profiles_returns_empty_when_none() {
            let state = make_test_state().await;
            let query = ListMetadataProfilesQuery {
                limit: 10,
                offset: 0,
            };
            let result = list_metadata_profiles(State(state), Query(query))
                .await
                .unwrap();
            assert_eq!(result.total, 0);
            assert!(result.items.is_empty());
        }

        #[tokio::test]
        async fn list_metadata_profiles_rejects_invalid_limit() {
            let state = make_test_state().await;
            let query = ListMetadataProfilesQuery {
                limit: 0,
                offset: 0,
            };
            let result = list_metadata_profiles(State(state), Query(query)).await;
            assert!(result.is_err());
            let (status, _) = result.unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn list_metadata_profiles_rejects_negative_offset() {
            let state = make_test_state().await;
            let query = ListMetadataProfilesQuery {
                limit: 10,
                offset: -1,
            };
            let result = list_metadata_profiles(State(state), Query(query)).await;
            assert!(result.is_err());
            let (status, _) = result.unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn list_metadata_profiles_returns_accurate_total_with_pagination() {
            let state = make_test_state().await;
            for name in ["Profile A", "Profile B", "Profile C"] {
                state
                    .metadata_profile_repository
                    .create(chorrosion_domain::MetadataProfile::new(name))
                    .await
                    .unwrap();
            }
            let query = ListMetadataProfilesQuery {
                limit: 2,
                offset: 0,
            };
            let result = list_metadata_profiles(State(state), Query(query))
                .await
                .unwrap();
            assert_eq!(result.total, 3);
            assert_eq!(result.items.len(), 2);
        }

        // --- get_metadata_profile ---

        #[tokio::test]
        async fn get_metadata_profile_returns_200_on_success() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let response = get_metadata_profile(State(state), Path(profile.id.to_string()))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn get_metadata_profile_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = get_metadata_profile(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // --- create_metadata_profile ---

        #[tokio::test]
        async fn create_metadata_profile_returns_201_on_success() {
            let state = make_test_state().await;
            let request = CreateMetadataProfileRequest {
                name: "New Profile".to_string(),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response = create_metadata_profile(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        #[tokio::test]
        async fn create_metadata_profile_returns_400_for_empty_name() {
            let state = make_test_state().await;
            let request = CreateMetadataProfileRequest {
                name: "   ".to_string(),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response = create_metadata_profile(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        // --- update_metadata_profile ---

        #[tokio::test]
        async fn update_metadata_profile_returns_200_on_success() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let request = UpdateMetadataProfileRequest {
                name: Some("Updated Name".to_string()),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response = update_metadata_profile(
                State(state),
                Path(profile.id.to_string()),
                Json(request),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn update_metadata_profile_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let request = UpdateMetadataProfileRequest {
                name: Some("Name".to_string()),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response =
                update_metadata_profile(State(state), Path(unknown_id), Json(request))
                    .await
                    .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn update_metadata_profile_returns_400_for_empty_name() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let request = UpdateMetadataProfileRequest {
                name: Some("  ".to_string()),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response = update_metadata_profile(
                State(state),
                Path(profile.id.to_string()),
                Json(request),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        // --- delete_metadata_profile ---

        #[tokio::test]
        async fn delete_metadata_profile_returns_204_on_success() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let response =
                delete_metadata_profile(State(state), Path(profile.id.to_string()))
                    .await
                    .into_response();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }

        #[tokio::test]
        async fn delete_metadata_profile_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = delete_metadata_profile(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }
}
