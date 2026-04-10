// SPDX-License-Identifier: GPL-3.0-or-later
use std::path::Path;

use anyhow::Result;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

/// Default request timeout (in seconds) for external metadata API HTTP clients.
///
/// Used as the default in all metadata-related config sections (`LastFmConfig`,
/// `DiscogsConfig`, `LyricsConfig`, `CoverArtConfig`) so the configuration defaults
/// stay consistent with each other.
pub const DEFAULT_METADATA_REQUEST_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub pool_max_size: u32,
    /// Queries that take longer than this threshold (in milliseconds) are logged at WARN level.
    /// Set to 0 to disable slow-query logging.
    pub slow_query_threshold_ms: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://chorrosion.db".to_string(),
            pool_max_size: 16,
            slow_query_threshold_ms: 50,
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
    /// HTTP requests that take longer than this threshold (in milliseconds) are logged at WARN
    /// level.  Set to `0` to disable slow-request logging.
    /// Env override: `CHORROSION_TELEMETRY__SLOW_REQUEST_THRESHOLD_MS`.
    pub slow_request_threshold_ms: u64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            slow_request_threshold_ms: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub max_concurrent_jobs: usize,
    /// Maximum number of files to process concurrently in a batch import.
    /// Must be >= 1. Bounded concurrency prevents overwhelming the fingerprint
    /// engine and OS file-descriptor limits while still providing a large speedup
    /// over serial processing.
    ///
    /// This value is passed as the `max_concurrent_imports` parameter when
    /// constructing a `FileImportService` in the application layer.
    /// Env override: `CHORROSION_SCHEDULER__MAX_CONCURRENT_IMPORTS`.
    pub max_concurrent_imports: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 8,
            max_concurrent_imports: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    ReadOnly,
    #[default]
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub basic_username: Option<String>,
    pub basic_password: Option<String>,
    pub basic_permission_level: PermissionLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastFmAlbumSeed {
    pub artist: String,
    pub album: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscogsAlbumSeed {
    pub artist: String,
    pub album: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsTrackSeed {
    pub artist: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastFmConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub max_concurrent_requests: usize,
    pub request_timeout_seconds: u64,
    pub seed_artists: Vec<String>,
    pub seed_albums: Vec<LastFmAlbumSeed>,
}

impl Default for LastFmConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            max_concurrent_requests: 1,
            request_timeout_seconds: DEFAULT_METADATA_REQUEST_TIMEOUT_SECS,
            seed_artists: Vec::new(),
            seed_albums: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscogsConfig {
    pub token: Option<String>,
    pub base_url: Option<String>,
    pub max_concurrent_requests: usize,
    pub request_timeout_seconds: u64,
    pub seed_artists: Vec<String>,
    pub seed_albums: Vec<DiscogsAlbumSeed>,
}

impl Default for DiscogsConfig {
    fn default() -> Self {
        Self {
            token: None,
            base_url: None,
            max_concurrent_requests: 1,
            request_timeout_seconds: DEFAULT_METADATA_REQUEST_TIMEOUT_SECS,
            seed_artists: Vec::new(),
            seed_albums: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsConfig {
    pub base_url: Option<String>,
    pub max_concurrent_requests: usize,
    pub request_timeout_seconds: u64,
    pub seed_tracks: Vec<LyricsTrackSeed>,
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            max_concurrent_requests: 1,
            request_timeout_seconds: DEFAULT_METADATA_REQUEST_TIMEOUT_SECS,
            seed_tracks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverArtConfig {
    pub fanart_api_key: Option<String>,
    pub fanart_client_key: Option<String>,
    pub fanart_base_url: Option<String>,
    pub cover_art_archive_base_url: Option<String>,
    pub max_concurrent_requests: usize,
    pub request_timeout_seconds: u64,
    pub provider_order: Vec<String>,
}

impl Default for CoverArtConfig {
    fn default() -> Self {
        Self {
            fanart_api_key: None,
            fanart_client_key: None,
            fanart_base_url: None,
            cover_art_archive_base_url: None,
            max_concurrent_requests: 1,
            request_timeout_seconds: DEFAULT_METADATA_REQUEST_TIMEOUT_SECS,
            provider_order: vec!["fanarttv".to_string(), "coverartarchive".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetadataConfig {
    pub lastfm: LastFmConfig,
    pub discogs: DiscogsConfig,
    pub lyrics: LyricsConfig,
    pub cover_art: CoverArtConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmailNotificationConfig {
    pub enabled: bool,
    pub from: Option<String>,
    pub to: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscordNotificationConfig {
    pub enabled: bool,
    pub webhook_url: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlackNotificationConfig {
    pub enabled: bool,
    pub webhook_url: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PushoverNotificationConfig {
    pub enabled: bool,
    pub api_token: Option<String>,
    pub user_key: Option<String>,
    pub api_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScriptNotificationConfig {
    pub enabled: bool,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub working_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MusicBrainzListsConfig {
    pub enabled: bool,
    pub base_url: Option<String>,
    pub artist_mbids: Vec<String>,
    pub album_mbids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpotifyListsConfig {
    pub enabled: bool,
    #[serde(alias = "api_base_url")]
    pub base_url: Option<String>,
    pub access_token: Option<String>,
    pub playlist_ids: Vec<String>,
    pub market: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastFmListsAlbumSeed {
    pub artist: String,
    pub album: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LastFmListsConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub artist_names: Vec<String>,
    pub album_seeds: Vec<LastFmListsAlbumSeed>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListsConfig {
    pub musicbrainz: MusicBrainzListsConfig,
    pub spotify: SpotifyListsConfig,
    pub lastfm: LastFmListsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationsConfig {
    pub email: EmailNotificationConfig,
    pub discord: DiscordNotificationConfig,
    pub slack: SlackNotificationConfig,
    pub pushover: PushoverNotificationConfig,
    pub script: ScriptNotificationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// TTL in seconds for cached API GET responses. Set to 0 to disable.
    pub api_response_ttl_seconds: u64,
    /// Maximum number of cached API responses kept in memory.
    pub api_response_max_capacity: u64,
    /// Maximum response body size in bytes that will be buffered and stored in the API
    /// response cache.  Responses whose `Content-Length` header exceeds this value are
    /// passed through without caching to avoid unbounded memory use.
    pub api_response_max_body_bytes: usize,
    /// Maximum number of cached metadata artist entries per client.
    pub metadata_artist_max_capacity: u64,
    /// Maximum number of cached metadata album entries per client.
    pub metadata_album_max_capacity: u64,
    /// Maximum number of cached cover art entries (FanartTV / Cover Art Archive).
    /// Pre-defined for future use when the cover-art client is wired into the scheduler;
    /// currently not yet consumed by any construction site.
    pub metadata_cover_art_max_capacity: u64,
    /// Maximum number of cached lyrics entries.
    /// Pre-defined for future use when the lyrics client is wired into the scheduler;
    /// currently not yet consumed by any construction site.
    pub metadata_lyrics_max_capacity: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            api_response_ttl_seconds: 60,
            api_response_max_capacity: 1_000,
            api_response_max_body_bytes: 16 * 1024 * 1024,
            metadata_artist_max_capacity: 5_000,
            metadata_album_max_capacity: 5_000,
            metadata_cover_art_max_capacity: 5_000,
            metadata_lyrics_max_capacity: 5_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub http: HttpConfig,
    pub telemetry: TelemetryConfig,
    pub scheduler: SchedulerConfig,
    pub auth: AuthConfig,
    pub cache: CacheConfig,
    pub metadata: MetadataConfig,
    pub notifications: NotificationsConfig,
    pub lists: ListsConfig,
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
