// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{extract::State, Json};
use chorrosion_application::AppState;
use serde::Serialize;
use tracing::debug;
use utoipa::ToSchema;

use crate::{API_V1_BASE, APP_VERSION};

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemStatusResponse {
    pub status: &'static str,
    pub api_base: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemVersionResponse {
    pub name: &'static str,
    pub version: &'static str,
}

#[utoipa::path(
    get,
    path = "/api/v1/system/status",
    responses(
        (status = 200, description = "System status", body = SystemStatusResponse)
    ),
    tag = "system"
)]
pub async fn get_system_status(State(_state): State<AppState>) -> Json<SystemStatusResponse> {
    debug!(target: "api", "fetching system status");
    Json(SystemStatusResponse {
        status: "ok",
        api_base: API_V1_BASE,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/system/version",
    responses(
        (status = 200, description = "System version", body = SystemVersionResponse)
    ),
    tag = "system"
)]
pub async fn get_system_version(State(_state): State<AppState>) -> Json<SystemVersionResponse> {
    debug!(target: "api", "fetching system version");
    Json(SystemVersionResponse {
        name: "chorrosion",
        version: APP_VERSION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
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
            Arc::new(SqliteMetadataProfileRepository::new(pool.clone())),
            Arc::new(SqliteIndexerDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool)),
        )
    }

    #[tokio::test]
    async fn get_system_status_returns_ok() {
        let state = make_test_state().await;
        let Json(resp) = get_system_status(State(state)).await;
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.api_base, API_V1_BASE);
    }

    #[tokio::test]
    async fn get_system_version_returns_name_and_version() {
        let state = make_test_state().await;
        let Json(resp) = get_system_version(State(state)).await;
        assert_eq!(resp.name, "chorrosion");
        assert_eq!(resp.version, APP_VERSION);
        assert!(!resp.version.is_empty());
    }
}
