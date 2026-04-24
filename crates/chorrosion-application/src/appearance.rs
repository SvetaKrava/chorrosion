// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

impl FromStr for ThemeMode {
    type Err = AppearanceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();

        if trimmed.eq_ignore_ascii_case("system") {
            Ok(Self::System)
        } else if trimmed.eq_ignore_ascii_case("dark") {
            Ok(Self::Dark)
        } else if trimmed.eq_ignore_ascii_case("light") {
            Ok(Self::Light)
        } else {
            Err(AppearanceError::InvalidThemeMode(value.to_string()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub theme_mode: ThemeMode,
}

impl AppearanceSettings {
    pub fn new(theme_mode: ThemeMode) -> Self {
        Self { theme_mode }
    }
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
        }
    }
}

#[derive(Debug, Error)]
pub enum AppearanceError {
    #[error("invalid theme mode: {0}")]
    InvalidThemeMode(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_mode_is_system() {
        assert_eq!(ThemeMode::default(), ThemeMode::System);
        assert_eq!(AppearanceSettings::default().theme_mode, ThemeMode::System);
    }

    #[test]
    fn theme_mode_parse_accepts_valid_values() {
        assert_eq!(
            ThemeMode::from_str("system").expect("system"),
            ThemeMode::System
        );
        assert_eq!(ThemeMode::from_str("dark").expect("dark"), ThemeMode::Dark);
        assert_eq!(
            ThemeMode::from_str("light").expect("light"),
            ThemeMode::Light
        );
        assert_eq!(
            ThemeMode::from_str(" DARK ").expect("trimmed"),
            ThemeMode::Dark
        );
    }

    #[test]
    fn theme_mode_parse_rejects_invalid_values() {
        let err = ThemeMode::from_str("midnight").expect_err("invalid should fail");
        assert!(err.to_string().contains("invalid theme mode"));
    }

    #[test]
    fn theme_mode_serde_roundtrip() {
        let json = serde_json::to_string(&ThemeMode::Dark).expect("serialize");
        assert_eq!(json, "\"dark\"");
        let mode: ThemeMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, ThemeMode::Dark);
    }

    #[test]
    fn appearance_settings_new_sets_theme_mode() {
        let settings = AppearanceSettings::new(ThemeMode::Light);
        assert_eq!(settings.theme_mode, ThemeMode::Light);
    }
}
