// SPDX-License-Identifier: GPL-3.0-or-later
//! HTTP request performance-tracing middleware.
//!
//! [`request_tracing_middleware`] wraps every request in a timing scope and
//! emits two kinds of structured [`tracing`] events:
//!
//! * **INFO** `"request completed"` — always emitted; carries `method`,
//!   `route`, `status`, and `elapsed_ms`.
//! * **WARN** `"slow request detected"` — emitted when `elapsed_ms` reaches
//!   the configured `telemetry.slow_request_threshold_ms`.  Set that value to
//!   `0` to suppress the warning entirely.
//!
//! The middleware is registered with `axum_middleware::from_fn_with_state` so
//! it can read the threshold directly from [`AppState::config`].  Route labels
//! are taken from Axum's [`MatchedPath`] extension when available (i.e. for
//! grouped routes); raw URI paths are used as a fallback for unmatched paths
//! such as `/health` and `/metrics`.

use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};
use chorrosion_application::AppState;
use std::time::Instant;
use tracing::{info, warn};

/// Middleware function — register with
/// `axum_middleware::from_fn_with_state(state.clone(), request_tracing_middleware)`.
pub async fn request_tracing_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or_else(|| req.uri().path())
        .to_owned();
    let started_at = Instant::now();

    let response = next.run(req).await;

    let status = response.status().as_u16();
    let elapsed_ms = started_at.elapsed().as_millis() as u64;

    info!(
        target: "api",
        method = method.as_str(),
        route = %route,
        status,
        elapsed_ms,
        "request completed"
    );

    let threshold_ms = state.config.telemetry.slow_request_threshold_ms;
    if threshold_ms > 0 && elapsed_ms >= threshold_ms {
        warn!(
            target: "api",
            method = method.as_str(),
            route = %route,
            status,
            elapsed_ms,
            threshold_ms,
            "slow request detected"
        );
    }

    response
}

#[cfg(test)]
mod tests {
    use super::request_tracing_middleware;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware as axum_middleware,
        routing::get,
        Router,
    };
    use chorrosion_application::AppState;
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTagRepository, SqliteTaggedEntityRepository,
        SqliteTrackRepository,
    };
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    fn make_state(pool: SqlitePool) -> AppState {
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
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        )
    }

    async fn pool() -> SqlitePool {
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool")
    }

    async fn ok_handler() -> &'static str {
        "ok"
    }

    async fn not_found_handler() -> StatusCode {
        StatusCode::NOT_FOUND
    }

    #[tokio::test]
    async fn middleware_passes_2xx_response_through() {
        let state = make_state(pool().await);
        let app = Router::new()
            .route("/ping", get(ok_handler))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                request_tracing_middleware,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ping")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_passes_non_2xx_response_through() {
        let state = make_state(pool().await);
        let app = Router::new()
            .route("/gone", get(not_found_handler))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                request_tracing_middleware,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/gone")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn middleware_completes_normally_when_threshold_disabled() {
        // threshold_ms = 0 disables slow-request warnings; verify the request
        // still completes and the response passes through unchanged.
        let mut config = AppConfig::default();
        config.telemetry.slow_request_threshold_ms = 0;

        let pool_handle = pool().await;
        let state = AppState::new(
            config,
            Arc::new(SqliteArtistRepository::new(pool_handle.clone())),
            Arc::new(SqliteAlbumRepository::new(pool_handle.clone())),
            Arc::new(SqliteTrackRepository::new(pool_handle.clone())),
            Arc::new(SqliteQualityProfileRepository::new(pool_handle.clone())),
            Arc::new(SqliteMetadataProfileRepository::new(pool_handle.clone())),
            Arc::new(SqliteIndexerDefinitionRepository::new(pool_handle.clone())),
            Arc::new(SqliteDownloadClientDefinitionRepository::new(
                pool_handle.clone(),
            )),
            Arc::new(SqliteTagRepository::new(pool_handle.clone())),
            Arc::new(SqliteTaggedEntityRepository::new(pool_handle.clone())),
            Arc::new(
                chorrosion_infrastructure::sqlite_adapters::SqliteSmartPlaylistRepository::new(
                    pool_handle.clone(),
                ),
            ),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        );

        let app = Router::new()
            .route("/ping", get(ok_handler))
            .route_layer(axum_middleware::from_fn_with_state(
                state.clone(),
                request_tracing_middleware,
            ))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/ping")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
