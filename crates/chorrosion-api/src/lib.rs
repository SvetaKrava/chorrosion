pub mod handlers;
pub mod middleware;

use axum::{middleware as axum_middleware, routing::get, Json, Router};
use chorrosion_application::AppState;
use handlers::artists::{
    create_artist, delete_artist, get_artist, list_artists, update_artist, ArtistResponse,
    CreateArtistRequest, ErrorResponse, UpdateArtistRequest, __path_create_artist,
    __path_delete_artist, __path_get_artist, __path_list_artists, __path_update_artist,
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
    ),
    components(
        schemas(
            HealthResponse,
            ArtistResponse,
            CreateArtistRequest,
            UpdateArtistRequest,
            ErrorResponse,
        )
    ),
    tags(
        (name = "system", description = "System health and status endpoints"),
        (name = "artists", description = "Artist management endpoints")
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
        .layer(axum_middleware::from_fn(auth_middleware));

    let openapi = ApiDoc::openapi();

    Router::new()
        .route("/health", get(health_handler))
        .nest("/api/v1", api_v1)
        .merge(SwaggerUi::new("/docs").url("/api-doc/openapi.json", openapi))
        .with_state(state)
}
