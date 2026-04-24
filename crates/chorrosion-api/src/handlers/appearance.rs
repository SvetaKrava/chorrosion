// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{extract::State, http::StatusCode, Json};
use chorrosion_application::{AppState, ThemeMode};
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
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAppearanceSettingsRequest {
    pub theme_mode: String,
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
    let settings = state.appearance_settings();
    Json(AppearanceSettingsResponse {
        theme_mode: settings.theme_mode.into(),
    })
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/appearance",
    request_body = UpdateAppearanceSettingsRequest,
    responses(
        (status = 200, description = "Updated appearance settings", body = AppearanceSettingsResponse),
        (status = 400, description = "Invalid theme mode", body = AppearanceErrorResponse)
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

    let updated = state.set_theme_mode(theme_mode);
    Ok(Json(AppearanceSettingsResponse {
        theme_mode: updated.theme_mode.into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }

    #[tokio::test]
    async fn update_appearance_settings_updates_theme_mode() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state.clone()),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "dark".to_string(),
            }),
        )
        .await
        .expect("valid update");

        assert!(matches!(result.0.theme_mode, ThemeModeApi::Dark));

        let Json(check) = get_appearance_settings(State(state)).await;
        assert!(matches!(check.theme_mode, ThemeModeApi::Dark));
    }

    #[tokio::test]
    async fn update_appearance_settings_rejects_invalid_mode() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "midnight".to_string(),
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid mode should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid theme mode"));
    }
}
