// SPDX-License-Identifier: GPL-3.0-or-later
use chorrosion_config::AppConfig;
use chorrosion_infrastructure::sqlite_adapters::{
    SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
    SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
    SqliteQualityProfileRepository, SqliteTrackRepository,
};
use std::sync::Arc;
pub mod download_clients;
pub mod embedded_tags;
pub mod events;
pub mod file_organization;
pub mod filename_heuristics;
mod http_client;
pub mod import;
pub mod import_matching;
pub mod indexers;
pub mod lists;
pub mod matching;
pub mod matching_precedence;
pub mod notifications;
pub mod release_parsing;
pub mod search_automation;
pub mod tag_embedding;

pub use download_clients::{
    AddTorrentRequest, DownloadClient, DownloadClientError, DownloadItem, DownloadState,
    QBittorrentClient,
};
pub use embedded_tags::{
    EmbeddedTagError, EmbeddedTagMatchingService, EmbeddedTagResult, ExtractedTags,
};
pub use file_organization::{
    apply_file_operation, build_organized_file_path, render_naming_pattern, FileOperationMode,
    FileOrganizationError, TrackPathContext,
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
pub use release_parsing::{
    deduplicate_releases, filter_releases, find_duplicate_keys, parse_release_title, rank_releases,
    AudioQuality, ParsedReleaseTitle, ReleaseFilterOptions,
};
pub use search_automation::{
    automatic_search_missing_albums, detect_missing_albums, manual_search, AlbumSearchTarget,
    AutomaticSearchDecision, ManualSearchRequest, RankedRelease,
};
pub use tag_embedding::{
    ArtworkData, LoftyTagEmbeddingBackend, TagEmbeddingBackend, TagEmbeddingError,
    TagEmbeddingOptions, TagEmbeddingOutcome, TagEmbeddingPayload, TagEmbeddingRequest,
    TagEmbeddingService, TagFormat, TagRoundtripSnapshot,
};

use tracing::info;

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
    ) -> Self {
        Self {
            config,
            artist_repository,
            album_repository,
            track_repository,
            quality_profile_repository,
            metadata_profile_repository,
            indexer_definition_repository,
            download_client_definition_repository,
        }
    }

    pub fn on_start(&self) {
        info!(target: "application", "application state initialized");
    }
}

#[cfg(test)]
mod matching_precedence_tests;
