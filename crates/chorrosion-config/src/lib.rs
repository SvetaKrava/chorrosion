// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::Path;

use anyhow::Result;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub pool_max_size: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://chorrosion.db".to_string(),
            pool_max_size: 16,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub host: String,
    pub port: u16,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5150,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub log_level: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub max_concurrent_jobs: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastFmAlbumSeed {
    pub artist: String,
    pub album: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastFmConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub max_concurrent_requests: usize,
    pub seed_artists: Vec<String>,
    pub seed_albums: Vec<LastFmAlbumSeed>,
}

impl Default for LastFmConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            max_concurrent_requests: 1,
            seed_artists: Vec::new(),
            seed_albums: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetadataConfig {
    pub lastfm: LastFmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub http: HttpConfig,
    pub telemetry: TelemetryConfig,
    pub scheduler: SchedulerConfig,
    pub metadata: MetadataConfig,
}

/// Load configuration from defaults, optional TOML file, and environment overrides (prefix: CHORROSION_).
pub fn load(config_path: Option<&Path>) -> Result<AppConfig> {
    let mut figment = Figment::from(Serialized::defaults(AppConfig::default()));

    if let Some(path) = config_path {
        figment = figment.merge(Toml::file(path));
    }

    figment = figment.merge(Env::prefixed("CHORROSION_").split("__"));

    let config: AppConfig = figment.extract()?;
    info!(target: "config", "configuration loaded");
    Ok(config)
}
