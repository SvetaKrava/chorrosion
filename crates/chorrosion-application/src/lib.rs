// SPDX-License-Identifier: GPL-3.0-or-later
use chorrosion_config::AppConfig;
pub mod embedded_tags;
pub mod download_clients;
pub mod events;
pub mod filename_heuristics;
pub mod import;
pub mod indexers;
pub mod matching;
pub mod matching_precedence;
pub mod release_parsing;
pub mod search_automation;

pub use embedded_tags::{
    EmbeddedTagError, EmbeddedTagMatchingService, EmbeddedTagResult, ExtractedTags,
};
pub use download_clients::{
    AddTorrentRequest, DownloadClient, DownloadClientError, DownloadItem, DownloadState,
    QBittorrentClient,
};
pub use filename_heuristics::{
    FilenameHeuristicsError, FilenameHeuristicsResult, FilenameHeuristicsService, ParsedFilename,
};
pub use import::{FileImportService, ImportError, ImportResult, ImportedFile};
pub use indexers::{
    parse_rss_feed, parse_search_results, IndexerCapabilities, IndexerClient, IndexerConfig,
    IndexerError, IndexerProtocol, IndexerRssItem, IndexerSearchQuery, IndexerSearchResult,
    IndexerTestResult, NewznabClient, TorznabClient,
};
pub use matching::{MatchResult, MatchingError, MatchingResult, TrackMatchingService};
pub use matching_precedence::{
    MatchingStrategy, PrecedenceMatchResult, PrecedenceMatchingEngine, PrecedenceMatchingError,
    PrecedenceMatchingResult,
};
pub use release_parsing::{
    deduplicate_releases, filter_releases, find_duplicate_keys, parse_release_title,
    rank_releases, AudioQuality, ParsedReleaseTitle, ReleaseFilterOptions,
};
pub use search_automation::{
    automatic_search_missing_albums, detect_missing_albums, manual_search,
    AlbumSearchTarget, AutomaticSearchDecision, ManualSearchRequest, RankedRelease,
};

use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn on_start(&self) {
        info!(target: "application", "application state initialized");
    }
}

#[cfg(test)]
mod matching_precedence_tests;
