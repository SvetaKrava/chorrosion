// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{extract::State, http::StatusCode, Json};
use chorrosion_application::{AppState, AppearanceSettings, ThemeMode};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThemeModeApi {
    System,
    Dark,
    Light,
}

impl From<ThemeMode> for ThemeModeApi {
    fn from(value: ThemeMode) -> Self {
        match value {
            ThemeMode::System => Self::System,
            ThemeMode::Dark => Self::Dark,
            ThemeMode::Light => Self::Light,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppearanceSettingsResponse {
    pub theme_mode: ThemeModeApi,
    pub mobile_breakpoint_px: u16,
    pub mobile_compact_layout: bool,
    pub touch_targets_optimized: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAppearanceSettingsRequest {
    #[schema(value_type = ThemeModeApi)]
    pub theme_mode: String,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_mobile_breakpoint_px")]
    pub mobile_breakpoint_px: u16,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_mobile_compact_layout")]
    pub mobile_compact_layout: bool,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_touch_targets_optimized")]
    pub touch_targets_optimized: bool,
}

impl UpdateAppearanceSettingsRequest {
    fn default_mobile_breakpoint_px() -> u16 {
        AppearanceSettings::default().mobile_breakpoint_px
    }

    fn default_mobile_compact_layout() -> bool {
        AppearanceSettings::default().mobile_compact_layout
    }

    fn default_touch_targets_optimized() -> bool {
        AppearanceSettings::default().touch_targets_optimized
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AppearanceErrorResponse {
    pub error: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/appearance",
    responses(
        (status = 200, description = "Current appearance settings", body = AppearanceSettingsResponse)
    ),
    tag = "settings"
)]
pub async fn get_appearance_settings(
    State(state): State<AppState>,
) -> Json<AppearanceSettingsResponse> {
    let settings = state.appearance_settings().await;
    Json(AppearanceSettingsResponse {
        theme_mode: settings.theme_mode.into(),
        mobile_breakpoint_px: settings.mobile_breakpoint_px,
        mobile_compact_layout: settings.mobile_compact_layout,
        touch_targets_optimized: settings.touch_targets_optimized,
    })
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/appearance",
    request_body = UpdateAppearanceSettingsRequest,
    responses(
        (status = 200, description = "Updated appearance settings", body = AppearanceSettingsResponse),
        (status = 400, description = "Invalid theme mode or mobile breakpoint", body = AppearanceErrorResponse)
    ),
    tag = "settings"
)]
pub async fn update_appearance_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateAppearanceSettingsRequest>,
) -> Result<Json<AppearanceSettingsResponse>, (StatusCode, Json<AppearanceErrorResponse>)> {
    let theme_mode = ThemeMode::from_str(&request.theme_mode).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            Json(AppearanceErrorResponse {
                error: err.to_string(),
            }),
        )
    })?;

    let updated = state
        .set_appearance_settings(AppearanceSettings {
            theme_mode,
            mobile_breakpoint_px: request.mobile_breakpoint_px,
            mobile_compact_layout: request.mobile_compact_layout,
            touch_targets_optimized: request.touch_targets_optimized,
        })
        .await
        .map_err(|err| {
            (
                StatusCode::BAD_REQUEST,
                Json(AppearanceErrorResponse {
                    error: err.to_string(),
                }),
            )
        })?;

    Ok(Json(AppearanceSettingsResponse {
        theme_mode: updated.theme_mode.into(),
        mobile_breakpoint_px: updated.mobile_breakpoint_px,
        mobile_compact_layout: updated.mobile_compact_layout,
        touch_targets_optimized: updated.touch_targets_optimized,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chorrosion_application::DEFAULT_MOBILE_BREAKPOINT_PX;
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTagRepository, SqliteTaggedEntityRepository,
        SqliteTrackRepository,
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
    async fn get_appearance_settings_returns_default_system_theme() {
        let state = make_test_state().await;

        let Json(response) = get_appearance_settings(State(state)).await;

        assert!(matches!(response.theme_mode, ThemeModeApi::System));
        assert_eq!(response.mobile_breakpoint_px, DEFAULT_MOBILE_BREAKPOINT_PX);
        assert!(response.mobile_compact_layout);
        assert!(response.touch_targets_optimized);
    }

    #[tokio::test]
    async fn update_appearance_settings_updates_theme_mode() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state.clone()),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "dark".to_string(),
                mobile_breakpoint_px: 640,
                mobile_compact_layout: false,
                touch_targets_optimized: true,
            }),
        )
        .await
        .expect("valid update");

        assert!(matches!(result.0.theme_mode, ThemeModeApi::Dark));
        assert_eq!(result.0.mobile_breakpoint_px, 640);
        assert!(!result.0.mobile_compact_layout);
        assert!(result.0.touch_targets_optimized);

        let Json(check) = get_appearance_settings(State(state)).await;
        assert!(matches!(check.theme_mode, ThemeModeApi::Dark));
        assert_eq!(check.mobile_breakpoint_px, 640);
        assert!(!check.mobile_compact_layout);
    }

    #[tokio::test]
    async fn update_appearance_settings_rejects_invalid_breakpoint() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "dark".to_string(),
                mobile_breakpoint_px: 200,
                mobile_compact_layout: true,
                touch_targets_optimized: true,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid breakpoint should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid mobile breakpoint"));
    }

    #[tokio::test]
    async fn update_appearance_settings_rejects_invalid_mode() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "midnight".to_string(),
                mobile_breakpoint_px: 768,
                mobile_compact_layout: true,
                touch_targets_optimized: true,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid mode should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid theme mode"));
    }
}
