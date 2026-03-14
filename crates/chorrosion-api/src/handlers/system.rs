// SPDX-License-Identifier: GPL-3.0-or-later
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{
    extract::{Query, State},
    Json,
};
use chorrosion_application::{
    AppState, NotificationEvent, NotificationPipeline, NotificationProviderKind,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};
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

pub(crate) async fn system_tasks_snapshot(state: &AppState) -> SystemTasksResponse {
    // NOTE: These job definitions mirror the registrations in `Scheduler::register_jobs`
    // (crates/chorrosion-scheduler/src/lib.rs). If a job is added, renamed, or its interval
    // changes there, this list must be updated to stay in sync.
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

    // Mirrors the seed-filtering logic in `DiscogsMetadataRefreshJob::from_config`:
    // enabled only when at least one non-empty (trimmed) seed artist or seed album exists.
    let discogs_config = &state.config.metadata.discogs;
    let has_discogs_seeds = discogs_config
        .seed_artists
        .iter()
        .any(|artist| !artist.trim().is_empty())
        || discogs_config
            .seed_albums
            .iter()
            .any(|album| !album.artist.trim().is_empty() && !album.album.trim().is_empty());
    items.push(SystemTaskResponse {
        id: "discogs-metadata-refresh".to_string(),
        name: "Discogs Metadata Refresh".to_string(),
        schedule_seconds: 6 * 60 * 60 + 30 * 60,
        enabled: has_discogs_seeds,
        status: if has_discogs_seeds {
            "scheduled".to_string()
        } else {
            "disabled".to_string()
        },
    });

    SystemTasksResponse {
        total: items.len() as i64,
        items,
        max_concurrent_jobs: state.config.scheduler.max_concurrent_jobs,
    }
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

/// Stable serialized representation of a notification provider kind for the API.
/// Uses `serde(rename_all = "snake_case")` to guarantee a stable contract
/// independent of Rust's `Debug` formatting.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationProviderKindApi {
    Email,
    Discord,
    Slack,
    Pushover,
    Script,
}

impl TryFrom<NotificationProviderKind> for NotificationProviderKindApi {
    type Error = ();

    fn try_from(kind: NotificationProviderKind) -> Result<Self, Self::Error> {
        match kind {
            NotificationProviderKind::Email => Ok(Self::Email),
            NotificationProviderKind::Discord => Ok(Self::Discord),
            NotificationProviderKind::Slack => Ok(Self::Slack),
            NotificationProviderKind::Pushover => Ok(Self::Pushover),
            NotificationProviderKind::Script => Ok(Self::Script),
            // Noop is filtered out by provider_configs() and never reaches the API layer
            NotificationProviderKind::Noop => Err(()),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationProviderStatusResponse {
    pub kind: NotificationProviderKindApi,
    pub enabled: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationStatusResponse {
    pub framework: String,
    pub providers: Vec<NotificationProviderStatusResponse>,
    pub enabled_provider_count: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NotificationTestResponse {
    pub status: String,
    pub dispatched: usize,
    pub message: String,
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

    Json(system_tasks_snapshot(&state).await)
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

#[utoipa::path(
    get,
    path = "/api/v1/system/notifications",
    responses(
        (status = 200, description = "Notification framework status", body = NotificationStatusResponse)
    ),
    tag = "system"
)]
pub async fn get_system_notifications(
    State(state): State<AppState>,
) -> Json<NotificationStatusResponse> {
    debug!(target: "api", "fetching notification framework status");

    let pipeline = NotificationPipeline::from_config(&state.config);
    let providers = pipeline
        .provider_configs()
        .into_iter()
        .map(|p| {
            let kind = NotificationProviderKindApi::try_from(p.kind)
                .expect("BUG: Noop provider should have been filtered by provider_configs()");
            NotificationProviderStatusResponse {
                kind,
                enabled: p.enabled,
            }
        })
        .collect::<Vec<_>>();

    Json(NotificationStatusResponse {
        framework: "baseline".to_string(),
        enabled_provider_count: providers.iter().filter(|p| p.enabled).count(),
        providers,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/system/notifications/test",
    responses(
        (status = 202, description = "Test notification dispatched", body = NotificationTestResponse),
        (status = 500, description = "Dispatch failed", body = NotificationTestResponse)
    ),
    tag = "system"
)]
pub async fn post_system_notifications_test(State(state): State<AppState>) -> impl IntoResponse {
    debug!(target: "api", "dispatching notification test event");

    let pipeline = NotificationPipeline::from_config(&state.config);
    let event = NotificationEvent::test();
    match pipeline.dispatch(&event).await {
        Ok(dispatched) => (
            StatusCode::ACCEPTED,
            Json(NotificationTestResponse {
                status: "accepted".to_string(),
                dispatched,
                message: "notification test event dispatched".to_string(),
            }),
        )
            .into_response(),
        Err(err) => {
            error!(target: "api", %err, "notification test dispatch failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(NotificationTestResponse {
                    status: "error".to_string(),
                    dispatched: 0,
                    message: format!("dispatch failed: {err}"),
                }),
            )
                .into_response()
        }
    }
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
        make_test_state_with_config(AppConfig::default()).await
    }

    async fn make_test_state_with_config(config: AppConfig) -> AppState {
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
            config,
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

        // With default AppConfig, Last.fm and Discogs integrations are unconfigured,
        // so their tasks should still be listed but marked as disabled.
        let lastfm = resp
            .items
            .iter()
            .find(|item| item.id == "lastfm-metadata-refresh")
            .expect("lastfm-metadata-refresh task should be present");
        assert!(
            !lastfm.enabled,
            "Last.fm task should be disabled by default"
        );
        assert_eq!(lastfm.status, "disabled");

        let discogs = resp
            .items
            .iter()
            .find(|item| item.id == "discogs-metadata-refresh")
            .expect("discogs-metadata-refresh task should be present");
        assert!(
            !discogs.enabled,
            "Discogs task should be disabled by default"
        );
        assert_eq!(discogs.status, "disabled");
    }

    #[tokio::test]
    async fn get_system_tasks_lastfm_enabled_when_api_key_configured() {
        use chorrosion_config::{LastFmConfig, MetadataConfig};
        let config = AppConfig {
            metadata: MetadataConfig {
                lastfm: LastFmConfig {
                    api_key: Some("test-api-key".to_string()),
                    ..LastFmConfig::default()
                },
                ..MetadataConfig::default()
            },
            ..AppConfig::default()
        };
        let state = make_test_state_with_config(config).await;
        let Json(resp) = get_system_tasks(State(state)).await;
        let lastfm = resp
            .items
            .iter()
            .find(|item| item.id == "lastfm-metadata-refresh")
            .expect("lastfm-metadata-refresh task should be present");
        assert!(
            lastfm.enabled,
            "Last.fm task should be enabled when api_key is set"
        );
        assert_eq!(lastfm.status, "scheduled");
    }

    #[tokio::test]
    async fn get_system_tasks_discogs_enabled_when_seeds_configured() {
        use chorrosion_config::{DiscogsConfig, MetadataConfig};
        let config = AppConfig {
            metadata: MetadataConfig {
                discogs: DiscogsConfig {
                    seed_artists: vec!["Massive Attack".to_string()],
                    ..DiscogsConfig::default()
                },
                ..MetadataConfig::default()
            },
            ..AppConfig::default()
        };
        let state = make_test_state_with_config(config).await;
        let Json(resp) = get_system_tasks(State(state)).await;
        let discogs = resp
            .items
            .iter()
            .find(|item| item.id == "discogs-metadata-refresh")
            .expect("discogs-metadata-refresh task should be present");
        assert!(
            discogs.enabled,
            "Discogs task should be enabled when seed_artists is set"
        );
        assert_eq!(discogs.status, "scheduled");
    }

    #[tokio::test]
    async fn get_system_tasks_discogs_disabled_when_only_whitespace_seeds() {
        use chorrosion_config::{DiscogsConfig, MetadataConfig};
        let config = AppConfig {
            metadata: MetadataConfig {
                discogs: DiscogsConfig {
                    // Whitespace-only seeds should not enable the job (matches scheduler logic)
                    seed_artists: vec!["   ".to_string()],
                    ..DiscogsConfig::default()
                },
                ..MetadataConfig::default()
            },
            ..AppConfig::default()
        };
        let state = make_test_state_with_config(config).await;
        let Json(resp) = get_system_tasks(State(state)).await;
        let discogs = resp
            .items
            .iter()
            .find(|item| item.id == "discogs-metadata-refresh")
            .expect("discogs-metadata-refresh task should be present");
        assert!(
            !discogs.enabled,
            "Discogs task should be disabled when seeds are whitespace-only"
        );
        assert_eq!(discogs.status, "disabled");
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

    #[tokio::test]
    async fn get_system_notifications_returns_framework_status() {
        let state = make_test_state().await;
        let Json(resp) = get_system_notifications(State(state)).await;
        assert_eq!(resp.framework, "baseline");
        // Default pipeline includes email + discord + slack providers, all disabled unless configured.
        // enabled_provider_count reflects only enabled providers.
        assert_eq!(resp.enabled_provider_count, 0);
        assert_eq!(resp.providers.len(), 3);
        assert!(matches!(
            resp.providers[0].kind,
            NotificationProviderKindApi::Email
        ));
        assert!(!resp.providers[0].enabled);
        assert!(matches!(
            resp.providers[1].kind,
            NotificationProviderKindApi::Discord
        ));
        assert!(!resp.providers[1].enabled);
        assert!(matches!(
            resp.providers[2].kind,
            NotificationProviderKindApi::Slack
        ));
        assert!(!resp.providers[2].enabled);
    }

    #[tokio::test]
    async fn post_system_notifications_test_returns_accepted() {
        let state = make_test_state().await;
        let response = post_system_notifications_test(State(state))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: NotificationTestResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.status, "accepted");
        // Default email provider is disabled without sender/recipient config.
        assert_eq!(resp.dispatched, 0);
    }

    #[tokio::test]
    async fn post_system_notifications_test_dispatches_when_email_configured() {
        use chorrosion_config::{AppConfig, EmailNotificationConfig, NotificationsConfig};

        let state = make_test_state_with_config(AppConfig {
            notifications: NotificationsConfig {
                email: EmailNotificationConfig {
                    enabled: true,
                    from: Some("noreply@example.com".to_string()),
                    to: vec!["ops@example.com".to_string()],
                },
                ..Default::default()
            },
            ..AppConfig::default()
        })
        .await;

        let response = post_system_notifications_test(State(state))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: NotificationTestResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.status, "accepted");
        assert_eq!(resp.dispatched, 1);
    }

    #[tokio::test]
    async fn get_system_notifications_marks_discord_enabled_when_configured() {
        use chorrosion_config::{AppConfig, DiscordNotificationConfig, NotificationsConfig};

        let state = make_test_state_with_config(AppConfig {
            notifications: NotificationsConfig {
                discord: DiscordNotificationConfig {
                    enabled: true,
                    webhook_url: Some("https://discord.example/webhook".to_string()),
                    username: Some("Chorrosion".to_string()),
                },
                ..Default::default()
            },
            ..AppConfig::default()
        })
        .await;

        let Json(resp) = get_system_notifications(State(state)).await;
        assert_eq!(resp.enabled_provider_count, 1);
        let discord = resp
            .providers
            .iter()
            .find(|p| matches!(p.kind, NotificationProviderKindApi::Discord))
            .expect("discord provider should be present");
        assert!(discord.enabled);
    }

    #[tokio::test]
    async fn get_system_notifications_marks_slack_enabled_when_configured() {
        use chorrosion_config::{AppConfig, NotificationsConfig, SlackNotificationConfig};

        let state = make_test_state_with_config(AppConfig {
            notifications: NotificationsConfig {
                slack: SlackNotificationConfig {
                    enabled: true,
                    webhook_url: Some("https://hooks.slack.com/services/test".to_string()),
                    username: Some("Chorrosion".to_string()),
                },
                ..Default::default()
            },
            ..AppConfig::default()
        })
        .await;

        let Json(resp) = get_system_notifications(State(state)).await;
        assert_eq!(resp.enabled_provider_count, 1);
        let slack = resp
            .providers
            .iter()
            .find(|p| matches!(p.kind, NotificationProviderKindApi::Slack))
            .expect("slack provider should be present");
        assert!(slack.enabled);
    }
}
