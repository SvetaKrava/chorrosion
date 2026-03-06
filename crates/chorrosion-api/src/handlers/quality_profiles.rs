// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::QualityProfile;
use chorrosion_infrastructure::repositories::Repository;
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListQualityProfilesQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct QualityProfileResponse {
    pub id: String,
    pub name: String,
    pub allowed_qualities: Vec<String>,
    pub upgrade_allowed: bool,
    pub cutoff_quality: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListQualityProfilesResponse {
    pub items: Vec<QualityProfileResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl From<QualityProfile> for QualityProfileResponse {
    fn from(profile: QualityProfile) -> Self {
        Self {
            id: profile.id.to_string(),
            name: profile.name,
            allowed_qualities: profile.allowed_qualities,
            upgrade_allowed: profile.upgrade_allowed,
            cutoff_quality: profile.cutoff_quality,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateQualityProfileRequest {
    pub name: String,
    pub allowed_qualities: Vec<String>,
    pub upgrade_allowed: Option<bool>,
    pub cutoff_quality: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateQualityProfileRequest {
    pub name: Option<String>,
    pub allowed_qualities: Option<Vec<String>>,
    pub upgrade_allowed: Option<bool>,
    pub cutoff_quality: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = QualityProfileErrorResponse)]
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

fn validate_allowed_qualities(
    allowed_qualities: &[String],
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if allowed_qualities.is_empty() || allowed_qualities.iter().all(|q| q.trim().is_empty()) {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "allowed_qualities must contain at least one non-empty value".to_string(),
            }),
        ))
    } else {
        Ok(())
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/quality-profiles",
    params(ListQualityProfilesQuery),
    responses(
        (status = 200, description = "List quality profiles", body = ListQualityProfilesResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn list_quality_profiles(
    State(state): State<AppState>,
    Query(query): Query<ListQualityProfilesQuery>,
) -> Result<Json<ListQualityProfilesResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", ?query, "listing quality profiles");

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
        .quality_profile_repository
        .list(5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list quality profiles: {error}"),
                }),
            )
        })?;

    let total = all_profiles.len() as i64;
    let offset = query.offset as usize;
    let limit = query.limit as usize;
    let items = all_profiles
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(QualityProfileResponse::from)
        .collect();

    Ok(Json(ListQualityProfilesResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/quality-profiles/{id}",
    params(("id" = String, Path, description = "Quality profile ID")),
    responses(
        (status = 200, description = "Quality profile found", body = QualityProfileResponse),
        (status = 404, description = "Quality profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn get_quality_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "fetching quality profile");

    match state.quality_profile_repository.get_by_id(id.clone()).await {
        Ok(Some(profile)) => {
            (StatusCode::OK, Json(QualityProfileResponse::from(profile))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Quality profile {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch quality profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/quality-profiles",
    request_body = CreateQualityProfileRequest,
    responses(
        (status = 201, description = "Quality profile created", body = QualityProfileResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn create_quality_profile(
    State(state): State<AppState>,
    Json(request): Json<CreateQualityProfileRequest>,
) -> impl IntoResponse {
    debug!(target: "api", ?request, "creating quality profile");

    if let Err(err_response) = validate_name(&request.name) {
        return err_response.into_response();
    }
    if let Err(err_response) = validate_allowed_qualities(&request.allowed_qualities) {
        return err_response.into_response();
    }

    let mut profile = QualityProfile::new(request.name, request.allowed_qualities);
    profile.upgrade_allowed = request.upgrade_allowed.unwrap_or(false);
    profile.cutoff_quality = request.cutoff_quality;

    match state.quality_profile_repository.create(profile).await {
        Ok(created) => (
            StatusCode::CREATED,
            Json(QualityProfileResponse::from(created)),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to create quality profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/quality-profiles/{id}",
    params(("id" = String, Path, description = "Quality profile ID")),
    request_body = UpdateQualityProfileRequest,
    responses(
        (status = 200, description = "Quality profile updated", body = QualityProfileResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Quality profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn update_quality_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateQualityProfileRequest>,
) -> impl IntoResponse {
    debug!(target: "api", %id, ?request, "updating quality profile");

    let mut profile = match state.quality_profile_repository.get_by_id(id.clone()).await {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Quality profile {} not found", id),
                }),
            )
                .into_response()
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch quality profile: {error}"),
                }),
            )
                .into_response()
        }
    };

    if let Some(name) = request.name {
        if let Err(err_response) = validate_name(&name) {
            return err_response.into_response();
        }
        profile.name = name;
    }
    if let Some(allowed_qualities) = request.allowed_qualities {
        if let Err(err_response) = validate_allowed_qualities(&allowed_qualities) {
            return err_response.into_response();
        }
        profile.allowed_qualities = allowed_qualities;
    }
    if let Some(upgrade_allowed) = request.upgrade_allowed {
        profile.upgrade_allowed = upgrade_allowed;
    }
    if let Some(cutoff_quality) = request.cutoff_quality {
        profile.cutoff_quality = Some(cutoff_quality);
    }

    match state.quality_profile_repository.update(profile).await {
        Ok(updated) => {
            (StatusCode::OK, Json(QualityProfileResponse::from(updated))).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to update quality profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/settings/quality-profiles/{id}",
    params(("id" = String, Path, description = "Quality profile ID")),
    responses(
        (status = 204, description = "Quality profile deleted"),
        (status = 404, description = "Quality profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn delete_quality_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "deleting quality profile");

    match state.quality_profile_repository.delete(id.clone()).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => {
            let err_msg = error.to_string();
            if err_msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("Quality profile {} not found", id),
                    }),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to delete quality profile: {error}"),
                    }),
                )
                    .into_response()
            }
        }
    }
}
