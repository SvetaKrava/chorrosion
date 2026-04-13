// SPDX-License-Identifier: GPL-3.0-or-later
use chorrosion_config::AppConfig;
use chorrosion_infrastructure::{
    sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    },
    ResponseCache,
};
use moka::sync::Cache;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
pub mod download_clients;
pub mod embedded_tags;
pub mod events;
pub mod file_organization;
pub mod file_replacement;
pub mod filename_heuristics;
mod http_client;
pub mod import;
pub mod import_matching;
pub mod indexers;
pub mod lists;
pub mod matching;
pub mod matching_precedence;
pub mod notifications;
pub mod permission;
pub mod quality_upgrade;
pub mod release_parsing;
pub mod scan_cache;
pub mod search_automation;
pub mod tag_embedding;
pub mod tag_sanitation;

pub use download_clients::{
    AddTorrentRequest, DelugeClient, DownloadClient, DownloadClientError, DownloadItem,
    DownloadState, NzbgetClient, QBittorrentClient, SabnzbdClient, TransmissionClient,
};
pub use embedded_tags::{
    EmbeddedTagError, EmbeddedTagMatchingService, EmbeddedTagResult, ExtractedTags,
};
pub use file_organization::{
    apply_file_operation, build_organized_file_path, render_naming_pattern, FileOperationMode,
    FileOrganizationError, TrackPathContext,
};
pub use file_replacement::{
    FileReplacementConfig, FileReplacementError, FileReplacementService, ReplacementOutcome,
};
pub use filename_heuristics::{
    FilenameHeuristicsError, FilenameHeuristicsResult, FilenameHeuristicsService, ParsedFilename,
};
pub use import::{FileImportService, ImportError, ImportResult, ImportedFile};
pub use import_matching::{
    evaluate_import_match, parse_track_metadata, scan_audio_files, CatalogAlbum, CatalogAlbumMatch,
    ImportDecision, ImportEvaluation, ImportMatchingError, MatchStrategy, MetadataSource,
    ParsedTrackMetadata, RawTrackMetadata, ScannedAudioFile,
};
pub use indexers::{
    parse_rss_feed, parse_search_results, IndexerCapabilities, IndexerClient, IndexerConfig,
    IndexerError, IndexerProtocol, IndexerRssItem, IndexerSearchQuery, IndexerSearchResult,
    IndexerTestResult, NewznabClient, TorznabClient,
};
pub use lists::{
    auto_add_from_list_entries, dedupe_list_entries, ExternalListEntry, LastFmListProvider,
    ListAutoAddSummary, ListEntityType, ListProvider, ListProviderCapabilities, ListProviderHealth,
    MusicBrainzListProvider, SpotifyPlaylistListProvider,
};
pub use matching::{MatchResult, MatchingError, MatchingResult, TrackMatchingService};
pub use matching_precedence::{
    MatchingStrategy, PrecedenceMatchResult, PrecedenceMatchingEngine, PrecedenceMatchingError,
    PrecedenceMatchingResult,
};
pub use notifications::{
    DiscordWebhookProvider, EmailNotificationProvider, NoopNotificationProvider, NotificationEvent,
    NotificationEventKind, NotificationPipeline, NotificationProvider, NotificationProviderConfig,
    NotificationProviderKind, PushoverProvider, ScriptNotificationProvider, SlackWebhookProvider,
};
pub use permission::{PermissionChecker, PermissionConfig, PermissionError, PermissionManager};
pub use quality_upgrade::{QualityComparer, QualityUpgradeService, UpgradeDecision, UpgradeReason};
pub use release_parsing::{
    deduplicate_releases, filter_releases, find_duplicate_keys, parse_release_title, rank_releases,
    AudioQuality, ParsedReleaseTitle, ReleaseFilterOptions,
};
pub use scan_cache::{cached_scan_audio_files, DirScanCache};
pub use search_automation::{
    automatic_search_missing_albums, detect_missing_albums, manual_search, AlbumSearchTarget,
    AutomaticSearchDecision, ManualSearchRequest, RankedRelease,
};
pub use tag_embedding::{
    ArtworkData, EmbeddedTagPreference, LoftyTagEmbeddingBackend, TagEmbeddingBackend,
    TagEmbeddingError, TagEmbeddingOptions, TagEmbeddingOutcome, TagEmbeddingPayload,
    TagEmbeddingRequest, TagEmbeddingService, TagFormat, TagRoundtripSnapshot,
};
pub use tag_sanitation::TagSanitizer;

use tracing::info;

/// A single download item cached from a download client poll, tagged with the
/// originating client definition metadata.
#[derive(Clone, Debug)]
pub struct CachedActivityItem {
    pub definition_id: String,
    pub definition_name: String,
    pub download: DownloadItem,
}

/// Short-lived, in-memory cache for the activity queue snapshot.
///
/// Endpoints that present different views of the download queue
/// (`/activity/queue`, `/activity/history`, `/activity/failed`) share
/// a single polled snapshot via this cache so that concurrent or
/// near-simultaneous requests do not poll download clients redundantly.
///
/// Uses a [`moka`] sync cache with a configurable TTL (default 5 s).
#[derive(Clone, Debug)]
pub struct ActivitySnapshotCache {
    inner: Cache<(), Vec<CachedActivityItem>>,
}

/// Default activity snapshot TTL in seconds.
const ACTIVITY_SNAPSHOT_TTL_SECONDS: u64 = 5;

impl ActivitySnapshotCache {
    /// Create a new cache with the given TTL (clamped to ≥ 1 s).
    pub fn new(ttl_seconds: u64) -> Self {
        let ttl = Duration::from_secs(ttl_seconds.max(1));
        Self {
            inner: Cache::builder().max_capacity(1).time_to_live(ttl).build(),
        }
    }

    /// Return the cached snapshot if still within TTL.
    pub fn get(&self) -> Option<Vec<CachedActivityItem>> {
        self.inner.get(&())
    }

    /// Replace the cached snapshot.
    pub fn set(&self, items: Vec<CachedActivityItem>) {
        self.inner.insert((), items);
    }

    /// Clear the cached snapshot so the next request performs a fresh poll.
    pub fn clear(&self) {
        self.inner.invalidate(&());
    }
}

impl Default for ActivitySnapshotCache {
    fn default() -> Self {
        Self::new(ACTIVITY_SNAPSHOT_TTL_SECONDS)
    }
}

#[derive(Clone, Debug)]
struct TrackedActivityProgress {
    progress_percent: u8,
    last_progress_at: Instant,
    repeated_samples: u32,
}

/// Tracks whether active downloads have stopped making progress across fresh polls.
#[derive(Clone, Debug)]
pub struct ActivityStallTracker {
    stall_after: Duration,
    inner: Arc<Mutex<HashMap<String, TrackedActivityProgress>>>,
}

/// Default stall detection window in seconds.
const ACTIVITY_STALL_AFTER_SECONDS: u64 = 300;

impl ActivityStallTracker {
    /// Create a new tracker with the given stall window.
    pub fn new(stall_after_seconds: u64) -> Self {
        Self {
            stall_after: Duration::from_secs(stall_after_seconds),
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record a fresh poll observation for the current download items.
    pub fn observe(&self, items: &[CachedActivityItem]) {
        self.observe_at(items, Instant::now());
    }

    /// Return the IDs currently considered stalled.
    pub fn stalled_ids(&self, items: &[CachedActivityItem]) -> Vec<String> {
        self.stalled_ids_at(items, Instant::now())
    }

    fn observe_at(&self, items: &[CachedActivityItem], now: Instant) {
        let mut tracked = self.inner.lock().expect("activity stall tracker lock");
        let mut active_ids: HashSet<String> = HashSet::new();

        for item in items {
            let id = format!("{}:{}", item.definition_id, item.download.hash);

            if item.download.state != DownloadState::Downloading {
                tracked.remove(&id);
                continue;
            }

            let progress_percent = item.download.progress_percent;
            match tracked.get_mut(&id) {
                Some(entry) if entry.progress_percent == progress_percent => {
                    entry.repeated_samples += 1;
                }
                Some(entry) => {
                    entry.progress_percent = progress_percent;
                    entry.last_progress_at = now;
                    entry.repeated_samples = 1;
                }
                None => {
                    tracked.insert(
                        id.clone(),
                        TrackedActivityProgress {
                            progress_percent,
                            last_progress_at: now,
                            repeated_samples: 1,
                        },
                    );
                }
            }

            active_ids.insert(id);
        }

        tracked.retain(|id, _| active_ids.contains(id));
    }

    fn stalled_ids_at(&self, items: &[CachedActivityItem], now: Instant) -> Vec<String> {
        let tracked = self.inner.lock().expect("activity stall tracker lock");

        items
            .iter()
            .filter(|item| item.download.state == DownloadState::Downloading)
            .filter_map(|item| {
                let id = format!("{}:{}", item.definition_id, item.download.hash);
                tracked.get(&id).and_then(|entry| {
                    if entry.progress_percent == item.download.progress_percent
                        && entry.repeated_samples >= 2
                        && now.duration_since(entry.last_progress_at) >= self.stall_after
                    {
                        Some(id)
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
}

impl Default for ActivityStallTracker {
    fn default() -> Self {
        Self::new(ACTIVITY_STALL_AFTER_SECONDS)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub artist_repository: Arc<SqliteArtistRepository>,
    pub album_repository: Arc<SqliteAlbumRepository>,
    pub track_repository: Arc<SqliteTrackRepository>,
    pub quality_profile_repository: Arc<SqliteQualityProfileRepository>,
    pub metadata_profile_repository: Arc<SqliteMetadataProfileRepository>,
    pub indexer_definition_repository: Arc<SqliteIndexerDefinitionRepository>,
    pub download_client_definition_repository: Arc<SqliteDownloadClientDefinitionRepository>,
    /// In-memory cache for serialized API GET responses.
    pub response_cache: ResponseCache,
    /// Short-lived cache for the polled download-client activity snapshot.
    pub activity_snapshot_cache: ActivitySnapshotCache,
    /// In-memory tracker used to detect downloads that stop making progress.
    pub activity_stall_tracker: ActivityStallTracker,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: AppConfig,
        artist_repository: Arc<SqliteArtistRepository>,
        album_repository: Arc<SqliteAlbumRepository>,
        track_repository: Arc<SqliteTrackRepository>,
        quality_profile_repository: Arc<SqliteQualityProfileRepository>,
        metadata_profile_repository: Arc<SqliteMetadataProfileRepository>,
        indexer_definition_repository: Arc<SqliteIndexerDefinitionRepository>,
        download_client_definition_repository: Arc<SqliteDownloadClientDefinitionRepository>,
        response_cache: ResponseCache,
    ) -> Self {
        Self {
            activity_snapshot_cache: ActivitySnapshotCache::default(),
            activity_stall_tracker: ActivityStallTracker::new(
                config.activity.stall_after_seconds,
            ),
            config,
            artist_repository,
            album_repository,
            track_repository,
            quality_profile_repository,
            metadata_profile_repository,
            indexer_definition_repository,
            download_client_definition_repository,
            response_cache,
        }
    }

    pub fn on_start(&self) {
        info!(target: "application", "application state initialized");
    }
}

#[cfg(test)]
mod matching_precedence_tests;
