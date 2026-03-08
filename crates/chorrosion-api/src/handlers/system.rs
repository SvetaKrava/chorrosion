// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Query, State},
    Json,
};
use chorrosion_application::AppState;
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::{IntoParams, ToSchema};

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

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemTaskResponse {
    pub id: String,
    pub name: String,
    pub schedule_seconds: u64,
    pub enabled: bool,
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemTasksResponse {
    pub items: Vec<SystemTaskResponse>,
    pub total: i64,
    pub max_concurrent_jobs: usize,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListSystemLogsQuery {
    #[serde(default = "default_log_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_log_limit() -> i64 {
    100
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemLogEntryResponse {
    pub id: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemLogsResponse {
    pub items: Vec<SystemLogEntryResponse>,
    pub total: i64,
    pub source: String,
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

#[utoipa::path(
    get,
    path = "/api/v1/system/tasks",
    responses(
        (status = 200, description = "Registered system tasks", body = SystemTasksResponse)
    ),
    tag = "system"
)]
pub async fn get_system_tasks(State(state): State<AppState>) -> Json<SystemTasksResponse> {
    debug!(target: "api", "fetching system task metadata");

    let mut items = vec![
        SystemTaskResponse {
            id: "rss-sync".to_string(),
            name: "RSS Sync".to_string(),
            schedule_seconds: 15 * 60,
            enabled: true,
            status: "scheduled".to_string(),
        },
        SystemTaskResponse {
            id: "backlog-search".to_string(),
            name: "Backlog Search".to_string(),
            schedule_seconds: 60 * 60,
            enabled: true,
            status: "scheduled".to_string(),
        },
        SystemTaskResponse {
            id: "refresh-artists".to_string(),
            name: "Refresh Artists".to_string(),
            schedule_seconds: 12 * 60 * 60,
            enabled: true,
            status: "scheduled".to_string(),
        },
        SystemTaskResponse {
            id: "refresh-albums".to_string(),
            name: "Refresh Albums".to_string(),
            schedule_seconds: 12 * 60 * 60 + 15 * 60,
            enabled: true,
            status: "scheduled".to_string(),
        },
        SystemTaskResponse {
            id: "housekeeping".to_string(),
            name: "Housekeeping".to_string(),
            schedule_seconds: 24 * 60 * 60,
            enabled: true,
            status: "scheduled".to_string(),
        },
    ];

    let has_lastfm_key = state
        .config
        .metadata
        .lastfm
        .api_key
        .as_ref()
        .is_some_and(|key| !key.trim().is_empty());
    items.push(SystemTaskResponse {
        id: "lastfm-metadata-refresh".to_string(),
        name: "Last.fm Metadata Refresh".to_string(),
        schedule_seconds: 6 * 60 * 60,
        enabled: has_lastfm_key,
        status: if has_lastfm_key {
            "scheduled".to_string()
        } else {
            "disabled".to_string()
        },
    });

    let has_discogs_tokens = state
        .config
        .metadata
        .discogs
        .token
        .as_ref()
        .is_some_and(|token| !token.trim().is_empty())
        || !state.config.metadata.discogs.seed_artists.is_empty()
        || !state.config.metadata.discogs.seed_albums.is_empty();
    items.push(SystemTaskResponse {
        id: "discogs-metadata-refresh".to_string(),
        name: "Discogs Metadata Refresh".to_string(),
        schedule_seconds: 6 * 60 * 60 + 30 * 60,
        enabled: has_discogs_tokens,
        status: if has_discogs_tokens {
            "scheduled".to_string()
        } else {
            "disabled".to_string()
        },
    });

    Json(SystemTasksResponse {
        total: items.len() as i64,
        items,
        max_concurrent_jobs: state.config.scheduler.max_concurrent_jobs,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/system/logs",
    params(ListSystemLogsQuery),
    responses(
        (status = 200, description = "System log entries", body = SystemLogsResponse)
    ),
    tag = "system"
)]
pub async fn get_system_logs(
    State(_state): State<AppState>,
    Query(_query): Query<ListSystemLogsQuery>,
) -> Json<SystemLogsResponse> {
    debug!(target: "api", "fetching system log entries");

    // Placeholder until runtime log sink and persisted job log querying are wired.
    Json(SystemLogsResponse {
        items: vec![],
        total: 0,
        source: "job_logs".to_string(),
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

    #[tokio::test]
    async fn get_system_tasks_includes_core_jobs() {
        let state = make_test_state().await;
        let Json(resp) = get_system_tasks(State(state)).await;
        assert!(resp.total >= 5);
        assert!(resp.items.iter().any(|item| item.id == "rss-sync"));
        assert!(resp.items.iter().any(|item| item.id == "housekeeping"));
    }

    #[tokio::test]
    async fn get_system_logs_returns_empty_placeholder_list() {
        let state = make_test_state().await;
        let Json(resp) = get_system_logs(
            State(state),
            Query(ListSystemLogsQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await;
        assert_eq!(resp.total, 0);
        assert!(resp.items.is_empty());
        assert_eq!(resp.source, "job_logs");
    }
}
