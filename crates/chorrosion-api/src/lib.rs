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
    create_album, delete_album, get_album, list_albums, list_albums_by_artist,
    trigger_album_search, update_album, AlbumResponse, CreateAlbumRequest,
    ErrorResponse as AlbumErrorResponse, ListAlbumsResponse, TriggerAlbumSearchResponse,
    UpdateAlbumRequest, __path_create_album, __path_delete_album, __path_get_album,
    __path_list_albums, __path_list_albums_by_artist, __path_trigger_album_search,
    __path_update_album,
};
use handlers::artists::{
    create_artist, delete_artist, get_artist, get_artist_statistics, list_artists, update_artist,
    ArtistResponse, ArtistStatisticsResponse, CreateArtistRequest, ErrorResponse,
    ListArtistsResponse, UpdateArtistRequest, __path_create_artist, __path_delete_artist,
    __path_get_artist, __path_get_artist_statistics, __path_list_artists, __path_update_artist,
};
use handlers::auth::{
    create_api_key, delete_api_key, list_api_keys, ApiKeyMetadataResponse, ApiKeyResponse,
    AuthErrorResponse, CreateApiKeyRequest, DeleteApiKeyResponse, ListApiKeysResponse,
    __path_create_api_key, __path_delete_api_key, __path_list_api_keys,
};
use handlers::download_clients::{
    create_download_client, delete_download_client, get_download_client, list_download_clients,
    update_download_client, CreateDownloadClientRequest, DownloadClientErrorResponse,
    DownloadClientResponse, ListDownloadClientsResponse, UpdateDownloadClientRequest,
    __path_create_download_client, __path_delete_download_client, __path_get_download_client,
    __path_list_download_clients, __path_update_download_client,
};
use handlers::events::{
    __path_get_sse_connections, __path_post_broadcast_event,
    __path_stream_download_progress_events, __path_stream_events,
    __path_stream_import_progress_events, __path_stream_job_status_events, get_sse_connections,
    post_broadcast_event, stream_download_progress_events, stream_events,
    stream_import_progress_events, stream_job_status_events, BroadcastErrorResponse,
    BroadcastEventRequest, BroadcastEventResponse, SseConnectionsResponse,
};
use handlers::indexers::{
    create_indexer, delete_indexer, get_indexer, list_indexers, test_indexer_endpoint,
    update_indexer, CreateIndexerRequest, IndexerCapabilitiesResponse, IndexerErrorResponse,
    IndexerResponse, IndexerTestErrorResponse, ListIndexersResponse, TestIndexerRequest,
    TestIndexerResponse, UpdateIndexerRequest, __path_create_indexer, __path_delete_indexer,
    __path_get_indexer, __path_list_indexers, __path_test_indexer_endpoint, __path_update_indexer,
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
    get_system_logs, get_system_status, get_system_tasks, get_system_version,
    SystemLogEntryResponse, SystemLogsResponse, SystemStatusResponse, SystemTaskResponse,
    SystemTasksResponse, SystemVersionResponse, __path_get_system_logs, __path_get_system_status,
    __path_get_system_tasks, __path_get_system_version,
};
use handlers::tracks::{
    create_track, delete_track, get_track, list_tracks, list_tracks_by_album,
    list_tracks_by_artist, update_track, CreateTrackRequest, ErrorResponse as TrackErrorResponse,
    ListTracksResponse, TrackResponse, UpdateTrackRequest, __path_create_track,
    __path_delete_track, __path_get_track, __path_list_tracks, __path_list_tracks_by_album,
    __path_list_tracks_by_artist, __path_update_track,
};
use handlers::wanted::{
    list_missing_albums, list_wanted_albums, WantedAlbumResponse, WantedAlbumsResponse,
    WantedErrorResponse, __path_list_missing_albums, __path_list_wanted_albums,
};
use middleware::auth::auth_middleware;
use serde::Serialize;
use tracing::info;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// OpenAPI modifier that registers the API key / Bearer security schemes and
/// applies a default global security requirement for all operations.
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::new);
        components.add_security_scheme(
            "ApiKeyAuth",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-Api-Key"))),
        );
        components.add_security_scheme(
            "BearerAuth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
        // Apply a default global security requirement (api key OR bearer).
        openapi.security = Some(vec![
            utoipa::openapi::security::SecurityRequirement::new::<&str, [&str; 0], &str>(
                "ApiKeyAuth",
                [],
            ),
            utoipa::openapi::security::SecurityRequirement::new::<&str, [&str; 0], &str>(
                "BearerAuth",
                [],
            ),
        ]);
    }
}

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
    security(()),
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
        list_api_keys,
        create_api_key,
        delete_api_key,
        list_artists,
        get_artist,
        get_artist_statistics,
        create_artist,
        update_artist,
        delete_artist,
        list_albums,
        list_albums_by_artist,
        get_album,
        create_album,
        update_album,
        delete_album,
        trigger_album_search,
        list_tracks,
        list_tracks_by_album,
        list_tracks_by_artist,
        get_track,
        create_track,
        update_track,
        delete_track,
        get_system_status,
        get_system_version,
        get_system_tasks,
        get_system_logs,
        get_activity_queue,
        get_activity_history,
        get_activity_processing,
        stream_events,
        get_sse_connections,
        post_broadcast_event,
        stream_download_progress_events,
        stream_import_progress_events,
        stream_job_status_events,
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
        list_download_clients,
        get_download_client,
        create_download_client,
        update_download_client,
        delete_download_client,
        list_indexers,
        get_indexer,
        create_indexer,
        update_indexer,
        delete_indexer,
        test_indexer_endpoint,
        list_wanted_albums,
        list_missing_albums,
    ),
    components(
        schemas(
            HealthResponse,
            ListApiKeysResponse,
            ApiKeyResponse,
            ApiKeyMetadataResponse,
            CreateApiKeyRequest,
            DeleteApiKeyResponse,
            AuthErrorResponse,
            BroadcastEventRequest,
            BroadcastEventResponse,
            ListArtistsResponse,
            ArtistResponse,
            ArtistStatisticsResponse,
            CreateArtistRequest,
            UpdateArtistRequest,
            ErrorResponse,
            ListAlbumsResponse,
            AlbumResponse,
            CreateAlbumRequest,
            UpdateAlbumRequest,
            TriggerAlbumSearchResponse,
            AlbumErrorResponse,
            ListTracksResponse,
            TrackResponse,
            CreateTrackRequest,
            UpdateTrackRequest,
            TrackErrorResponse,
            SystemStatusResponse,
            SystemVersionResponse,
            SystemTasksResponse,
            SystemTaskResponse,
            SystemLogsResponse,
            SystemLogEntryResponse,
            ActivityItemResponse,
            ActivityListResponse,
            BroadcastErrorResponse,
            SseConnectionsResponse,
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
            ListDownloadClientsResponse,
            DownloadClientResponse,
            CreateDownloadClientRequest,
            UpdateDownloadClientRequest,
            DownloadClientErrorResponse,
            ListIndexersResponse,
            IndexerResponse,
            CreateIndexerRequest,
            UpdateIndexerRequest,
            IndexerErrorResponse,
            TestIndexerRequest,
            TestIndexerResponse,
            IndexerCapabilitiesResponse,
            IndexerTestErrorResponse,
            WantedAlbumsResponse,
            WantedAlbumResponse,
            WantedErrorResponse,
        )
    ),
    tags(
        (name = "system", description = "System health and status endpoints"),
        (name = "artists", description = "Artist management endpoints"),
        (name = "albums", description = "Album management endpoints"),
        (name = "tracks", description = "Track management endpoints"),
        (name = "activity", description = "Queue and activity endpoints"),
        (name = "auth", description = "Authentication and API key management endpoints"),
        (name = "settings", description = "Configuration and profile endpoints"),
        (name = "indexers", description = "Indexer configuration and validation endpoints"),
        (name = "wanted", description = "Wanted and missing album tracking")
    ),
    modifiers(&SecurityAddon),
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
        .route("/auth/api-keys", get(list_api_keys).post(create_api_key))
        .route("/auth/api-keys/:id", axum::routing::delete(delete_api_key))
        .route("/artists", get(list_artists).post(create_artist))
        .route(
            "/artists/:id",
            get(get_artist).put(update_artist).delete(delete_artist),
        )
        .route("/artists/:id/statistics", get(get_artist_statistics))
        .route("/albums", get(list_albums).post(create_album))
        .route(
            "/albums/:id",
            get(get_album).put(update_album).delete(delete_album),
        )
        .route("/albums/:id/search", post(trigger_album_search))
        .route("/artists/:artist_id/albums", get(list_albums_by_artist))
        .route("/tracks", get(list_tracks).post(create_track))
        .route(
            "/tracks/:id",
            get(get_track).put(update_track).delete(delete_track),
        )
        .route("/albums/:album_id/tracks", get(list_tracks_by_album))
        .route("/artists/:artist_id/tracks", get(list_tracks_by_artist))
        .route("/system/status", get(get_system_status))
        .route("/system/version", get(get_system_version))
        .route("/system/tasks", get(get_system_tasks))
        .route("/system/logs", get(get_system_logs))
        .route("/activity/queue", get(get_activity_queue))
        .route("/activity/history", get(get_activity_history))
        .route("/activity/processing", get(get_activity_processing))
        .route("/events", get(stream_events))
        .route("/events/connections", get(get_sse_connections))
        .route("/events/broadcast", post(post_broadcast_event))
        .route(
            "/events/download-progress",
            get(stream_download_progress_events),
        )
        .route(
            "/events/import-progress",
            get(stream_import_progress_events),
        )
        .route("/events/job-status", get(stream_job_status_events))
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
        .route(
            "/settings/download-clients",
            get(list_download_clients).post(create_download_client),
        )
        .route(
            "/settings/download-clients/:id",
            get(get_download_client)
                .put(update_download_client)
                .delete(delete_download_client),
        )
        .route(
            "/settings/indexers",
            get(list_indexers).post(create_indexer),
        )
        .route(
            "/settings/indexers/:id",
            get(get_indexer).put(update_indexer).delete(delete_indexer),
        )
        .route("/indexers/test", post(test_indexer_endpoint))
        .route("/wanted", get(list_wanted_albums))
        .route("/wanted/missing", get(list_missing_albums))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let mut openapi = ApiDoc::openapi();
    openapi.info.version = APP_VERSION.to_string();

    Router::new()
        .route("/health", get(health_handler))
        .nest(API_V1_BASE, api_v1)
        .merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", openapi))
        .with_state(state)
}
