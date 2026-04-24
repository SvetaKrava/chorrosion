// SPDX-License-Identifier: GPL-3.0-or-later
use chorrosion_config::AppConfig;
use chorrosion_infrastructure::{
    repositories::{
        AlbumRepository, ArtistRepository, DownloadClientDefinitionRepository, DuplicateRepository,
        IndexerDefinitionRepository, MetadataProfileRepository, QualityProfileRepository,
        SmartPlaylistRepository, TagRepository, TaggedEntityRepository, TrackRepository,
    },
    ResponseCache,
};
use moka::sync::Cache;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
pub mod community_indexers;
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
pub mod plugins;
pub mod quality_upgrade;
pub mod release_parsing;
pub mod release_restrictions;
pub mod scan_cache;
pub mod search_automation;
pub mod tag_embedding;
pub mod tag_sanitation;
#[cfg(test)]
pub(crate) mod test_fixtures;

pub use community_indexers::{CommunityIndexerRegistry, CommunityIndexerTemplate};
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
    parse_rss_feed, parse_search_results, GazelleClient, IndexerCapabilities, IndexerClient,
    IndexerConfig, IndexerError, IndexerProtocol, IndexerRssItem, IndexerSearchQuery,
    IndexerSearchResult, IndexerTestResult, NewznabClient, TorznabClient,
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
pub use plugins::{
    ExtensionApiHandler, ExtensionApiRequest, ExtensionApiResponse, Plugin, PluginCapability,
    PluginManifest, PluginRegistry,
};
pub use quality_upgrade::{QualityComparer, QualityUpgradeService, UpgradeDecision, UpgradeReason};
pub use release_parsing::{
    deduplicate_releases, filter_releases, find_duplicate_keys, parse_release_title, rank_releases,
    AudioQuality, CustomFormatRule, ParsedReleaseTitle, ReleaseFilterOptions,
};
pub use release_restrictions::{ReleaseRestrictionSet, RestrictionRule};
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

// Re-export tag, smart playlist, and duplicate detection domain types for API layer
pub use chorrosion_domain::{
    DuplicateDetectionMethod, DuplicateFileDetail, DuplicateGroup, EntityType, SmartPlaylist,
    SmartPlaylistCriteria, SmartPlaylistId, Tag, TagId, TaggedEntity,
};

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
struct ActivityHistoryRecord {
    item: CachedActivityItem,
    last_seen: Instant,
}

/// In-memory history of terminal download states observed across fresh polls.
#[derive(Clone, Debug)]
pub struct ActivityHistoryStore {
    max_entries: usize,
    inner: Arc<Mutex<HashMap<String, ActivityHistoryRecord>>>,
}

/// Default maximum number of download history records to retain.
const ACTIVITY_HISTORY_MAX_ENTRIES: usize = 500;

impl ActivityHistoryStore {
    /// Create a new in-memory store with bounded capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries: max_entries.max(1),
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record terminal items (completed or error) from a fresh poll.
    pub fn observe_terminal(&self, items: &[CachedActivityItem]) {
        let now = Instant::now();
        let mut records = self.inner.lock().expect("activity history store lock");

        for item in items {
            if !matches!(
                item.download.state,
                DownloadState::Completed | DownloadState::Error
            ) {
                continue;
            }

            let id = format!("{}:{}", item.definition_id, item.download.hash);
            records.insert(
                id,
                ActivityHistoryRecord {
                    item: item.clone(),
                    last_seen: now,
                },
            );
        }

        while records.len() > self.max_entries {
            let Some(evict_id) = records
                .iter()
                .min_by_key(|(_, record)| record.last_seen)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            records.remove(&evict_id);
        }
    }

    /// Return history items sorted by latest observation first.
    pub fn snapshot(&self) -> Vec<CachedActivityItem> {
        let records = self.inner.lock().expect("activity history store lock");
        let mut sorted: Vec<_> = records.values().cloned().collect();
        sorted.sort_by_key(|record| Reverse(record.last_seen));
        sorted.into_iter().map(|record| record.item).collect()
    }
}

impl Default for ActivityHistoryStore {
    fn default() -> Self {
        Self::new(ACTIVITY_HISTORY_MAX_ENTRIES)
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
        Self::new(300)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub artist_repository: Arc<dyn ArtistRepository>,
    pub album_repository: Arc<dyn AlbumRepository>,
    pub track_repository: Arc<dyn TrackRepository>,
    pub quality_profile_repository: Arc<dyn QualityProfileRepository>,
    pub metadata_profile_repository: Arc<dyn MetadataProfileRepository>,
    pub indexer_definition_repository: Arc<dyn IndexerDefinitionRepository>,
    pub download_client_definition_repository: Arc<dyn DownloadClientDefinitionRepository>,
    pub tag_repository: Arc<dyn TagRepository>,
    pub tagged_entity_repository: Arc<dyn TaggedEntityRepository>,
    pub smart_playlist_repository: Arc<dyn SmartPlaylistRepository>,
    pub duplicate_repository: Arc<dyn DuplicateRepository>,
    /// In-memory cache for serialized API GET responses.
    pub response_cache: ResponseCache,
    /// Short-lived cache for the polled download-client activity snapshot.
    pub activity_snapshot_cache: ActivitySnapshotCache,
    /// In-memory terminal-state history accumulated across fresh polls.
    pub activity_history_store: ActivityHistoryStore,
    /// In-memory tracker used to detect downloads that stop making progress.
    pub activity_stall_tracker: ActivityStallTracker,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: AppConfig,
        artist_repository: Arc<dyn ArtistRepository>,
        album_repository: Arc<dyn AlbumRepository>,
        track_repository: Arc<dyn TrackRepository>,
        quality_profile_repository: Arc<dyn QualityProfileRepository>,
        metadata_profile_repository: Arc<dyn MetadataProfileRepository>,
        indexer_definition_repository: Arc<dyn IndexerDefinitionRepository>,
        download_client_definition_repository: Arc<dyn DownloadClientDefinitionRepository>,
        tag_repository: Arc<dyn TagRepository>,
        tagged_entity_repository: Arc<dyn TaggedEntityRepository>,
        smart_playlist_repository: Arc<dyn SmartPlaylistRepository>,
        duplicate_repository: Arc<dyn DuplicateRepository>,
        response_cache: ResponseCache,
    ) -> Self {
        Self {
            activity_snapshot_cache: ActivitySnapshotCache::default(),
            activity_history_store: ActivityHistoryStore::default(),
            activity_stall_tracker: ActivityStallTracker::new(config.activity.stall_after_seconds),
            config,
            artist_repository,
            album_repository,
            track_repository,
            quality_profile_repository,
            metadata_profile_repository,
            indexer_definition_repository,
            download_client_definition_repository,
            tag_repository,
            tagged_entity_repository,
            smart_playlist_repository,
            duplicate_repository,
            response_cache,
        }
    }

    pub fn on_start(&self) {
        info!(target: "application", "application state initialized");
    }
}

#[cfg(test)]
mod matching_precedence_tests;
