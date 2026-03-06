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

    match state.metadata_profile_repository.delete(id.clone()).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            let err_msg = error.to_string();
            if err_msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("Metadata profile {} not found", id),
                    }),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to delete metadata profile: {error}"),
                    }),
                )
                    .into_response()
            }
        }
    }
}
