// SPDX-License-Identifier: GPL-3.0-or-later

//! Search orchestration for manual and automatic music release discovery.
//!
//! This module provides two complementary search flows:
//!
//! - **Manual search** ([`manual_search`]): user-driven search by artist, album, or free-form
//!   query. Results are filtered, deduplicated, and ranked so that the caller receives a
//!   sorted list of [`RankedRelease`] candidates.
//! - **Automatic search** ([`automatic_search_missing_albums`]): library-driven search that
//!   accepts a list of [`AlbumSearchTarget`]s, skips albums already owned, and for each missing
//!   album queries the indexer and returns the best-ranked release as an
//!   [`AutomaticSearchDecision`].
//!
//! Both flows share the `filter → dedupe → rank` pipeline from [`crate::release_parsing`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::indexers::{IndexerClient, IndexerError, IndexerSearchQuery, IndexerSearchResult};
use crate::release_parsing::{
    deduplicate_releases, filter_releases, parse_release_title, rank_releases, ParsedReleaseTitle,
    ReleaseFilterOptions,
};

/// Parameters for a manually initiated search against an indexer.
///
/// At least one of `artist`, `album`, or `query` must be provided as a
/// non-empty (non-whitespace) value; otherwise the request will be rejected.
/// When `query` is set and non-empty, it takes precedence over the structured
/// `artist`/`album` fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualSearchRequest {
    /// Optional artist name to constrain the search (e.g. `"Radiohead"`).
    pub artist: Option<String>,
    /// Optional album title to constrain the search (e.g. `"OK Computer"`).
    pub album: Option<String>,
    /// Optional free-form search query to send directly to the indexer.
    ///
    /// When non-empty, this field takes precedence over `artist` and `album`.
    pub query: Option<String>,
}

/// A parsed and ranked release returned from an indexer search.
///
/// This couples the raw [`IndexerSearchResult`] with the structured
/// [`ParsedReleaseTitle`] derived from the release name so that downstream
/// logic can reason about the release metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedRelease {
    /// Parsed details extracted from the release title (artist, album, quality, etc.).
    pub parsed: ParsedReleaseTitle,
    /// The original search result as returned by the indexer client.
    pub search_result: IndexerSearchResult,
}

/// A specific album that the system should search for automatically.
///
/// Instances are typically derived from a library catalog and passed to
/// [`automatic_search_missing_albums`] to identify which albums still need to
/// be acquired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlbumSearchTarget {
    /// Name of the artist whose album should be searched for.
    pub artist: String,
    /// Title of the album that is being targeted by the search.
    pub album: String,
    /// Whether this album is already owned locally.
    ///
    /// Targets marked as already owned are skipped by automated searches.
    pub already_owned: bool,
}

/// The outcome of running an automatic search for a single album target.
///
/// Contains the original [`AlbumSearchTarget`] and, if any were found, the
/// highest-ranked release that matched that target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomaticSearchDecision {
    /// The album that was evaluated by the automated search.
    pub target: AlbumSearchTarget,
    /// The best-ranked release candidate found for this target, or `None` if
    /// nothing suitable was found.
    pub best_release: Option<RankedRelease>,
}

/// Execute a user-driven search against an indexer and return ranked results.
///
/// The query is built from the fields of `request` (see [`ManualSearchRequest`]).
/// Results are passed through the `filter → dedupe → rank` pipeline before being
/// returned as a sorted [`Vec<RankedRelease>`].
///
/// # Arguments
///
/// * `indexer` – The indexer client to query.
/// * `request` – Search parameters (artist, album, or free-form query).
/// * `options` – Filter and ranking preferences (quality, bitrate, release groups).
///
/// # Returns
///
/// * `Ok(Vec<RankedRelease>)` – Ranked list of matching releases (may be empty).
/// * `Err(IndexerError)` – Query could not be built or the indexer returned an error.
pub async fn manual_search<I: IndexerClient>(
    indexer: &I,
    request: &ManualSearchRequest,
    options: &ReleaseFilterOptions,
) -> Result<Vec<RankedRelease>, IndexerError> {
    let query = build_manual_query(request)?;
    debug!(
        target: "search_automation",
        indexer = %indexer.config().name,
        query = %query,
        "executing manual search"
    );
    let raw_results = indexer
        .search(&IndexerSearchQuery {
            query,
            category: Some("music".to_string()),
            limit: Some(100),
            offset: Some(0),
        })
        .await?;

    Ok(rank_results(raw_results, options))
}

/// Search for all missing albums in `targets` and return one decision per target.
///
/// Albums marked as `already_owned` are skipped. For each remaining target, the
/// indexer is queried using `"<artist> <album>"` and the top-ranked result is
/// selected as the best release candidate.
///
/// # Arguments
///
/// * `indexer` – The indexer client to query.
/// * `targets` – Slice of album targets to evaluate.
/// * `options` – Filter and ranking preferences applied to each search.
///
/// # Returns
///
/// * `Ok(Vec<AutomaticSearchDecision>)` – One decision per missing target (may be empty).
/// * `Err(IndexerError)` – The indexer returned an error for one of the queries.
pub async fn automatic_search_missing_albums<I: IndexerClient>(
    indexer: &I,
    targets: &[AlbumSearchTarget],
    options: &ReleaseFilterOptions,
) -> Result<Vec<AutomaticSearchDecision>, IndexerError> {
    let missing_targets = detect_missing_albums(targets);
    debug!(
        target: "search_automation",
        indexer = %indexer.config().name,
        missing_count = missing_targets.len(),
        "starting automatic search for missing albums"
    );

    let mut decisions = Vec::with_capacity(missing_targets.len());
    for target in missing_targets {
        let query = format!("{} {}", &target.artist, &target.album);
        debug!(
            target: "search_automation",
            artist = %target.artist,
            album = %target.album,
            query = %query,
            "searching for missing album"
        );
        let raw_results = indexer
            .search(&IndexerSearchQuery {
                query,
                category: Some("music".to_string()),
                limit: Some(100),
                offset: Some(0),
            })
            .await?;

        let ranked = rank_results(raw_results, options);
        let best_release = ranked.into_iter().next();
        debug!(
            target: "search_automation",
            artist = %target.artist,
            album = %target.album,
            found = best_release.is_some(),
            "automatic search decision made"
        );

        decisions.push(AutomaticSearchDecision {
            target,
            best_release,
        });
    }

    Ok(decisions)
}

/// Returns the subset of album search targets that are not already owned.
///
/// # Arguments
///
/// * `targets` – A slice of [`AlbumSearchTarget`] values to inspect.
///
/// # Returns
///
/// A `Vec<AlbumSearchTarget>` containing only those targets where
/// `already_owned` is `false`. The returned targets are cloned from the input.
pub fn detect_missing_albums(targets: &[AlbumSearchTarget]) -> Vec<AlbumSearchTarget> {
    targets
        .iter()
        .filter(|target| !target.already_owned)
        .cloned()
        .collect()
}

fn build_manual_query(request: &ManualSearchRequest) -> Result<String, IndexerError> {
    if let Some(query) = request.query.as_deref() {
        let query = query.trim();
        if !query.is_empty() {
            return Ok(query.to_string());
        }
    }

    let mut parts = Vec::new();
    if let Some(artist) = request.artist.as_deref() {
        let artist = artist.trim();
        if !artist.is_empty() {
            parts.push(artist.to_string());
        }
    }
    if let Some(album) = request.album.as_deref() {
        let album = album.trim();
        if !album.is_empty() {
            parts.push(album.to_string());
        }
    }

    if parts.is_empty() {
        return Err(IndexerError::Request(
            "manual search requires either query or artist/album".to_string(),
        ));
    }

    Ok(parts.join(" "))
}

fn rank_results(
    raw_results: Vec<IndexerSearchResult>,
    options: &ReleaseFilterOptions,
) -> Vec<RankedRelease> {
    // Parse titles before consuming the vec so we avoid an extra clone.
    let parsed_titles: Vec<ParsedReleaseTitle> = raw_results
        .iter()
        .map(|r| parse_release_title(&r.title))
        .collect();

    // Build a title→result map for O(1) lookup when pairing ranked titles back
    // to their original IndexerSearchResult (avoids O(n*m) nested scan).
    // Use first-win semantics: if multiple results share the same title, keep
    // the first one encountered rather than overwriting with later entries.
    let mut result_map: HashMap<String, IndexerSearchResult> = HashMap::new();
    for r in raw_results {
        result_map.entry(r.title.clone()).or_insert(r);
    }

    let filtered = filter_releases(&parsed_titles, options);
    let deduped = deduplicate_releases(&filtered);
    let ranked = rank_releases(deduped, options);

    ranked
        .into_iter()
        .filter_map(|parsed| {
            result_map
                .get(&parsed.original_title)
                .cloned()
                .map(|search_result| RankedRelease {
                    parsed,
                    search_result,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        automatic_search_missing_albums, detect_missing_albums, manual_search, AlbumSearchTarget,
        ManualSearchRequest,
    };
    use crate::indexers::{
        IndexerCapabilities, IndexerClient, IndexerConfig, IndexerError, IndexerProtocol,
        IndexerRssItem, IndexerSearchQuery, IndexerSearchResult, IndexerTestResult,
    };
    use crate::release_parsing::{AudioQuality, ReleaseFilterOptions};
    use async_trait::async_trait;

    #[derive(Clone)]
    struct FakeIndexer {
        config: IndexerConfig,
    }

    impl FakeIndexer {
        fn new() -> Self {
            Self {
                config: IndexerConfig {
                    name: "fake".to_string(),
                    base_url: "https://example.invalid".to_string(),
                    protocol: IndexerProtocol::Custom,
                    api_key: None,
                    enabled: true,
                },
            }
        }
    }

    #[async_trait]
    impl IndexerClient for FakeIndexer {
        fn config(&self) -> &IndexerConfig {
            &self.config
        }

        async fn detect_capabilities(&self) -> Result<IndexerCapabilities, IndexerError> {
            Ok(IndexerCapabilities {
                supports_search: true,
                supports_rss: true,
                supports_capabilities_detection: true,
                supports_categories: true,
                supported_categories: vec!["music".to_string()],
            })
        }

        async fn search(
            &self,
            query: &IndexerSearchQuery,
        ) -> Result<Vec<IndexerSearchResult>, IndexerError> {
            if query.query.to_lowercase().contains("daft punk") {
                return Ok(vec![
                    IndexerSearchResult {
                        title: "Daft Punk - Discovery [FLAC]-A".to_string(),
                        guid: Some("1".to_string()),
                        download_url: Some("magnet:?xt=1".to_string()),
                        published_at: None,
                        size_bytes: None,
                        seeders: Some(10),
                        leechers: Some(1),
                    },
                    IndexerSearchResult {
                        title: "Daft Punk - Discovery 320kbps MP3-B".to_string(),
                        guid: Some("2".to_string()),
                        download_url: Some("magnet:?xt=2".to_string()),
                        published_at: None,
                        size_bytes: None,
                        seeders: Some(8),
                        leechers: Some(2),
                    },
                ]);
            }

            if query.query.to_lowercase().contains("radiohead") {
                return Ok(vec![IndexerSearchResult {
                    title: "Radiohead - OK Computer 320kbps MP3-RLS".to_string(),
                    guid: Some("3".to_string()),
                    download_url: Some("magnet:?xt=3".to_string()),
                    published_at: None,
                    size_bytes: None,
                    seeders: Some(4),
                    leechers: Some(1),
                }]);
            }

            Ok(Vec::new())
        }

        async fn fetch_rss_feed(&self) -> Result<Vec<IndexerRssItem>, IndexerError> {
            Ok(Vec::new())
        }

        async fn test_connection(&self) -> Result<IndexerTestResult, IndexerError> {
            Ok(IndexerTestResult {
                success: true,
                message: "ok".to_string(),
                capabilities: None,
            })
        }
    }

    #[test]
    fn detects_missing_targets_only() {
        let targets = vec![
            AlbumSearchTarget {
                artist: "Daft Punk".to_string(),
                album: "Discovery".to_string(),
                already_owned: false,
            },
            AlbumSearchTarget {
                artist: "Radiohead".to_string(),
                album: "OK Computer".to_string(),
                already_owned: true,
            },
        ];

        let missing = detect_missing_albums(&targets);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].artist, "Daft Punk");
    }

    #[tokio::test]
    async fn manual_search_ranks_lossless_above_lossy() {
        let indexer = FakeIndexer::new();
        let request = ManualSearchRequest {
            artist: Some("Daft Punk".to_string()),
            album: Some("Discovery".to_string()),
            query: None,
        };

        let results = manual_search(&indexer, &request, &ReleaseFilterOptions::default())
            .await
            .expect("manual search should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].parsed.quality, AudioQuality::Flac);
    }

    #[tokio::test]
    async fn automatic_search_selects_best_release_for_missing_album() {
        let indexer = FakeIndexer::new();
        let targets = vec![AlbumSearchTarget {
            artist: "Radiohead".to_string(),
            album: "OK Computer".to_string(),
            already_owned: false,
        }];

        let decisions = automatic_search_missing_albums(
            &indexer,
            &targets,
            &ReleaseFilterOptions {
                preferred_qualities: vec![AudioQuality::Mp3],
                min_bitrate_kbps: Some(256),
                preferred_release_groups: vec![],
            },
        )
        .await
        .expect("automatic search should succeed");

        assert_eq!(decisions.len(), 1);
        assert!(decisions[0].best_release.is_some());
        assert_eq!(
            decisions[0]
                .best_release
                .as_ref()
                .and_then(|r| r.parsed.album.as_deref()),
            Some("OK Computer")
        );
    }

    #[tokio::test]
    async fn manual_search_query_field_takes_precedence_over_artist_album() {
        let indexer = FakeIndexer::new();
        // query="daft punk" should override artist="Radiohead"/album="OK Computer"
        let request = ManualSearchRequest {
            artist: Some("Radiohead".to_string()),
            album: Some("OK Computer".to_string()),
            query: Some("daft punk".to_string()),
        };

        let results = manual_search(&indexer, &request, &ReleaseFilterOptions::default())
            .await
            .expect("manual search with query override should succeed");

        // FakeIndexer returns Daft Punk results for "daft punk", not Radiohead
        assert!(!results.is_empty());
        assert!(results[0]
            .search_result
            .title
            .to_lowercase()
            .contains("daft punk"));
    }

    #[tokio::test]
    async fn automatic_search_returns_none_best_release_when_no_results() {
        let indexer = FakeIndexer::new();
        // FakeIndexer returns empty vec for any query not containing known keywords
        let targets = vec![AlbumSearchTarget {
            artist: "Unknown Artist".to_string(),
            album: "Nonexistent Album".to_string(),
            already_owned: false,
        }];

        let decisions = automatic_search_missing_albums(
            &indexer,
            &targets,
            &ReleaseFilterOptions::default(),
        )
        .await
        .expect("automatic search should succeed even with no results");

        assert_eq!(decisions.len(), 1);
        assert!(
            decisions[0].best_release.is_none(),
            "expected no best release when indexer returns no results"
        );
    }

    #[tokio::test]
    async fn manual_search_returns_error_for_empty_request() {
        let indexer = FakeIndexer::new();
        let request = ManualSearchRequest {
            artist: None,
            album: None,
            query: None,
        };

        let result = manual_search(&indexer, &request, &ReleaseFilterOptions::default()).await;
        assert!(
            result.is_err(),
            "expected error for empty manual search request"
        );
        if let Err(IndexerError::Request(msg)) = result {
            assert!(
                msg.contains("manual search requires"),
                "error message should mention requirement: {msg}"
            );
        } else {
            panic!("expected IndexerError::Request variant");
        }
    }

    #[tokio::test]
    async fn manual_search_returns_error_for_whitespace_only_fields() {
        let indexer = FakeIndexer::new();
        let request = ManualSearchRequest {
            artist: Some("   ".to_string()),
            album: Some("   ".to_string()),
            query: Some("   ".to_string()),
        };

        let result = manual_search(&indexer, &request, &ReleaseFilterOptions::default()).await;
        assert!(
            result.is_err(),
            "expected error when all fields are whitespace"
        );
    }
}
