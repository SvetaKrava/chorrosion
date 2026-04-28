// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{extract::State, http::StatusCode, Json};
use chorrosion_application::{
    AppState, AppearanceSettings, FilterOperator, ShortcutProfile, ThemeMode,
    DEFAULT_BULK_SELECTION_LIMIT, DEFAULT_FILTER_HISTORY_LIMIT, DEFAULT_FILTER_OPERATOR,
    DEFAULT_MAX_FILTER_CLAUSES, DEFAULT_SHORTCUT_PROFILE,
};
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

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperatorApi {
    And,
    Or,
}

impl From<FilterOperator> for FilterOperatorApi {
    fn from(value: FilterOperator) -> Self {
        match value {
            FilterOperator::And => Self::And,
            FilterOperator::Or => Self::Or,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutProfileApi {
    Standard,
    Vim,
    Emacs,
}

impl From<ShortcutProfile> for ShortcutProfileApi {
    fn from(value: ShortcutProfile) -> Self {
        match value {
            ShortcutProfile::Standard => Self::Standard,
            ShortcutProfile::Vim => Self::Vim,
            ShortcutProfile::Emacs => Self::Emacs,
        }
    }
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
    pub keyboard_shortcuts_enabled: bool,
    pub shortcut_profile: ShortcutProfileApi,
    pub bulk_operations_enabled: bool,
    pub bulk_selection_limit: u16,
    pub bulk_action_confirmation: bool,
    pub advanced_filtering_enabled: bool,
    pub default_filter_operator: FilterOperatorApi,
    pub max_filter_clauses: u8,
    pub filter_history_enabled: bool,
    pub filter_history_limit: u8,
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
    #[serde(default = "UpdateAppearanceSettingsRequest::default_keyboard_shortcuts_enabled")]
    pub keyboard_shortcuts_enabled: bool,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_shortcut_profile")]
    #[schema(value_type = ShortcutProfileApi)]
    pub shortcut_profile: String,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_bulk_operations_enabled")]
    pub bulk_operations_enabled: bool,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_bulk_selection_limit")]
    pub bulk_selection_limit: u16,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_bulk_action_confirmation")]
    pub bulk_action_confirmation: bool,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_advanced_filtering_enabled")]
    pub advanced_filtering_enabled: bool,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_filter_operator")]
    #[schema(value_type = FilterOperatorApi)]
    pub default_filter_operator: String,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_max_filter_clauses")]
    pub max_filter_clauses: u8,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_filter_history_enabled")]
    pub filter_history_enabled: bool,
    #[serde(default = "UpdateAppearanceSettingsRequest::default_filter_history_limit")]
    pub filter_history_limit: u8,
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

    fn default_keyboard_shortcuts_enabled() -> bool {
        AppearanceSettings::default().keyboard_shortcuts_enabled
    }

    fn default_shortcut_profile() -> String {
        DEFAULT_SHORTCUT_PROFILE.as_str().to_string()
    }

    fn default_bulk_operations_enabled() -> bool {
        AppearanceSettings::default().bulk_operations_enabled
    }

    fn default_bulk_selection_limit() -> u16 {
        DEFAULT_BULK_SELECTION_LIMIT
    }

    fn default_bulk_action_confirmation() -> bool {
        AppearanceSettings::default().bulk_action_confirmation
    }

    fn default_advanced_filtering_enabled() -> bool {
        AppearanceSettings::default().advanced_filtering_enabled
    }

    fn default_filter_operator() -> String {
        DEFAULT_FILTER_OPERATOR.as_str().to_string()
    }

    fn default_max_filter_clauses() -> u8 {
        DEFAULT_MAX_FILTER_CLAUSES
    }

    fn default_filter_history_enabled() -> bool {
        AppearanceSettings::default().filter_history_enabled
    }

    fn default_filter_history_limit() -> u8 {
        DEFAULT_FILTER_HISTORY_LIMIT
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
        keyboard_shortcuts_enabled: settings.keyboard_shortcuts_enabled,
        shortcut_profile: settings.shortcut_profile.into(),
        bulk_operations_enabled: settings.bulk_operations_enabled,
        bulk_selection_limit: settings.bulk_selection_limit,
        bulk_action_confirmation: settings.bulk_action_confirmation,
        advanced_filtering_enabled: settings.advanced_filtering_enabled,
        default_filter_operator: settings.default_filter_operator.into(),
        max_filter_clauses: settings.max_filter_clauses,
        filter_history_enabled: settings.filter_history_enabled,
        filter_history_limit: settings.filter_history_limit,
    })
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/appearance",
    request_body = UpdateAppearanceSettingsRequest,
    responses(
        (status = 200, description = "Updated appearance settings", body = AppearanceSettingsResponse),
        (status = 400, description = "Invalid theme mode, mobile breakpoint, shortcut profile, or bulk selection limit", body = AppearanceErrorResponse)
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

    let shortcut_profile = ShortcutProfile::from_str(&request.shortcut_profile).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            Json(AppearanceErrorResponse {
                error: err.to_string(),
            }),
        )
    })?;

    let default_filter_operator = FilterOperator::from_str(&request.default_filter_operator)
        .map_err(|err| {
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
            keyboard_shortcuts_enabled: request.keyboard_shortcuts_enabled,
            shortcut_profile,
            bulk_operations_enabled: request.bulk_operations_enabled,
            bulk_selection_limit: request.bulk_selection_limit,
            bulk_action_confirmation: request.bulk_action_confirmation,
            advanced_filtering_enabled: request.advanced_filtering_enabled,
            default_filter_operator,
            max_filter_clauses: request.max_filter_clauses,
            filter_history_enabled: request.filter_history_enabled,
            filter_history_limit: request.filter_history_limit,
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
        keyboard_shortcuts_enabled: updated.keyboard_shortcuts_enabled,
        shortcut_profile: updated.shortcut_profile.into(),
        bulk_operations_enabled: updated.bulk_operations_enabled,
        bulk_selection_limit: updated.bulk_selection_limit,
        bulk_action_confirmation: updated.bulk_action_confirmation,
        advanced_filtering_enabled: updated.advanced_filtering_enabled,
        default_filter_operator: updated.default_filter_operator.into(),
        max_filter_clauses: updated.max_filter_clauses,
        filter_history_enabled: updated.filter_history_enabled,
        filter_history_limit: updated.filter_history_limit,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chorrosion_application::{DEFAULT_MAX_FILTER_CLAUSES, DEFAULT_MOBILE_BREAKPOINT_PX};
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
        assert!(response.keyboard_shortcuts_enabled);
        assert!(matches!(
            response.shortcut_profile,
            ShortcutProfileApi::Standard
        ));
        assert!(response.bulk_operations_enabled);
        assert_eq!(response.bulk_selection_limit, DEFAULT_BULK_SELECTION_LIMIT);
        assert!(response.bulk_action_confirmation);
        assert!(response.advanced_filtering_enabled);
        assert!(matches!(
            response.default_filter_operator,
            FilterOperatorApi::And
        ));
        assert_eq!(response.max_filter_clauses, DEFAULT_MAX_FILTER_CLAUSES);
        assert!(response.filter_history_enabled);
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
                keyboard_shortcuts_enabled: false,
                shortcut_profile: "vim".to_string(),
                bulk_operations_enabled: true,
                bulk_selection_limit: 250,
                bulk_action_confirmation: false,
                advanced_filtering_enabled: false,
                default_filter_operator: "or".to_string(),
                max_filter_clauses: 15,
                filter_history_enabled: false,
                filter_history_limit: 30,
            }),
        )
        .await
        .expect("valid update");

        assert!(matches!(result.0.theme_mode, ThemeModeApi::Dark));
        assert_eq!(result.0.mobile_breakpoint_px, 640);
        assert!(!result.0.mobile_compact_layout);
        assert!(result.0.touch_targets_optimized);
        assert!(!result.0.keyboard_shortcuts_enabled);
        assert!(matches!(result.0.shortcut_profile, ShortcutProfileApi::Vim));
        assert!(result.0.bulk_operations_enabled);
        assert_eq!(result.0.bulk_selection_limit, 250);
        assert!(!result.0.bulk_action_confirmation);
        assert!(!result.0.advanced_filtering_enabled);
        assert!(matches!(
            result.0.default_filter_operator,
            FilterOperatorApi::Or
        ));
        assert_eq!(result.0.max_filter_clauses, 15);
        assert!(!result.0.filter_history_enabled);
        assert_eq!(result.0.filter_history_limit, 30);

        let Json(check) = get_appearance_settings(State(state)).await;
        assert!(matches!(check.theme_mode, ThemeModeApi::Dark));
        assert_eq!(check.mobile_breakpoint_px, 640);
        assert!(!check.mobile_compact_layout);
        assert!(!check.keyboard_shortcuts_enabled);
        assert!(matches!(check.shortcut_profile, ShortcutProfileApi::Vim));
        assert!(check.bulk_operations_enabled);
        assert_eq!(check.bulk_selection_limit, 250);
        assert!(!check.bulk_action_confirmation);
        assert!(!check.advanced_filtering_enabled);
        assert!(matches!(
            check.default_filter_operator,
            FilterOperatorApi::Or
        ));
        assert_eq!(check.max_filter_clauses, 15);
        assert!(!check.filter_history_enabled);
        assert_eq!(check.filter_history_limit, 30);
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
                keyboard_shortcuts_enabled: true,
                shortcut_profile: "standard".to_string(),
                bulk_operations_enabled: true,
                bulk_selection_limit: 100,
                bulk_action_confirmation: true,
                advanced_filtering_enabled: true,
                default_filter_operator: "and".to_string(),
                max_filter_clauses: 10,
                filter_history_enabled: true,
                filter_history_limit: 20,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid breakpoint should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid mobile breakpoint"));
    }

    #[tokio::test]
    async fn update_appearance_settings_rejects_invalid_shortcut_profile() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "dark".to_string(),
                mobile_breakpoint_px: 768,
                mobile_compact_layout: true,
                touch_targets_optimized: true,
                keyboard_shortcuts_enabled: true,
                shortcut_profile: "gaming".to_string(),
                bulk_operations_enabled: true,
                bulk_selection_limit: 100,
                bulk_action_confirmation: true,
                advanced_filtering_enabled: true,
                default_filter_operator: "and".to_string(),
                max_filter_clauses: 10,
                filter_history_enabled: true,
                filter_history_limit: 20,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid profile should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid shortcut profile"));
    }

    #[tokio::test]
    async fn update_appearance_settings_rejects_invalid_bulk_selection_limit() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "dark".to_string(),
                mobile_breakpoint_px: 768,
                mobile_compact_layout: true,
                touch_targets_optimized: true,
                keyboard_shortcuts_enabled: true,
                shortcut_profile: "standard".to_string(),
                bulk_operations_enabled: true,
                bulk_selection_limit: 5,
                bulk_action_confirmation: true,
                advanced_filtering_enabled: true,
                default_filter_operator: "and".to_string(),
                max_filter_clauses: 10,
                filter_history_enabled: true,
                filter_history_limit: 20,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid bulk limit should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid bulk selection limit"));
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
                keyboard_shortcuts_enabled: true,
                shortcut_profile: "standard".to_string(),
                bulk_operations_enabled: true,
                bulk_selection_limit: 100,
                bulk_action_confirmation: true,
                advanced_filtering_enabled: true,
                default_filter_operator: "and".to_string(),
                max_filter_clauses: 10,
                filter_history_enabled: true,
                filter_history_limit: 20,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid mode should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid theme mode"));
    }

    #[tokio::test]
    async fn update_appearance_settings_rejects_invalid_filter_operator() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "dark".to_string(),
                mobile_breakpoint_px: 768,
                mobile_compact_layout: true,
                touch_targets_optimized: true,
                keyboard_shortcuts_enabled: true,
                shortcut_profile: "standard".to_string(),
                bulk_operations_enabled: true,
                bulk_selection_limit: 100,
                bulk_action_confirmation: true,
                advanced_filtering_enabled: true,
                default_filter_operator: "xor".to_string(),
                max_filter_clauses: 10,
                filter_history_enabled: true,
                filter_history_limit: 20,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid filter operator should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid filter operator"));
    }

    #[tokio::test]
    async fn update_appearance_settings_rejects_invalid_max_filter_clauses() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "dark".to_string(),
                mobile_breakpoint_px: 768,
                mobile_compact_layout: true,
                touch_targets_optimized: true,
                keyboard_shortcuts_enabled: true,
                shortcut_profile: "standard".to_string(),
                bulk_operations_enabled: true,
                bulk_selection_limit: 100,
                bulk_action_confirmation: true,
                advanced_filtering_enabled: true,
                default_filter_operator: "and".to_string(),
                max_filter_clauses: 1,
                filter_history_enabled: true,
                filter_history_limit: 20,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid max filter clauses should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid max filter clauses"));
    }

    #[tokio::test]
    async fn update_appearance_settings_rejects_invalid_filter_history_limit() {
        let state = make_test_state().await;

        let result = update_appearance_settings(
            State(state),
            Json(UpdateAppearanceSettingsRequest {
                theme_mode: "dark".to_string(),
                mobile_breakpoint_px: 768,
                mobile_compact_layout: true,
                touch_targets_optimized: true,
                keyboard_shortcuts_enabled: true,
                shortcut_profile: "standard".to_string(),
                bulk_operations_enabled: true,
                bulk_selection_limit: 100,
                bulk_action_confirmation: true,
                advanced_filtering_enabled: true,
                default_filter_operator: "and".to_string(),
                max_filter_clauses: 10,
                filter_history_enabled: true,
                filter_history_limit: 0,
            }),
        )
        .await;

        let (status, Json(error)) = result.expect_err("invalid filter history limit should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("invalid filter history limit"));
    }
}
