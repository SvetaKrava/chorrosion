// SPDX-License-Identifier: GPL-3.0-or-later
pub mod handlers;
pub mod middleware;

/// Base path for all v1 API routes.
pub const API_V1_BASE: &str = "/api/v1";

/// Application version sourced from `Cargo.toml` at compile time.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Json, Router,
};
use chorrosion_application::AppState;
use handlers::activity::{
    get_activity_history, get_activity_processing, get_activity_queue, ActivityItemResponse,
    ActivityListResponse, __path_get_activity_history, __path_get_activity_processing,
    __path_get_activity_queue,
};
use handlers::albums::{
    create_album, delete_album, get_album, list_albums, update_album, AlbumResponse,
    CreateAlbumRequest, ErrorResponse as AlbumErrorResponse, ListAlbumsResponse,
    UpdateAlbumRequest, __path_create_album, __path_delete_album, __path_get_album,
    __path_list_albums, __path_update_album,
};
use handlers::artists::{
    create_artist, delete_artist, get_artist, list_artists, update_artist, ArtistResponse,
    CreateArtistRequest, ErrorResponse, ListArtistsResponse, UpdateArtistRequest,
    __path_create_artist, __path_delete_artist, __path_get_artist, __path_list_artists,
    __path_update_artist,
};
use handlers::indexers::{
    test_indexer_endpoint, IndexerCapabilitiesResponse, IndexerTestErrorResponse,
    TestIndexerRequest, TestIndexerResponse, __path_test_indexer_endpoint,
};
use handlers::metadata_profiles::{
    create_metadata_profile, delete_metadata_profile, get_metadata_profile, list_metadata_profiles,
    update_metadata_profile, CreateMetadataProfileRequest,
    ErrorResponse as MetadataProfileErrorResponse, ListMetadataProfilesResponse,
    MetadataProfileResponse, UpdateMetadataProfileRequest, __path_create_metadata_profile,
    __path_delete_metadata_profile, __path_get_metadata_profile, __path_list_metadata_profiles,
    __path_update_metadata_profile,
};
use handlers::quality_profiles::{
    create_quality_profile, delete_quality_profile, get_quality_profile, list_quality_profiles,
    update_quality_profile, CreateQualityProfileRequest,
    ErrorResponse as QualityProfileErrorResponse, ListQualityProfilesResponse,
    QualityProfileResponse, UpdateQualityProfileRequest, __path_create_quality_profile,
    __path_delete_quality_profile, __path_get_quality_profile, __path_list_quality_profiles,
    __path_update_quality_profile,
};
use handlers::system::{
    get_system_status, get_system_version, SystemStatusResponse, SystemVersionResponse,
    __path_get_system_status, __path_get_system_version,
};
use handlers::tracks::{
    create_track, delete_track, get_track, list_tracks, update_track, CreateTrackRequest,
    ErrorResponse as TrackErrorResponse, ListTracksResponse, TrackResponse, UpdateTrackRequest,
    __path_create_track, __path_delete_track, __path_get_track, __path_list_tracks,
    __path_update_track,
};
use middleware::auth::auth_middleware;
use serde::Serialize;
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Serialize, utoipa::ToSchema)]
struct HealthResponse {
    status: &'static str,
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    ),
    tag = "system"
)]
#[allow(dead_code)]
async fn health() -> Json<HealthResponse> {
    health_handler().await
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        list_artists,
        get_artist,
        create_artist,
        update_artist,
        delete_artist,
        list_albums,
        get_album,
        create_album,
        update_album,
        delete_album,
        list_tracks,
        get_track,
        create_track,
        update_track,
        delete_track,
        get_system_status,
        get_system_version,
        get_activity_queue,
        get_activity_history,
        get_activity_processing,
        list_quality_profiles,
        get_quality_profile,
        create_quality_profile,
        update_quality_profile,
        delete_quality_profile,
        list_metadata_profiles,
        get_metadata_profile,
        create_metadata_profile,
        update_metadata_profile,
        delete_metadata_profile,
        test_indexer_endpoint,
    ),
    components(
        schemas(
            HealthResponse,
            ListArtistsResponse,
            ArtistResponse,
            CreateArtistRequest,
            UpdateArtistRequest,
            ErrorResponse,
            ListAlbumsResponse,
            AlbumResponse,
            CreateAlbumRequest,
            UpdateAlbumRequest,
            AlbumErrorResponse,
            ListTracksResponse,
            TrackResponse,
            CreateTrackRequest,
            UpdateTrackRequest,
            TrackErrorResponse,
            SystemStatusResponse,
            SystemVersionResponse,
            ActivityItemResponse,
            ActivityListResponse,
            ListQualityProfilesResponse,
            QualityProfileResponse,
            CreateQualityProfileRequest,
            UpdateQualityProfileRequest,
            QualityProfileErrorResponse,
            ListMetadataProfilesResponse,
            MetadataProfileResponse,
            CreateMetadataProfileRequest,
            UpdateMetadataProfileRequest,
            MetadataProfileErrorResponse,
            TestIndexerRequest,
            TestIndexerResponse,
            IndexerCapabilitiesResponse,
            IndexerTestErrorResponse,
        )
    ),
    tags(
        (name = "system", description = "System health and status endpoints"),
        (name = "artists", description = "Artist management endpoints"),
        (name = "albums", description = "Album management endpoints"),
        (name = "tracks", description = "Track management endpoints"),
        (name = "activity", description = "Queue and activity endpoints"),
        (name = "settings", description = "Configuration and profile endpoints"),
        (name = "indexers", description = "Indexer configuration and validation endpoints")
    ),
    info(
        title = "Chorrosion API",
        version = "0.1.0",
        description = "High-performance Chorrosion server written in Rust",
    )
)]
struct ApiDoc;

pub fn router(state: AppState) -> Router {
    info!(target: "api", "building router");

    let api_v1 = Router::new()
        .route("/artists", get(list_artists).post(create_artist))
        .route(
            "/artists/:id",
            get(get_artist).put(update_artist).delete(delete_artist),
        )
        .route("/albums", get(list_albums).post(create_album))
        .route(
            "/albums/:id",
            get(get_album).put(update_album).delete(delete_album),
        )
        .route("/tracks", get(list_tracks).post(create_track))
        .route(
            "/tracks/:id",
            get(get_track).put(update_track).delete(delete_track),
        )
        .route("/system/status", get(get_system_status))
        .route("/system/version", get(get_system_version))
        .route("/activity/queue", get(get_activity_queue))
        .route("/activity/history", get(get_activity_history))
        .route("/activity/processing", get(get_activity_processing))
        .route(
            "/settings/quality-profiles",
            get(list_quality_profiles).post(create_quality_profile),
        )
        .route(
            "/settings/quality-profiles/:id",
            get(get_quality_profile)
                .put(update_quality_profile)
                .delete(delete_quality_profile),
        )
        .route(
            "/settings/metadata-profiles",
            get(list_metadata_profiles).post(create_metadata_profile),
        )
        .route(
            "/settings/metadata-profiles/:id",
            get(get_metadata_profile)
                .put(update_metadata_profile)
                .delete(delete_metadata_profile),
        )
        .route("/indexers/test", post(test_indexer_endpoint))
        .layer(axum_middleware::from_fn(auth_middleware));

    let mut openapi = ApiDoc::openapi();
    openapi.info.version = APP_VERSION.to_string();

    Router::new()
        .route("/health", get(health_handler))
        .nest(API_V1_BASE, api_v1)
        .merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", openapi))
        .with_state(state)
}
