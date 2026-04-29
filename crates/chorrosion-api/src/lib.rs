// SPDX-License-Identifier: GPL-3.0-or-later
pub mod handlers;
pub mod middleware;

/// Base path for all v1 API routes.
pub const API_V1_BASE: &str = "/api/v1";

/// Application version sourced from `Cargo.toml` at compile time.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

use axum::{
    http::StatusCode,
    http::{header, HeaderValue, Method},
    middleware as axum_middleware,
    routing::{get, post},
    Json, Router,
};
use chorrosion_application::AppState;
use chorrosion_config::PermissionLevel;
use handlers::activity::{
    get_activity_failed, get_activity_history, get_activity_processing, get_activity_queue,
    get_activity_stalled, ActivityErrorResponse, ActivityItemResponse, ActivityListResponse,
    __path_get_activity_failed, __path_get_activity_history, __path_get_activity_processing,
    __path_get_activity_queue, __path_get_activity_stalled,
};
use handlers::albums::{
    create_album, delete_album, get_album, list_albums, list_albums_by_artist,
    trigger_album_search, update_album, AlbumResponse, CreateAlbumRequest,
    ErrorResponse as AlbumErrorResponse, ListAlbumsResponse, TriggerAlbumSearchResponse,
    UpdateAlbumRequest, __path_create_album, __path_delete_album, __path_get_album,
    __path_list_albums, __path_list_albums_by_artist, __path_trigger_album_search,
    __path_update_album,
};
use handlers::appearance::{
    get_appearance_settings, update_appearance_settings, AppearanceErrorResponse,
    AppearanceSettingsResponse, FilterOperatorApi, ShortcutProfileApi, ThemeModeApi,
    UpdateAppearanceSettingsRequest, __path_get_appearance_settings,
    __path_update_appearance_settings,
};
use handlers::artists::{
    create_artist, delete_artist, get_artist, get_artist_statistics, list_artists, update_artist,
    ArtistResponse, ArtistStatisticsResponse, CreateArtistRequest, ErrorResponse,
    ListArtistsResponse, UpdateArtistRequest, __path_create_artist, __path_delete_artist,
    __path_get_artist, __path_get_artist_statistics, __path_list_artists, __path_update_artist,
};
use handlers::auth::{
    create_api_key, delete_api_key, forms_login, forms_logout, list_api_keys,
    ApiKeyMetadataResponse, ApiKeyResponse, AuthErrorResponse, CreateApiKeyRequest,
    DeleteApiKeyResponse, FormsLoginRequest, FormsLoginResponse, FormsLogoutResponse,
    ListApiKeysResponse, __path_create_api_key, __path_delete_api_key, __path_forms_login,
    __path_forms_logout, __path_list_api_keys,
};
use handlers::calendar::{
    get_ical_feed, list_upcoming_releases, CalendarAlbumResponse, CalendarErrorResponse,
    CalendarResponse, __path_get_ical_feed, __path_list_upcoming_releases,
};
use handlers::download_clients::{
    create_download_client, delete_download_client, get_download_client, list_download_clients,
    update_download_client, CreateDownloadClientRequest, DownloadClientErrorResponse,
    DownloadClientResponse, ListDownloadClientsResponse, UpdateDownloadClientRequest,
    __path_create_download_client, __path_delete_download_client, __path_get_download_client,
    __path_list_download_clients, __path_update_download_client,
};
use handlers::duplicates::{
    get_duplicate_group, list_duplicate_groups, resolve_duplicate_group, DuplicateFileResponse,
    DuplicateGroupDetailResponse, DuplicateGroupQuery, DuplicateGroupResponse,
    ErrorResponse as DuplicateErrorResponse, ListDuplicatesQuery, ListDuplicatesResponse,
    ResolveDuplicateRequest, ResolveDuplicateResponse, __path_get_duplicate_group,
    __path_list_duplicate_groups, __path_resolve_duplicate_group,
};
use handlers::events::{
    __path_get_sse_connections, __path_post_broadcast_event,
    __path_stream_download_progress_events, __path_stream_events,
    __path_stream_import_progress_events, __path_stream_job_status_events, get_sse_connections,
    post_broadcast_event, stream_download_progress_events, stream_events,
    stream_import_progress_events, stream_job_status_events, BroadcastErrorResponse,
    BroadcastEventRequest, BroadcastEventResponse, SseConnectionsResponse,
};
use handlers::imports::{
    evaluate_import_candidate, submit_manual_import_decision, CatalogAlbumMatchResponse,
    ImportCandidateRequest, ImportCandidateResponse, ImportDecisionResponse, ImportErrorResponse,
    ImportRawMetadataRequest, ManualImportDecisionRequest, ManualImportDecisionResponse,
    ParsedMetadataResponse, __path_evaluate_import_candidate, __path_submit_manual_import_decision,
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
use handlers::search::{
    manual_search_endpoint, ManualSearchApiRequest, ManualSearchApiResponse,
    ManualSearchResultItem, SearchErrorResponse, __path_manual_search_endpoint,
};
use handlers::smart_playlists::{
    create_smart_playlist, delete_smart_playlist, get_smart_playlist, get_smart_playlist_items,
    list_smart_playlists, update_smart_playlist, CreateSmartPlaylistRequest,
    ErrorResponse as SmartPlaylistErrorResponse, ListSmartPlaylistsResponse,
    SmartPlaylistCriteriaRequest, SmartPlaylistItemsResponse, SmartPlaylistResponse,
    __path_create_smart_playlist, __path_delete_smart_playlist, __path_get_smart_playlist,
    __path_get_smart_playlist_items, __path_list_smart_playlists, __path_update_smart_playlist,
};
use handlers::system::{
    get_system_logs, get_system_notifications, get_system_status, get_system_tasks,
    get_system_version, post_system_notifications_test, NotificationProviderStatusResponse,
    NotificationStatusResponse, NotificationTestResponse, SystemLogEntryResponse,
    SystemLogsResponse, SystemStatusResponse, SystemTaskResponse, SystemTasksResponse,
    SystemVersionResponse, __path_get_system_logs, __path_get_system_notifications,
    __path_get_system_status, __path_get_system_tasks, __path_get_system_version,
    __path_post_system_notifications_test,
};
use handlers::tags::{
    assign_tag_to_entity, create_tag, delete_tag, get_entity_tags, get_tag, list_tags,
    remove_tag_from_entity, update_tag, CreateTagRequest, EntityTagsResponse,
    ErrorResponse as TagErrorResponse, ListTagsResponse, TagResponse, UpdateTagRequest,
    __path_assign_tag_to_entity, __path_create_tag, __path_delete_tag, __path_get_entity_tags,
    __path_get_tag, __path_list_tags, __path_remove_tag_from_entity, __path_update_tag,
};
use handlers::tracks::{
    create_track, delete_track, get_track, list_tracks, list_tracks_by_album,
    list_tracks_by_artist, update_track, CreateTrackRequest, ErrorResponse as TrackErrorResponse,
    ListTracksResponse, TrackResponse, UpdateTrackRequest, __path_create_track,
    __path_delete_track, __path_get_track, __path_list_tracks, __path_list_tracks_by_album,
    __path_list_tracks_by_artist, __path_update_track,
};
use handlers::wanted::{
    list_cutoff_unmet_albums, list_missing_albums, list_wanted_albums, trigger_wanted_album_search,
    WantedAlbumResponse, WantedAlbumsResponse, WantedErrorResponse, WantedManualSearchResponse,
    __path_list_cutoff_unmet_albums, __path_list_missing_albums, __path_list_wanted_albums,
    __path_trigger_wanted_album_search,
};
use middleware::auth::auth_middleware;
use middleware::metrics::{metrics_handler, metrics_middleware};
use middleware::response_cache::response_cache_middleware;
use middleware::tracing::request_tracing_middleware;
use serde::Serialize;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tracing::{info, warn};
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
struct HealthCheckDependency {
    status: &'static str,
    message: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct HealthResponse {
    status: &'static str,
    database: HealthCheckDependency,
}

async fn health_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> (StatusCode, Json<HealthResponse>) {
    match state.artist_repository.list(0, 0).await {
        Ok(_) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok",
                database: HealthCheckDependency {
                    status: "ok",
                    message: None,
                },
            }),
        ),
        Err(error) => {
            warn!(target: "api", error = %error, "health check database probe failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "degraded",
                    database: HealthCheckDependency {
                        status: "error",
                        message: Some("database probe failed".to_string()),
                    },
                }),
            )
        }
    }
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
        (status = 503, description = "Service is degraded", body = HealthResponse)
    ),
    security(()),
    tag = "system"
)]
#[allow(dead_code)]
async fn health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> (StatusCode, Json<HealthResponse>) {
    health_handler(axum::extract::State(state)).await
}

#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Prometheus metrics in text exposition format", body = String)
    ),
    security(()),
    tag = "system"
)]
#[allow(dead_code)]
async fn metrics() -> axum::response::Response {
    metrics_handler().await
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        metrics,
        list_api_keys,
        create_api_key,
        delete_api_key,
        forms_login,
        forms_logout,
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
        get_system_notifications,
        post_system_notifications_test,
        get_appearance_settings,
        update_appearance_settings,
        get_activity_queue,
        get_activity_history,
        get_activity_failed,
        get_activity_stalled,
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
        manual_search_endpoint,
        evaluate_import_candidate,
        submit_manual_import_decision,
        list_wanted_albums,
        list_missing_albums,
        list_cutoff_unmet_albums,
        trigger_wanted_album_search,
        list_upcoming_releases,
        get_ical_feed,
        create_tag,
        list_tags,
        get_tag,
        update_tag,
        delete_tag,
        get_entity_tags,
        assign_tag_to_entity,
        remove_tag_from_entity,
        list_smart_playlists,
        create_smart_playlist,
        get_smart_playlist,
        update_smart_playlist,
        delete_smart_playlist,
        get_smart_playlist_items,
        list_duplicate_groups,
        get_duplicate_group,
        resolve_duplicate_group,
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
            PermissionLevel,
            FormsLoginRequest,
            FormsLoginResponse,
            FormsLogoutResponse,
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
            NotificationStatusResponse,
            NotificationProviderStatusResponse,
            NotificationTestResponse,
            AppearanceSettingsResponse,
            UpdateAppearanceSettingsRequest,
            AppearanceErrorResponse,
            ThemeModeApi,
            ShortcutProfileApi,
            FilterOperatorApi,
            ActivityItemResponse,
            ActivityListResponse,
            ActivityErrorResponse,
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
            ManualSearchApiRequest,
            ManualSearchResultItem,
            ManualSearchApiResponse,
            SearchErrorResponse,
            ImportErrorResponse,
            ImportRawMetadataRequest,
            ImportCandidateRequest,
            ParsedMetadataResponse,
            CatalogAlbumMatchResponse,
            ImportDecisionResponse,
            ImportCandidateResponse,
            ManualImportDecisionRequest,
            ManualImportDecisionResponse,
            WantedAlbumsResponse,
            WantedAlbumResponse,
            WantedErrorResponse,
            WantedManualSearchResponse,
            CalendarResponse,
            CalendarAlbumResponse,
            CalendarErrorResponse,
            ListTagsResponse,
            TagResponse,
            EntityTagsResponse,
            TagErrorResponse,
            CreateTagRequest,
            UpdateTagRequest,
            ListSmartPlaylistsResponse,
            SmartPlaylistResponse,
            SmartPlaylistCriteriaRequest,
            CreateSmartPlaylistRequest,
            SmartPlaylistItemsResponse,
            SmartPlaylistErrorResponse,
            ListDuplicatesResponse,
            DuplicateGroupResponse,
            DuplicateGroupDetailResponse,
            DuplicateFileResponse,
            ResolveDuplicateRequest,
            ResolveDuplicateResponse,
            DuplicateErrorResponse,
            ListDuplicatesQuery,
            DuplicateGroupQuery,
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
        (name = "search", description = "Manual and interactive search endpoints"),
        (name = "imports", description = "Import evaluation and manual decision endpoints"),
        (name = "wanted", description = "Wanted and missing album tracking"),
        (name = "calendar", description = "Upcoming releases calendar"),
        (name = "tags", description = "Tag organization endpoints"),
        (name = "smart_playlists", description = "Dynamic smart playlist endpoints"),
        (name = "duplicates", description = "Duplicate file detection and management endpoints")
    ),
    modifiers(&SecurityAddon),
    info(
        title = "Chorrosion API",
        version = "0.1.0",
        description = "High-performance Chorrosion server written in Rust",
    )
)]
struct ApiDoc;

fn build_cors_layer(origins: &[String]) -> Option<CorsLayer> {
    let allowed_origins: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|origin| match origin.parse::<HeaderValue>() {
            Ok(value) => Some(value),
            Err(_) => {
                warn!(target: "api", origin = %origin, "ignoring invalid CORS origin");
                None
            }
        })
        .collect();

    if allowed_origins.is_empty() {
        return None;
    }

    Some(
        CorsLayer::new()
            .allow_origin(allowed_origins)
            .allow_credentials(true)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([
                header::ACCEPT,
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                header::HeaderName::from_static("x-api-key"),
            ]),
    )
}

pub fn router(state: AppState) -> Router {
    info!(target: "api", "building router");
    let web_config = state.config.web.clone();

    let api_v1 = Router::new()
        .route("/auth/api-keys", get(list_api_keys).post(create_api_key))
        .route("/auth/api-keys/:id", axum::routing::delete(delete_api_key))
        .route("/auth/forms/login", post(forms_login))
        .route("/auth/forms/logout", post(forms_logout))
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
        .route("/system/notifications", get(get_system_notifications))
        .route(
            "/system/notifications/test",
            post(post_system_notifications_test),
        )
        .route("/activity/queue", get(get_activity_queue))
        .route("/activity/history", get(get_activity_history))
        .route("/activity/failed", get(get_activity_failed))
        .route("/activity/stalled", get(get_activity_stalled))
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
            "/settings/appearance",
            get(get_appearance_settings).put(update_appearance_settings),
        )
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
        .route("/search/manual", post(manual_search_endpoint))
        .route(
            "/smart-playlists",
            get(list_smart_playlists).post(create_smart_playlist),
        )
        .route(
            "/smart-playlists/:playlist_id",
            get(get_smart_playlist)
                .patch(update_smart_playlist)
                .delete(delete_smart_playlist),
        )
        .route(
            "/smart-playlists/:playlist_id/items",
            get(get_smart_playlist_items),
        )
        .route("/duplicates", get(list_duplicate_groups))
        .route("/duplicates/:key", get(get_duplicate_group))
        .route("/duplicates/:key/resolve", post(resolve_duplicate_group))
        .route("/tags", get(list_tags).post(create_tag))
        .route(
            "/tags/:tag_id",
            get(get_tag).patch(update_tag).delete(delete_tag),
        )
        .route("/:entity_type/:entity_id/tags", get(get_entity_tags))
        .route(
            "/:entity_type/:entity_id/tags/:tag_id",
            post(assign_tag_to_entity).delete(remove_tag_from_entity),
        )
        .route("/imports/evaluate", post(evaluate_import_candidate))
        .route("/imports/decision", post(submit_manual_import_decision))
        .route("/wanted", get(list_wanted_albums))
        .route("/wanted/missing", get(list_missing_albums))
        .route("/wanted/cutoff", get(list_cutoff_unmet_albums))
        .route("/wanted/:id/search", post(trigger_wanted_album_search))
        .route("/calendar", get(list_upcoming_releases))
        .route("/calendar/ical", get(get_ical_feed))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            response_cache_middleware,
        ))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let mut openapi = ApiDoc::openapi();
    openapi.info.version = APP_VERSION.to_string();

    let mut app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .nest(API_V1_BASE, api_v1)
        .merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", openapi))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            request_tracing_middleware,
        ))
        .route_layer(axum_middleware::from_fn(metrics_middleware))
        .with_state(state);

    if let Some(cors_layer) = build_cors_layer(&web_config.allowed_origins) {
        app = app.layer(cors_layer);
    }

    if web_config.serve_static_assets {
        let static_dist_dir = PathBuf::from(&web_config.static_dist_dir);
        let index_html = static_dist_dir.join("index.html");
        info!(target: "api", static_dist_dir = %static_dist_dir.display(), "enabling static web asset serving");
        app = app.fallback_service(
            ServeDir::new(&static_dist_dir)
                .append_index_html_on_directories(true)
                .not_found_service(ServeFile::new(index_html)),
        );
    }

    app
}

#[cfg(test)]
mod health_tests {
    use super::*;
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTagRepository, SqliteTaggedEntityRepository,
        SqliteTrackRepository,
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;

    fn make_state_with_pool(pool: SqlitePool) -> AppState {
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
            Arc::new(
                chorrosion_infrastructure::sqlite_adapters::SqliteDuplicateRepository::new(
                    pool.clone(),
                ),
            ),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        )
    }

    #[tokio::test]
    async fn health_returns_ok_when_database_is_ready() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations should run");

        let state = make_state_with_pool(pool);
        let (status, Json(body)) = health_handler(axum::extract::State(state)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.status, "ok");
        assert_eq!(body.database.status, "ok");
        assert!(body.database.message.is_none());
    }

    #[tokio::test]
    async fn health_returns_degraded_when_database_probe_fails() {
        // Intentionally skip migrations to force the repository probe to fail.
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        let state = make_state_with_pool(pool);
        let (status, Json(body)) = health_handler(axum::extract::State(state)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "degraded");
        assert_eq!(body.database.status, "error");
        assert_eq!(
            body.database.message.as_deref(),
            Some("database probe failed")
        );
    }
}
