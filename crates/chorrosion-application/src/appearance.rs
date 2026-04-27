// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

pub const DEFAULT_MOBILE_BREAKPOINT_PX: u16 = 768;
pub const MIN_MOBILE_BREAKPOINT_PX: u16 = 320;
pub const MAX_MOBILE_BREAKPOINT_PX: u16 = 1440;
pub const DEFAULT_SHORTCUT_PROFILE: ShortcutProfile = ShortcutProfile::Standard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutProfile {
    #[default]
    Standard,
    Vim,
    Emacs,
}

impl ShortcutProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Vim => "vim",
            Self::Emacs => "emacs",
        }
    }
}

impl FromStr for ShortcutProfile {
    type Err = AppearanceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();

        if trimmed.eq_ignore_ascii_case("standard") {
            Ok(Self::Standard)
        } else if trimmed.eq_ignore_ascii_case("vim") {
            Ok(Self::Vim)
        } else if trimmed.eq_ignore_ascii_case("emacs") {
            Ok(Self::Emacs)
        } else {
            Err(AppearanceError::InvalidShortcutProfile(value.to_string()))
        }
    }
}

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
    pub mobile_breakpoint_px: u16,
    pub mobile_compact_layout: bool,
    pub touch_targets_optimized: bool,
    pub keyboard_shortcuts_enabled: bool,
    pub shortcut_profile: ShortcutProfile,
}

impl AppearanceSettings {
    pub fn new(theme_mode: ThemeMode) -> Self {
        Self {
            theme_mode,
            mobile_breakpoint_px: DEFAULT_MOBILE_BREAKPOINT_PX,
            mobile_compact_layout: true,
            touch_targets_optimized: true,
            keyboard_shortcuts_enabled: true,
            shortcut_profile: DEFAULT_SHORTCUT_PROFILE,
        }
    }

    pub fn validate_mobile_breakpoint_px(value: u16) -> Result<(), AppearanceError> {
        if (MIN_MOBILE_BREAKPOINT_PX..=MAX_MOBILE_BREAKPOINT_PX).contains(&value) {
            Ok(())
        } else {
            Err(AppearanceError::InvalidMobileBreakpoint(value))
        }
    }
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self::new(ThemeMode::System)
    }
}

#[derive(Debug, Error)]
pub enum AppearanceError {
    #[error("invalid theme mode: {0}")]
    InvalidThemeMode(String),
    #[error(
        "invalid mobile breakpoint: {0}. expected {MIN_MOBILE_BREAKPOINT_PX}..={MAX_MOBILE_BREAKPOINT_PX} px"
    )]
    InvalidMobileBreakpoint(u16),
    #[error("invalid shortcut profile: {0}")]
    InvalidShortcutProfile(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_mode_is_system() {
        assert_eq!(ThemeMode::default(), ThemeMode::System);
        assert_eq!(AppearanceSettings::default().theme_mode, ThemeMode::System);
        assert_eq!(
            AppearanceSettings::default().mobile_breakpoint_px,
            DEFAULT_MOBILE_BREAKPOINT_PX
        );
        assert!(AppearanceSettings::default().mobile_compact_layout);
        assert!(AppearanceSettings::default().touch_targets_optimized);
        assert!(AppearanceSettings::default().keyboard_shortcuts_enabled);
        assert_eq!(
            AppearanceSettings::default().shortcut_profile,
            ShortcutProfile::Standard
        );
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
    fn shortcut_profile_parse_accepts_valid_values() {
        assert_eq!(
            ShortcutProfile::from_str("standard").expect("standard"),
            ShortcutProfile::Standard
        );
        assert_eq!(
            ShortcutProfile::from_str("vim").expect("vim"),
            ShortcutProfile::Vim
        );
        assert_eq!(
            ShortcutProfile::from_str("EMACS").expect("emacs"),
            ShortcutProfile::Emacs
        );
    }

    #[test]
    fn shortcut_profile_parse_rejects_invalid_values() {
        let err = ShortcutProfile::from_str("gaming").expect_err("invalid should fail");
        assert!(err.to_string().contains("invalid shortcut profile"));
    }

    #[test]
    fn appearance_settings_new_sets_theme_mode() {
        let settings = AppearanceSettings::new(ThemeMode::Light);
        assert_eq!(settings.theme_mode, ThemeMode::Light);
        assert_eq!(settings.mobile_breakpoint_px, DEFAULT_MOBILE_BREAKPOINT_PX);
        assert!(settings.keyboard_shortcuts_enabled);
        assert_eq!(settings.shortcut_profile, ShortcutProfile::Standard);
    }

    #[test]
    fn validate_mobile_breakpoint_accepts_values_in_range() {
        AppearanceSettings::validate_mobile_breakpoint_px(MIN_MOBILE_BREAKPOINT_PX)
            .expect("min breakpoint should be valid");
        AppearanceSettings::validate_mobile_breakpoint_px(DEFAULT_MOBILE_BREAKPOINT_PX)
            .expect("default breakpoint should be valid");
        AppearanceSettings::validate_mobile_breakpoint_px(MAX_MOBILE_BREAKPOINT_PX)
            .expect("max breakpoint should be valid");
    }

    #[test]
    fn validate_mobile_breakpoint_rejects_out_of_range_values() {
        let below = AppearanceSettings::validate_mobile_breakpoint_px(
            MIN_MOBILE_BREAKPOINT_PX.saturating_sub(1),
        )
        .expect_err("below min should be invalid");
        assert!(below.to_string().contains("invalid mobile breakpoint"));

        let above = AppearanceSettings::validate_mobile_breakpoint_px(
            MAX_MOBILE_BREAKPOINT_PX.saturating_add(1),
        )
        .expect_err("above max should be invalid");
        assert!(above.to_string().contains("invalid mobile breakpoint"));
    }
}
