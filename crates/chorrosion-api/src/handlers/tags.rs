// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chorrosion_application::{AppState, EntityType, Tag, TagId};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};
use utoipa::{IntoParams, ToSchema};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListTagsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TagResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateTagRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateTagRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AssignTagRequest {
    pub tag_id: String,
    pub entity_id: String,
    pub entity_type: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
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

fn parse_entity_type(
    entity_type_str: &str,
) -> Result<EntityType, (StatusCode, Json<ErrorResponse>)> {
    match entity_type_str.to_lowercase().as_str() {
        "artist" => Ok(EntityType::Artist),
        "album" => Ok(EntityType::Album),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "Invalid entity type",
        )),
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListTagsResponse {
    pub items: Vec<TagResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EntityTagsResponse {
    pub entity_id: String,
    pub entity_type: String,
    pub tags: Vec<TagResponse>,
}

impl From<Tag> for TagResponse {
    fn from(tag: Tag) -> Self {
        Self {
            id: tag.id.to_string(),
            name: tag.name,
            description: tag.description,
            created_at: tag.created_at.to_rfc3339(),
            updated_at: tag.updated_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Handler Functions
// ============================================================================

/// Create a new tag
#[utoipa::path(
    post,
    path = "/api/v1/tags",
    request_body = CreateTagRequest,
    responses(
        (status = 201, description = "Tag created successfully", body = TagResponse),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Tag with this name already exists"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Tags"
)]
pub async fn create_tag(
    State(state): State<AppState>,
    Json(payload): Json<CreateTagRequest>,
) -> Result<(StatusCode, Json<TagResponse>), (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", "creating tag: {}", payload.name);

    if payload.name.trim().is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Tag name cannot be empty",
        ));
    }

    let tag = Tag::new(payload.name, payload.description);

    match state.tag_repository.create(tag).await {
        Ok(created_tag) => Ok((StatusCode::CREATED, Json(TagResponse::from(created_tag)))),
        Err(e) => {
            error!(target: "api", "failed to create tag: {}", e);
            if e.to_string().contains("UNIQUE") {
                Err(error_response(
                    StatusCode::CONFLICT,
                    "Tag with this name already exists",
                ))
            } else {
                Err(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to create tag",
                ))
            }
        }
    }
}

/// List all tags
#[utoipa::path(
    get,
    path = "/api/v1/tags",
    params(ListTagsQuery),
    responses(
        (status = 200, description = "List of tags", body = ListTagsResponse),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Tags"
)]
pub async fn list_tags(
    State(state): State<AppState>,
    Query(params): Query<ListTagsQuery>,
) -> Result<Json<ListTagsResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api",
        "listing tags: limit={}, offset={}",
        params.limit, params.offset
    );

    let limit = params.limit.clamp(1, 1000);
    let offset = params.offset.max(0);

    match state.tag_repository.list(5000, 0).await {
        Ok(tags) => {
            let total = tags.len() as i64;
            let items = tags
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .map(TagResponse::from)
                .collect();
            Ok(Json(ListTagsResponse {
                items,
                total,
                limit,
                offset,
            }))
        }
        Err(e) => {
            error!(target: "api", "failed to list tags: {}", e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to list tags",
            ))
        }
    }
}

/// Get a tag by ID
#[utoipa::path(
    get,
    path = "/api/v1/tags/{tag_id}",
    params(
        ("tag_id" = String, Path, description = "Tag ID"),
    ),
    responses(
        (status = 200, description = "Tag details", body = TagResponse),
        (status = 404, description = "Tag not found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Tags"
)]
pub async fn get_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<String>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", "getting tag: {}", tag_id);

    match state.tag_repository.get_by_id(&tag_id).await {
        Ok(Some(tag)) => Ok(Json(TagResponse::from(tag))),
        Ok(None) => Err(error_response(StatusCode::NOT_FOUND, "Tag not found")),
        Err(e) => {
            error!(target: "api", "failed to get tag: {}", e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get tag",
            ))
        }
    }
}

/// Update a tag
#[utoipa::path(
    patch,
    path = "/api/v1/tags/{tag_id}",
    request_body = UpdateTagRequest,
    params(
        ("tag_id" = String, Path, description = "Tag ID"),
    ),
    responses(
        (status = 200, description = "Tag updated successfully", body = TagResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Tag not found"),
        (status = 409, description = "Tag with this name already exists"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Tags"
)]
pub async fn update_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<String>,
    Json(payload): Json<UpdateTagRequest>,
) -> Result<Json<TagResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", "updating tag: {}", tag_id);

    match state.tag_repository.get_by_id(&tag_id).await {
        Ok(Some(mut tag)) => {
            if let Some(name) = payload.name {
                if name.trim().is_empty() {
                    return Err(error_response(
                        StatusCode::BAD_REQUEST,
                        "Tag name cannot be empty",
                    ));
                }
                tag.name = name;
            }
            if payload.description.is_some() {
                tag.description = payload.description;
            }
            tag.updated_at = chrono::Utc::now();

            match state.tag_repository.update(tag).await {
                Ok(updated_tag) => Ok(Json(TagResponse::from(updated_tag))),
                Err(e) => {
                    error!(target: "api", "failed to update tag: {}", e);
                    if e.to_string().contains("UNIQUE") {
                        Err(error_response(
                            StatusCode::CONFLICT,
                            "Tag with this name already exists",
                        ))
                    } else {
                        Err(error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to update tag",
                        ))
                    }
                }
            }
        }
        Ok(None) => Err(error_response(StatusCode::NOT_FOUND, "Tag not found")),
        Err(e) => {
            error!(target: "api", "failed to get tag: {}", e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update tag",
            ))
        }
    }
}

/// Delete a tag
#[utoipa::path(
    delete,
    path = "/api/v1/tags/{tag_id}",
    params(
        ("tag_id" = String, Path, description = "Tag ID"),
    ),
    responses(
        (status = 204, description = "Tag deleted successfully"),
        (status = 404, description = "Tag not found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Tags"
)]
pub async fn delete_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", "deleting tag: {}", tag_id);

    match state.tag_repository.delete(&tag_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!(target: "api", "failed to delete tag: {}", e);
            if e.to_string().contains("not found") {
                Err(error_response(StatusCode::NOT_FOUND, "Tag not found"))
            } else {
                Err(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to delete tag",
                ))
            }
        }
    }
}

/// Get all tags for an entity (artist or album)
#[utoipa::path(
    get,
    path = "/api/v1/{entity_type}/{entity_id}/tags",
    params(
        ("entity_type" = String, Path, description = "Entity type (artist or album)"),
        ("entity_id" = String, Path, description = "Entity ID"),
    ),
    responses(
        (status = 200, description = "List of tags for entity", body = EntityTagsResponse),
        (status = 400, description = "Invalid entity type"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Tags"
)]
pub async fn get_entity_tags(
    State(state): State<AppState>,
    Path((entity_type_str, entity_id)): Path<(String, String)>,
) -> Result<Json<EntityTagsResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", "getting tags for {}/{}", entity_type_str, entity_id);

    let entity_type = parse_entity_type(&entity_type_str)?;

    match state
        .tag_repository
        .get_tags_for_entity(&entity_id, entity_type)
        .await
    {
        Ok(tags) => Ok(Json(EntityTagsResponse {
            entity_id: entity_id.clone(),
            entity_type: entity_type_str,
            tags: tags.into_iter().map(TagResponse::from).collect(),
        })),
        Err(e) => {
            error!(target: "api", "failed to get tags for entity: {}", e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get entity tags",
            ))
        }
    }
}

/// Assign a tag to an entity
#[utoipa::path(
    post,
    path = "/api/v1/{entity_type}/{entity_id}/tags/{tag_id}",
    params(
        ("entity_type" = String, Path, description = "Entity type (artist or album)"),
        ("entity_id" = String, Path, description = "Entity ID"),
        ("tag_id" = String, Path, description = "Tag ID"),
    ),
    responses(
        (status = 204, description = "Tag assigned successfully"),
        (status = 400, description = "Invalid entity type"),
        (status = 404, description = "Tag or entity not found"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Tags"
)]
pub async fn assign_tag_to_entity(
    State(state): State<AppState>,
    Path((entity_type_str, entity_id, tag_id)): Path<(String, String, String)>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api",
        "assigning tag {} to {}/{}",
        tag_id, entity_type_str, entity_id
    );

    let entity_type = parse_entity_type(&entity_type_str)?;

    // Verify tag exists
    match state.tag_repository.get_by_id(&tag_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return Err(error_response(StatusCode::NOT_FOUND, "Tag not found")),
        Err(e) => {
            error!(target: "api", "failed to get tag: {}", e);
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to verify tag",
            ));
        }
    }

    let entity_exists = match entity_type {
        EntityType::Artist => state
            .artist_repository
            .get_by_id(&entity_id)
            .await
            .map(|entity| entity.is_some()),
        EntityType::Album => state
            .album_repository
            .get_by_id(&entity_id)
            .await
            .map(|entity| entity.is_some()),
    };

    match entity_exists {
        Ok(true) => {}
        Ok(false) => return Err(error_response(StatusCode::NOT_FOUND, "Entity not found")),
        Err(e) => {
            error!(target: "api", "failed to verify entity: {}", e);
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to verify entity",
            ));
        }
    }

    let parsed_tag_id = TagId::from_uuid(
        uuid::Uuid::parse_str(&tag_id)
            .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid tag ID format"))?,
    );

    match state
        .tagged_entity_repository
        .assign_tag(parsed_tag_id, &entity_id, entity_type)
        .await
    {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!(target: "api", "failed to assign tag: {}", e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to assign tag",
            ))
        }
    }
}

/// Remove a tag from an entity
#[utoipa::path(
    delete,
    path = "/api/v1/{entity_type}/{entity_id}/tags/{tag_id}",
    params(
        ("entity_type" = String, Path, description = "Entity type (artist or album)"),
        ("entity_id" = String, Path, description = "Entity ID"),
        ("tag_id" = String, Path, description = "Tag ID"),
    ),
    responses(
        (status = 204, description = "Tag removed successfully"),
        (status = 400, description = "Invalid entity type"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "Tags"
)]
pub async fn remove_tag_from_entity(
    State(state): State<AppState>,
    Path((entity_type_str, entity_id, tag_id)): Path<(String, String, String)>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api",
        "removing tag {} from {}/{}",
        tag_id, entity_type_str, entity_id
    );

    let entity_type = parse_entity_type(&entity_type_str)?;

    let parsed_tag_id = TagId::from_uuid(
        uuid::Uuid::parse_str(&tag_id)
            .map_err(|_| error_response(StatusCode::BAD_REQUEST, "Invalid tag ID format"))?,
    );

    match state
        .tagged_entity_repository
        .remove_tag(parsed_tag_id, &entity_id, entity_type)
        .await
    {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!(target: "api", "failed to remove tag: {}", e);
            Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to remove tag",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, Query, State};
    use chorrosion_config::AppConfig;
    use chorrosion_domain::Tag as DomainTag;
    use chorrosion_infrastructure::{
        sqlite_adapters::{
            SqliteAlbumRepository, SqliteArtistRepository,
            SqliteDownloadClientDefinitionRepository, SqliteIndexerDefinitionRepository,
            SqliteMetadataProfileRepository, SqliteQualityProfileRepository, SqliteTagRepository,
            SqliteTaggedEntityRepository, SqliteTrackRepository,
        },
        ResponseCache,
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
            ResponseCache::new(100, 60),
        )
    }

    #[tokio::test]
    async fn list_tags_returns_total_count_across_all_rows() {
        let state = make_test_state().await;
        for i in 0..3 {
            state
                .tag_repository
                .create(DomainTag::new(format!("Tag {i}"), None))
                .await
                .expect("create tag");
        }

        let response = list_tags(
            State(state),
            Query(ListTagsQuery {
                limit: 1,
                offset: 1,
            }),
        )
        .await
        .expect("list tags");

        assert_eq!(response.total, 3);
        assert_eq!(response.items.len(), 1);
    }

    #[tokio::test]
    async fn delete_tag_returns_not_found_for_missing_tag() {
        let state = make_test_state().await;
        let result = delete_tag(State(state), Path(uuid::Uuid::new_v4().to_string())).await;
        assert!(matches!(result, Err((StatusCode::NOT_FOUND, _))));
    }

    #[tokio::test]
    async fn assign_tag_returns_not_found_for_missing_entity() {
        let state = make_test_state().await;
        let tag = state
            .tag_repository
            .create(DomainTag::new("tag".to_string(), None))
            .await
            .expect("create tag");

        let result = assign_tag_to_entity(
            State(state),
            Path((
                "artist".to_string(),
                uuid::Uuid::new_v4().to_string(),
                tag.id.to_string(),
            )),
        )
        .await;

        assert!(matches!(result, Err((StatusCode::NOT_FOUND, _))));
    }
}
